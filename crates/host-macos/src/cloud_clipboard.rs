//! Bounded clipboard RPC to the native account owner. No credentials cross IPC.
use serde::Deserialize;
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

#[derive(Debug, PartialEq, Eq)]
pub enum CloudClipboardError {
    Invalid,
    Unavailable,
    OutcomeUnknown,
    Conflict,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudClipboardSession {
    version: u8,
    path: String,
    host_pid: i32,
}

impl CloudClipboardSession {
    pub fn parse(value: &str) -> Result<Self, CloudClipboardError> {
        if value.len() > super::SESSION_JSON_MAX_BYTES {
            return Err(CloudClipboardError::Invalid);
        }
        let session: Self =
            serde_json::from_str(value).map_err(|_| CloudClipboardError::Invalid)?;
        if session.version != 1
            || session.host_pid <= 0
            || !super::valid_session_socket_path(&session.path)
        {
            return Err(CloudClipboardError::Invalid);
        }
        Ok(session)
    }

    pub fn request(&self, action: &Value) -> Result<Value, CloudClipboardError> {
        self.request_bounded(action, 64 * 1024, Duration::from_secs(40))
    }

    pub fn request_dictionary(&self, action: &Value) -> Result<Value, CloudClipboardError> {
        let timeout = if action["operation"] == "export" {
            610
        } else {
            40
        };
        self.request_bounded(action, 512 * 1024, Duration::from_secs(timeout))
    }

    fn request_bounded(
        &self,
        action: &Value,
        maximum: usize,
        timeout: Duration,
    ) -> Result<Value, CloudClipboardError> {
        let bytes = serde_json::to_vec(action).map_err(|_| CloudClipboardError::Invalid)?;
        if !action.is_object() || bytes.len() > maximum {
            return Err(CloudClipboardError::Invalid);
        }
        let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
            .map_err(|_| CloudClipboardError::Unavailable)?;
        let address =
            socket2::SockAddr::unix(&self.path).map_err(|_| CloudClipboardError::Invalid)?;
        socket
            .connect_timeout(&address, Duration::from_secs(2))
            .map_err(|_| CloudClipboardError::Unavailable)?;
        let mut stream: UnixStream = socket.into();
        if !super::panel_session::peer_matches(&stream, self.host_pid) {
            return Err(CloudClipboardError::Unavailable);
        }
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|_| CloudClipboardError::Unavailable)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|_| CloudClipboardError::Unavailable)?;
        // Never retry a mutation after any frame byte may have been written.
        stream
            .write_all(&(bytes.len() as u32).to_be_bytes())
            .map_err(|_| CloudClipboardError::OutcomeUnknown)?;
        stream
            .write_all(&bytes)
            .map_err(|_| CloudClipboardError::OutcomeUnknown)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| CloudClipboardError::OutcomeUnknown)?;
        let mut length = [0_u8; 4];
        stream
            .read_exact(&mut length)
            .map_err(|_| CloudClipboardError::OutcomeUnknown)?;
        let length = u32::from_be_bytes(length) as usize;
        if length == 0 || length > 2 * 1024 * 1024 {
            return Err(CloudClipboardError::OutcomeUnknown);
        }
        let mut response = vec![0; length];
        stream
            .read_exact(&mut response)
            .map_err(|_| CloudClipboardError::OutcomeUnknown)?;
        let result: Value =
            serde_json::from_slice(&response).map_err(|_| CloudClipboardError::OutcomeUnknown)?;
        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(if result["ok"] == false && result["error"] == "conflict" {
                CloudClipboardError::Conflict
            } else {
                CloudClipboardError::Unavailable
            });
        }
        result
            .get("value")
            .filter(|value| value.is_object())
            .cloned()
            .ok_or(CloudClipboardError::OutcomeUnknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    #[test]
    fn malformed_configuration_is_rejected_without_transport() {
        for value in [
            serde_json::json!({"version":2,"path":"/tmp/synthetic.sock","host_pid":1}),
            serde_json::json!({"version":1,"path":"relative","host_pid":1}),
            serde_json::json!({"version":1,"path":"/tmp/synthetic.sock","host_pid":0}),
            serde_json::json!({"version":1,"path":"/tmp/synthetic.sock","host_pid":1,"token":"synthetic"}),
        ] {
            assert!(matches!(
                CloudClipboardSession::parse(&value.to_string()),
                Err(CloudClipboardError::Invalid)
            ));
        }
    }

    #[test]
    fn lost_or_malformed_acknowledgements_never_retry_mutations() {
        for response in [
            vec![],
            vec![0, 32, 0, 1],
            vec![0, 0, 0, 8, b'{'],
            vec![0, 0, 0, 2, b'[', b']'],
        ] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("cloud.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let session = CloudClipboardSession::parse(
                &serde_json::json!({"version":1,"path":path,"host_pid":std::process::id()})
                    .to_string(),
            )
            .unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = vec![];
                socket.read_to_end(&mut request).unwrap();
                assert!(!request.is_empty());
                socket.write_all(&response).unwrap();
                drop(socket);
                listener
            });
            assert!(session
                .request(&serde_json::json!({"operation":"delete","id":"a".repeat(64)}))
                .is_err());
            let listener = server.join().unwrap();
            listener.set_nonblocking(true).unwrap();
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
    }

    #[test]
    fn authenticated_frames_preserve_multiline_unicode_and_reject_wrong_peers() {
        for authorized in [true, false] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("cloud.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let session = CloudClipboardSession::parse(&serde_json::json!({"version":1,"path":path,"host_pid":std::process::id() + u32::from(!authorized)}).to_string()).unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                let mut bytes = vec![];
                socket.read_to_end(&mut bytes).unwrap();
                if !authorized {
                    assert!(bytes.is_empty());
                    return;
                }
                assert_eq!(
                    u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize,
                    bytes.len() - 4
                );
                let action: Value = serde_json::from_slice(&bytes[4..]).unwrap();
                assert_eq!(action["text"], "合成\n\t😀".repeat(500));
                let response = br#"{"ok":true,"value":{"enabled":true,"items":[]}}"#;
                socket
                    .write_all(&(response.len() as u32).to_be_bytes())
                    .unwrap();
                socket.write_all(response).unwrap();
            });
            let result = session
                .request(&serde_json::json!({"operation":"add","text":"合成\n\t😀".repeat(500)}));
            if authorized {
                assert_eq!(result.unwrap()["enabled"], true);
            } else {
                assert_eq!(result, Err(CloudClipboardError::Unavailable));
            }
            server.join().unwrap();
        }
    }
}
