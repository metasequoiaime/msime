//! 通过真实 Unix socket 验证独立 provider 的登录握手与绑定字段转交。

use super::*;
use std::ffi::CStr;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

#[test]
fn independent_provider_binding_and_bound_action_cross_the_native_ffi() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = directory
        .path()
        .canonicalize()
        .unwrap()
        .join("clipboard.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let login = json!({"user_id": "synthetic-provider", "session_id": "synthetic-login"});
    let expected_login = login.clone();
    let server = std::thread::spawn(move || {
        for operation in ["binding", "list"] {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "provider handshake timed out");
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("provider accept failed: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            let request: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["version"], 1);
            assert_eq!(request["kind"], "cloud_clipboard");
            assert_eq!(request["request"]["operation"], operation);
            if operation == "list" {
                assert_eq!(request["request"]["expected_login"], expected_login);
            }
            writeln!(stream, "{expected_login}").unwrap();
        }
    });
    for action in [
        json!({"operation": "binding"}),
        json!({"operation": "list", "expected_login": login}),
    ] {
        let encoded = action.to_string();
        let path = socket.to_str().unwrap();
        // SAFETY: 缓冲区在调用期间有效，返回值只读取一次并按契约释放。
        let reply: Value = unsafe {
            let raw = msime_client_cloud_clipboard_provider_request(
                encoded.as_ptr(),
                encoded.len(),
                path.as_ptr(),
                path.len(),
            );
            let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
            msime_client_string_free(raw);
            serde_json::from_str(&text).unwrap()
        };
        assert_eq!(reply["ok"], true, "{reply}");
        assert_eq!(reply["value"], login);
    }
    server.join().unwrap();
}
