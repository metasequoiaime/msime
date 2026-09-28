//! One-shot, kernel-authenticated transport to the IMK client which opened a panel.
use serde::Deserialize;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    Invalid,
    Unavailable,
    Rejected,
    OutcomeUnknown,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    version: u8,
    path: String,
    host_pid: i32,
    target_pid: i32,
    target_started: f64,
    #[serde(default)]
    clipboard: bool,
}

/// Does not implement Debug: paths and process identity stay out of logs/UI.
pub struct PanelSession {
    configuration: Configuration,
    submitted: AtomicBool,
}

impl PanelSession {
    pub fn accepts_clipboard(&self) -> bool {
        self.configuration.clipboard
    }

    pub fn is_used(&self) -> bool {
        self.submitted.load(Ordering::Acquire)
    }

    fn connect(&self) -> Result<UnixStream, SessionError> {
        let socket = socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)
            .map_err(|_| SessionError::Unavailable)?;
        let address =
            socket2::SockAddr::unix(&self.configuration.path).map_err(|_| SessionError::Invalid)?;
        socket
            .connect_timeout(&address, Duration::from_secs(2))
            .map_err(|_| SessionError::Unavailable)?;
        let stream: UnixStream = socket.into();
        if !peer_matches(&stream, self.configuration.host_pid) {
            return Err(SessionError::Unavailable);
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .map_err(|_| SessionError::Unavailable)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| SessionError::Unavailable)?;
        Ok(stream)
    }

    pub fn cancel(&self) {
        if self.submitted.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut stream) = self.connect() {
            let _ = stream.write_all(&0_u32.to_be_bytes());
            let _ = stream.shutdown(std::net::Shutdown::Write);
        }
    }
    pub fn parse(value: &str) -> Result<Self, SessionError> {
        if value.len() > super::SESSION_JSON_MAX_BYTES {
            return Err(SessionError::Invalid);
        }
        let configuration: Configuration =
            serde_json::from_str(value).map_err(|_| SessionError::Invalid)?;
        if configuration.version != 1
            || !super::valid_session_socket_path(&configuration.path)
            || configuration.host_pid <= 0
            || configuration.target_pid <= 0
            || !configuration.target_started.is_finite()
            || configuration.target_started <= 0.0
        {
            return Err(SessionError::Invalid);
        }
        Ok(Self {
            configuration,
            submitted: AtomicBool::new(false),
        })
    }

    /// Main thread only. The caller hides its panel before handing focus back.
    pub fn activate_target(&self) -> bool {
        unsafe extern "C" {
            fn msime_macos_activate_panel_target(pid: i32, launched: f64) -> bool;
        }
        // SAFETY: scalar ABI; native code checks thread, identity and foreground.
        unsafe {
            msime_macos_activate_panel_target(
                self.configuration.target_pid,
                self.configuration.target_started,
            )
        }
    }

    pub fn submit(&self, text: &str) -> Result<(), SessionError> {
        self.submit_with_mode(text, self.accepts_clipboard())
    }

    pub fn submit_candidate(&self, text: &str) -> Result<(), SessionError> {
        self.submit_with_mode(text, false)
    }

    pub fn submit_clipboard(&self, text: &str) -> Result<(), SessionError> {
        if !self.accepts_clipboard() {
            return Err(SessionError::Rejected);
        }
        self.submit_with_mode(text, true)
    }

    fn submit_with_mode(&self, text: &str, clipboard: bool) -> Result<(), SessionError> {
        if clipboard {
            validate_clipboard_text(text)?;
        } else if text.is_empty() || text.len() > 4096 || text.contains('\0') {
            return Err(SessionError::Invalid);
        }
        if self.submitted.load(Ordering::Acquire) {
            return Err(SessionError::Rejected);
        }
        let mut stream = self.connect()?;
        // Never retry after any part of a frame may have been sent, even if its
        // acknowledgement is lost. The IMK session is also one-shot.
        if self.submitted.swap(true, Ordering::AcqRel) {
            return Err(SessionError::Rejected);
        }
        stream
            .write_all(&(text.len() as u32).to_be_bytes())
            .map_err(|_| SessionError::OutcomeUnknown)?;
        stream
            .write_all(text.as_bytes())
            .map_err(|_| SessionError::OutcomeUnknown)?;
        stream
            .shutdown(std::net::Shutdown::Write)
            .map_err(|_| SessionError::OutcomeUnknown)?;
        let mut result = [0_u8];
        stream
            .read_exact(&mut result)
            .map_err(|_| SessionError::OutcomeUnknown)?;
        match result[0] {
            0 => Ok(()),
            1 => Err(SessionError::Rejected),
            _ => Err(SessionError::OutcomeUnknown),
        }
    }
}

pub fn validate_clipboard_text(text: &str) -> Result<(), SessionError> {
    if text.is_empty()
        || text.len() > 12_000
        || !msime_client_core::is_bounded_utf16(text, 4_000)
        || text.contains('\0')
    {
        return Err(SessionError::Invalid);
    }
    Ok(())
}

pub(crate) fn peer_matches(stream: &UnixStream, expected: i32) -> bool {
    unsafe extern "C" {
        fn getuid() -> u32;
        fn getpeereid(fd: i32, uid: *mut u32, gid: *mut u32) -> i32;
        fn getsockopt(
            fd: i32,
            level: i32,
            option: i32,
            value: *mut std::ffi::c_void,
            len: *mut u32,
        ) -> i32;
    }
    let (mut uid, mut gid, mut pid) = (0_u32, 0_u32, 0_i32);
    let mut size = std::mem::size_of::<i32>() as u32;
    // SAFETY: initialized scalar output buffers with their exact sizes. Darwin
    // SOL_LOCAL=0 and LOCAL_PEERPID=2 return kernel-authenticated process identity.
    unsafe {
        getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) == 0
            && uid == getuid()
            && getsockopt(
                stream.as_raw_fd(),
                0,
                2,
                (&mut pid as *mut i32).cast(),
                &mut size,
            ) == 0
            && size == std::mem::size_of::<i32>() as u32
            && pid == expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    #[test]
    fn clipboard_contract_counts_utf16_and_keeps_multiline_text() {
        for text in [
            "中".repeat(4000),
            "😀".repeat(2000),
            "中".repeat(3997) + "\r\n\t",
            "form\u{000C}feed\u{1b}escape".into(),
        ] {
            assert_eq!(validate_clipboard_text(&text), Ok(()));
        }
        for text in [
            "".into(),
            "a".repeat(4001),
            "😀".repeat(2001),
            "synthetic\0".into(),
            "synthetic\0\u{1b}".into(),
        ] {
            assert_eq!(validate_clipboard_text(&text), Err(SessionError::Invalid));
        }
    }

    #[test]
    fn clipboard_sessions_use_large_frames_without_changing_candidate_limits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let mut config: serde_json::Value =
            serde_json::from_str(&configuration(&path, std::process::id())).unwrap();
        config["clipboard"] = true.into();
        let session = PanelSession::parse(&config.to_string()).unwrap();
        assert!(session.accepts_clipboard());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            assert_eq!(u32::from_be_bytes(bytes[..4].try_into().unwrap()), 12000);
            assert_eq!(&bytes[4..], "中".repeat(4000).as_bytes());
            stream.write_all(&[0]).unwrap();
        });
        assert_eq!(
            session.submit(&"a".repeat(4001)),
            Err(SessionError::Invalid)
        );
        assert!(!session.is_used());
        assert_eq!(session.submit(&"中".repeat(4000)), Ok(()));
        assert_eq!(
            session.submit("synthetic-duplicate"),
            Err(SessionError::Rejected)
        );
        server.join().unwrap();
    }

    #[test]
    fn clipboard_sessions_can_submit_candidates_with_candidate_limits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let mut config: serde_json::Value =
            serde_json::from_str(&configuration(&path, std::process::id())).unwrap();
        config["clipboard"] = true.into();
        let session = PanelSession::parse(&config.to_string()).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            assert_eq!(u32::from_be_bytes(bytes[..4].try_into().unwrap()), 9);
            assert_eq!(&bytes[4..], b"synthetic");
            stream.write_all(&[0]).unwrap();
        });
        assert_eq!(
            session.submit_candidate(&"x".repeat(4097)),
            Err(SessionError::Invalid)
        );
        assert_eq!(session.submit_candidate("synthetic"), Ok(()));
        server.join().unwrap();
    }

    #[test]
    fn non_clipboard_sessions_reject_clipboard_mode_before_connecting() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("missing.sock");
        let session = PanelSession::parse(&configuration(&path, std::process::id())).unwrap();
        assert_eq!(
            session.submit_clipboard("synthetic"),
            Err(SessionError::Rejected)
        );
        assert!(!session.is_used());
    }

    fn configuration(path: &Path, pid: u32) -> String {
        serde_json::json!({"version":1,"path":path,"host_pid":pid,"target_pid":123,"target_started":42.0}).to_string()
    }
    #[test]
    fn frame_is_bounded_and_only_confirmed_native_success_is_success() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let session = PanelSession::parse(&configuration(&path, std::process::id())).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            let expected = "synthetic-😀".as_bytes();
            assert_eq!(
                u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize,
                expected.len()
            );
            assert_eq!(&bytes[4..], expected);
            stream.write_all(&[0]).unwrap();
        });
        assert_eq!(session.submit(""), Err(SessionError::Invalid));
        assert_eq!(
            session.submit(&"x".repeat(4097)),
            Err(SessionError::Invalid)
        );
        assert_eq!(session.submit("synthetic-😀"), Ok(()));
        assert_eq!(
            session.submit("synthetic-duplicate"),
            Err(SessionError::Rejected)
        );
        server.join().unwrap();
    }
    #[test]
    fn wrong_peer_never_receives_text_and_lost_acknowledgements_are_not_retried() {
        for authenticated in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("input.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let pid = std::process::id() + u32::from(!authenticated);
            let session = PanelSession::parse(&configuration(&path, pid)).unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                stream.read_to_end(&mut bytes).unwrap();
                assert_eq!(bytes.is_empty(), !authenticated);
            });
            assert_eq!(
                session.submit("synthetic-text"),
                Err(if authenticated {
                    SessionError::OutcomeUnknown
                } else {
                    SessionError::Unavailable
                })
            );
            if authenticated {
                assert_eq!(
                    session.submit("synthetic-text"),
                    Err(SessionError::Rejected)
                );
            }
            server.join().unwrap();
        }
    }
    #[test]
    fn malformed_configuration_has_only_sanitized_errors() {
        for input in [
            "{}".to_string(),
            "private-invalid-json".to_string(),
            configuration(Path::new("relative.sock"), 123),
            "x".repeat(2049),
        ] {
            assert!(matches!(
                PanelSession::parse(&input),
                Err(SessionError::Invalid)
            ));
        }
    }
}
