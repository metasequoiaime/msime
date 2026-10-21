use super::*;

const TOKEN: &str = "synthetic-relay-token-0001";

fn config(url: &str) -> RelayConfig {
    RelayConfig::new(url, TOKEN).unwrap()
}

#[test]
fn relay_addresses_are_https_or_loopback_http_without_query_fragment_or_userinfo() {
    for (url, valid) in [
        ("https://relay.example.test", true),
        ("https://relay.example.test:8443/msime/", true),
        ("http://127.0.0.1:8787", true),
        ("http://localhost/msime", true),
        ("http://[::1]:8787", true),
        ("http://relay.example.test", false),
        ("http://192.168.1.2:8787", false),
        ("https://relay.example.test/?token=x", false),
        ("https://relay.example.test?x", false),
        ("https://relay.example.test/#x", false),
        ("https://user@relay.example.test", false),
        ("https://user:secret@relay.example.test", false),
        ("https:///msime", false),
        ("wss://relay.example.test", false),
        ("relay.example.test", false),
        ("", false),
        ("https://relay.example.test/\u{7}", false),
    ] {
        assert_eq!(valid_relay_url(url), valid, "{url}");
    }
    assert!(!valid_relay_url(&format!(
        "https://relay.example.test/{}",
        "a".repeat(2048)
    )));
}

#[test]
fn relay_tokens_are_bounded_printable_ascii() {
    assert!(valid_relay_token(TOKEN));
    assert!(valid_relay_token(&"a".repeat(16)));
    assert!(valid_relay_token(&"a".repeat(256)));
    for token in [
        String::new(),
        "a".repeat(15),
        "a".repeat(257),
        "synthetic relay token".into(),
        "synthetic-relay-令牌-0001".into(),
        "synthetic-relay-token\n".into(),
    ] {
        assert!(!valid_relay_token(&token), "{token:?}");
    }
    assert_eq!(
        RelayConfig::new("https://relay.example.test", "short"),
        Err(RelayError::NotConfigured)
    );
}

#[test]
fn the_base_address_is_normalized_and_its_path_prefix_kept() {
    assert_eq!(
        config("https://Relay.Example.Test:443/msime/").url(),
        "https://relay.example.test/msime"
    );
    assert_eq!(
        config("https://relay.example.test").url(),
        "https://relay.example.test"
    );
    assert_eq!(
        config("http://127.0.0.1:8787/").url(),
        "http://127.0.0.1:8787"
    );

    let prefixed = config("https://relay.example.test/msime/");
    assert_eq!(
        prefixed
            .request_url("/v1/users/me/clipboard/keys?device_id=0123abcd")
            .unwrap()
            .as_str(),
        "https://relay.example.test/msime/v1/users/me/clipboard/keys?device_id=0123abcd"
    );
    assert_eq!(
        prefixed
            .request_url("/v1/users/me/clipboard")
            .unwrap()
            .as_str(),
        "https://relay.example.test/msime/v1/users/me/clipboard"
    );
    let bare = config("https://relay.example.test:8443");
    assert_eq!(
        bare.request_url("/v1/users/me/clipboard/settings")
            .unwrap()
            .as_str(),
        "https://relay.example.test:8443/v1/users/me/clipboard/settings"
    );
}

#[test]
fn paths_outside_the_clipboard_or_that_could_leave_the_relay_are_refused() {
    let relay = config("https://relay.example.test/msime");
    for path in [
        "/v1/users/me/profile",
        "/v1/users/me/clipboardx",
        "/v1/users/me/clipboard/../../profile",
        "/v1/users/me/clipboard/%2e%2e/profile",
        "/v1/users/me/clipboard/keys#fragment",
        "/v1/users/me/clipboard@evil.example.test",
        "/v1/users/me/clipboard//evil.example.test",
        "/v1/users/me/clipboard\\..\\profile",
        "//evil.example.test/v1/users/me/clipboard",
        "v1/users/me/clipboard",
        "",
    ] {
        assert_eq!(relay.request_url(path), Err(RelayError::Invalid), "{path}");
    }
}

#[test]
fn the_token_never_appears_in_debug_output() {
    let relay = config("https://relay.example.test/msime");
    let text = format!("{relay:?}");
    assert!(text.contains("relay.example.test"), "{text}");
    assert!(!text.contains(TOKEN), "{text}");
}

#[test]
fn preferences_without_an_address_mean_no_relay() {
    let mut preferences = CloudClipboardRelayPreferences::default();
    assert_eq!(RelayConfig::from_preferences(&preferences), Ok(None));
    preferences.url = "https://relay.example.test".into();
    assert_eq!(
        RelayConfig::from_preferences(&preferences),
        Err(RelayError::NotConfigured)
    );
    preferences.token = TOKEN.into();
    assert_eq!(
        RelayConfig::from_preferences(&preferences)
            .unwrap()
            .unwrap()
            .url(),
        "https://relay.example.test"
    );
}

#[test]
fn error_codes_name_the_relay_and_keep_the_driver_s_known_codes() {
    assert_eq!(RelayError::NotConfigured.code(), "relay_not_configured");
    assert_eq!(RelayError::Unauthorized.code(), "relay_unauthorized");
    assert_eq!(RelayError::Unavailable.code(), "relay_unavailable");
    assert_eq!(RelayError::Storage.code(), "relay_storage");
    assert_eq!(RelayError::Invalid.code(), "invalid_cloud_clipboard");
    let driver = |status: u16, code: &str| {
        RelayError::Clipboard(DriverError {
            status,
            code: code.into(),
        })
        .code()
    };
    assert_eq!(
        driver(0, "clipboard_device_pending"),
        "clipboard_device_pending"
    );
    assert_eq!(
        driver(409, "clipboard_key_conflict"),
        "clipboard_key_conflict"
    );
    assert_eq!(driver(409, "clipboard_full"), "clipboard_full");
    assert_eq!(driver(400, "invalid_retention"), "invalid_cloud_clipboard");
    assert_eq!(driver(404, "http_404"), "not_found");
    assert_eq!(driver(500, "http_500"), "relay_unavailable");
    for code in [
        "clipboard_full",
        "relay_unauthorized",
        "relay_not_configured",
        "relay_unavailable",
        "relay_storage",
    ] {
        assert_eq!(crate::account::cloud_clipboard_error_code(code), Some(code));
    }
}

fn clipboard(directory: &std::path::Path, url: &str, token: &str) -> RelayClipboard {
    RelayClipboard::new(RelayConfig::new(url, token).unwrap(), directory, "合成设备").unwrap()
}

#[test]
fn the_keystore_is_private_and_bound_to_its_relay_and_token_space() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("relay");
    let relay = clipboard(&directory, "https://relay.example.test/msime/", TOKEN);
    assert_eq!(relay.load_keystore().unwrap(), None);
    assert!(!directory.exists(), "reading creates nothing");

    relay.save_keystore("v1.synthetic-first").unwrap();
    relay.save_keystore("v1.synthetic-second").unwrap();
    assert_eq!(
        relay.load_keystore().unwrap().as_deref(),
        Some("v1.synthetic-second")
    );
    let path = directory.join(RELAY_KEYSTORE_FILE);
    let stored: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(stored["relay"], "https://relay.example.test/msime");
    assert_eq!(stored["keystore"], "v1.synthetic-second");
    assert!(!stored.to_string().contains(TOKEN));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(&directory).unwrap().permissions().mode() & 0o077,
            0
        );
    }

    // 同一个中继换了写法（大小写、末尾斜杠）仍是同一份。
    let same = clipboard(&directory, "https://RELAY.example.test/msime", TOKEN);
    assert_eq!(
        same.load_keystore().unwrap().as_deref(),
        Some("v1.synthetic-second")
    );
    // 换了中继地址或令牌空间，旧的密钥串不再用，驱动会生成新的设备身份。
    let other_relay = clipboard(&directory, "https://other.example.test/msime", TOKEN);
    assert_eq!(other_relay.load_keystore().unwrap(), None);
    let other_space = clipboard(
        &directory,
        "https://relay.example.test/msime",
        "synthetic-relay-token-0002",
    );
    assert_eq!(other_space.load_keystore().unwrap(), None);

    // 写坏的文件当作没有，而不是让每次操作都失败。
    std::fs::write(&path, b"not json").unwrap();
    assert_eq!(relay.load_keystore().unwrap(), None);
}

#[cfg(unix)]
#[test]
fn a_keystore_readable_by_others_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let relay = clipboard(root.path(), "https://relay.example.test", TOKEN);
    relay.save_keystore("v1.synthetic").unwrap();
    let path = root.path().join(RELAY_KEYSTORE_FILE);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(relay.load_keystore(), Err(RelayError::Storage));
}

#[test]
fn invalid_requests_never_leave_the_device() {
    let root = tempfile::tempdir().unwrap();
    // 端口 9 上没有服务；请求真发出去会得到 `relay_unavailable`，这里每一个都应在发出之前被拒。
    let relay = clipboard(root.path(), "http://127.0.0.1:9", TOKEN);
    assert_eq!(relay.set_retention(3), Err(RelayError::Invalid));
    assert_eq!(
        relay.set_pinned("not-an-id", true),
        Err(RelayError::Invalid)
    );
    assert_eq!(relay.delete(&"A".repeat(64)), Err(RelayError::Invalid));
    assert_eq!(relay.approve("not-a-device"), Err(RelayError::Invalid));
    assert_eq!(relay.add(" "), Err(RelayError::Invalid));
    assert!(RelayClipboard::new(config("https://relay.example.test"), "relative", "合成").is_err());
}

#[cfg(unix)]
#[test]
fn finished_relay_scope_releases_an_inherited_lock() {
    let root = tempfile::tempdir().unwrap();
    let relay = clipboard(root.path(), "https://relay.example.test", TOKEN);
    let held = relay.lock().unwrap();
    let inherited = crate::file_lock::testing::duplicate(&held);
    crate::file_lock::testing::assert_finished_scope_releases_lock(
        held,
        inherited,
        &root.path().join(RELAY_LOCK_FILE),
    );
}

#[test]
fn loopback_relay_bypasses_the_system_proxy() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    const CHILD_ORIGIN: &str = "MSIME_RELAY_PROXY_TEST_ORIGIN";
    if let Ok(origin) = std::env::var(CHILD_ORIGIN) {
        let response = RelayTransport::new(config(&origin))
            .unwrap()
            .send("GET", "/v1/users/me/clipboard", None)
            .unwrap();
        assert_eq!(response.status, 200);
        return;
    }
    // 环境代理只传给独立测试进程，避免与其他并行用例争用进程环境。
    let origin = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin_url = format!("http://{}", origin.local_addr().unwrap());
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    origin.set_nonblocking(true).unwrap();
    proxy.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            for (is_origin, listener) in [(true, &origin), (false, &proxy)] {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(1)))
                            .unwrap();
                        let mut bytes = [0; 4096];
                        let mut length = 0;
                        while !bytes[..length].ends_with(b"\r\n\r\n") {
                            let read = stream.read(&mut bytes[length..]).unwrap();
                            assert!(read > 0 && length + read < bytes.len());
                            length += read;
                        }
                        let request = String::from_utf8_lossy(&bytes[..length]);
                        let authenticated = request.contains(TOKEN);
                        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
                        return (is_origin, authenticated);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => panic!("合成中继监听失败: {error}"),
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        (false, false)
    });
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "clipboard_relay::tests::loopback_relay_bypasses_the_system_proxy",
        ])
        .env(CHILD_ORIGIN, origin_url);
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        command.env(name, &proxy_url);
    }
    for name in ["NO_PROXY", "no_proxy"] {
        command.env(name, "");
    }
    let status = command.status().unwrap();
    let (is_origin, authenticated) = server.join().unwrap();
    assert!(status.success());
    assert!(
        is_origin && authenticated,
        "合成令牌只能发往回环中继，不能进入系统代理"
    );
}
