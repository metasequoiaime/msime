use super::*;
use std::sync::Mutex;

const URL: &str = "https://dav.example.test/dav";
const USER: &str = "synthetic@example.test";
const PASSWORD: &str = "synthetic-app-password";

/// 记下发出的请求，什么也不做。
#[derive(Default)]
struct Recorder {
    sent: Mutex<Vec<(String, String, Option<String>)>>,
}

impl DavHttp for Arc<Recorder> {
    fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
        self.sent.lock().unwrap().push((
            request.method.to_owned(),
            request.path.to_owned(),
            request.destination.map(str::to_owned),
        ));
        Ok(DavResponse {
            status: 201,
            retry_after: None,
            body: Vec::new(),
        })
    }
}

#[test]
fn the_key_ring_lives_under_its_own_prefix() {
    let recorder = Arc::new(Recorder::default());
    let prefixed = Prefixed {
        inner: Box::new(recorder.clone()),
        prefix: KEYRING,
        roots_pending: AtomicBool::new(false),
        layout: AtomicUsize::new(LAYOUT_UNKNOWN),
    };
    let clock = WallClock;
    let dav = Dav::new(&prefixed, &clock);
    dav.mkcol("msime-clipboard/").unwrap();
    dav.create("msime-clipboard/v1/epoch-000000/e-000000000001.json", b"{}")
        .unwrap();
    let sent = recorder.sent.lock().unwrap().clone();
    assert!(!sent.is_empty());
    for (_, path, destination) in &sent {
        assert!(path.starts_with("msime-dictionary/keyring/"), "{path}");
        assert!(!path.starts_with("msime-dictionary/keyring/msime-clipboard/"));
        if let Some(destination) = destination {
            assert!(destination.starts_with("msime-dictionary/keyring/"));
            assert!(!destination.starts_with("msime-dictionary/keyring/msime-clipboard/"));
        }
    }
    assert_eq!(sent.len(), 3);
}

#[test]
fn the_key_ring_reads_the_published_nested_layout() {
    #[derive(Default)]
    struct LegacyRecorder {
        sent: Mutex<Vec<String>>,
    }

    impl DavHttp for Arc<LegacyRecorder> {
        fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
            self.sent.lock().unwrap().push(request.path.to_owned());
            let status = if request
                .path
                .starts_with("msime-dictionary/keyring/msime-clipboard/")
            {
                207
            } else {
                404
            };
            Ok(DavResponse {
                status,
                retry_after: None,
                body: Vec::new(),
            })
        }
    }

    let recorder = Arc::new(LegacyRecorder::default());
    let prefixed = Prefixed {
        inner: Box::new(recorder.clone()),
        prefix: KEYRING,
        roots_pending: AtomicBool::new(false),
        layout: AtomicUsize::new(LAYOUT_UNKNOWN),
    };
    let request = DavRequest {
        method: "PROPFIND",
        path: "msime-clipboard/v1/",
        depth: Some("1"),
        destination: None,
        body: None,
        limit: 4096,
    };
    assert_eq!(prefixed.send(&request).unwrap().status, 207);
    assert_eq!(
        recorder.sent.lock().unwrap().as_slice(),
        [
            "msime-dictionary/keyring/v1/",
            "msime-dictionary/keyring/msime-clipboard/v1/"
        ]
    );
    assert_eq!(prefixed.layout.load(Ordering::SeqCst), LAYOUT_LEGACY);
}

#[test]
fn temporary_probe_errors_do_not_choose_a_key_ring_layout() {
    struct Responses(Mutex<std::collections::VecDeque<(&'static str, u16)>>);
    impl DavHttp for Arc<Responses> {
        fn send(&self, request: &DavRequest<'_>) -> Result<DavResponse, RelayError> {
            let (path, status) = self.0.lock().unwrap().pop_front().expect("预期的探测请求");
            assert_eq!(request.path, path);
            Ok(DavResponse {
                status,
                retry_after: None,
                body: Vec::new(),
            })
        }
    }
    const FLAT: &str = "msime-dictionary/keyring/v1/";
    const LEGACY: &str = "msime-dictionary/keyring/msime-clipboard/v1/";
    for status in [401, 403, 429, 500, 503] {
        for legacy_probe in [false, true] {
            let mut responses = std::collections::VecDeque::new();
            if legacy_probe {
                responses.push_back((FLAT, 404));
            }
            responses.push_back((if legacy_probe { LEGACY } else { FLAT }, status));
            responses.extend([(FLAT, 404), (LEGACY, 207)]);
            let transport = Arc::new(Responses(Mutex::new(responses)));
            let prefixed = Prefixed {
                inner: Box::new(transport.clone()),
                prefix: KEYRING,
                roots_pending: AtomicBool::new(false),
                layout: AtomicUsize::new(LAYOUT_UNKNOWN),
            };
            let request = DavRequest {
                method: "PROPFIND",
                path: "msime-clipboard/v1/",
                depth: Some("1"),
                destination: None,
                body: None,
                limit: 4096,
            };
            // 鉴权、限流和服务端错误都不能证明目录存在；重试必须重新探测并读到原有密钥串。
            assert_eq!(prefixed.send(&request).unwrap().status, status);
            assert_eq!(prefixed.send(&request).unwrap().status, 207);
            assert!(transport.0.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn errors_keep_the_codes_hosts_already_know() {
    assert_eq!(
        DictionarySyncError::from_relay(RelayError::Unauthorized).code(),
        "dictionary_sync_unauthorized"
    );
    assert_eq!(
        DictionarySyncError::from_relay(RelayError::Rollback).code(),
        "dictionary_sync_rollback"
    );
    assert_eq!(
        DictionarySyncError::from_relay(RelayError::RateLimited).code(),
        "dictionary_sync_rate_limited"
    );
    assert_eq!(
        DictionarySyncError::from_relay(RelayError::Unsupported).code(),
        "dictionary_sync_unsupported"
    );
    assert_eq!(
        DictionarySyncError::from_relay(RelayError::Clipboard(
            crate::cloud::clipboard_e2e::DriverError::from_response(
                &crate::cloud::clipboard_e2e::HttpResponse {
                    status: 404,
                    body: r#"{"error":{"code":"not_found"}}"#.into(),
                }
            )
        ))
        .code(),
        "not_found"
    );
    assert_eq!(
        DictionarySyncError::NotConfigured.code(),
        "dictionary_sync_not_configured"
    );
}

#[test]
fn configuration_follows_the_webdav_rules_and_never_prints_credentials() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(
        DictionarySync::new(
            "http://dav.example.test",
            USER,
            PASSWORD,
            directory.path(),
            ""
        )
        .unwrap_err(),
        DictionarySyncError::NotConfigured
    );
    assert_eq!(
        DictionarySync::new(URL, USER, PASSWORD, "relative", "").unwrap_err(),
        DictionarySyncError::Invalid
    );
    let preferences = DictionarySyncWebDavPreferences::default();
    assert!(
        DictionarySync::from_preferences(&preferences, directory.path(), "")
            .unwrap()
            .is_none()
    );
    let sync = DictionarySync::new(URL, USER, PASSWORD, directory.path(), "").unwrap();
    let debug = format!("{sync:?}");
    assert!(
        !debug.contains(USER) && !debug.contains(PASSWORD),
        "{debug}"
    );
    assert!(valid_device(&"a".repeat(32)).is_ok());
    assert!(valid_device("A").is_err());
}

#[test]
fn the_saved_state_belongs_to_one_folder_and_account() {
    let directory = tempfile::tempdir().unwrap();
    let sync = DictionarySync::new(URL, USER, PASSWORD, directory.path(), "").unwrap();
    let state = SyncState::new(&"a".repeat(32)).unwrap();
    sync.storage().create_directory().unwrap();
    sync.sink().save(&state).unwrap();
    assert_eq!(sync.load_state().unwrap(), Some(state.clone()));
    // 换应用密码不换空间；换账号或文件夹就当作没有。
    let new_password =
        DictionarySync::new(URL, USER, "another-app-password", directory.path(), "").unwrap();
    assert_eq!(new_password.load_state().unwrap(), Some(state));
    let other_user =
        DictionarySync::new(URL, "other@example.test", PASSWORD, directory.path(), "").unwrap();
    assert_eq!(other_user.load_state().unwrap(), None);
    let other_folder = DictionarySync::new(
        "https://dav.example.test/other",
        USER,
        PASSWORD,
        directory.path(),
        "",
    )
    .unwrap();
    assert_eq!(other_folder.load_state().unwrap(), None);
    // 状态文件里没有凭据。
    let saved = std::fs::read_to_string(directory.path().join(STATE_FILE)).unwrap();
    assert!(
        !saved.contains(PASSWORD) && !saved.contains(USER),
        "{saved}"
    );
}

#[test]
fn status_without_a_key_ring_reports_nothing_and_does_not_touch_the_network() {
    let directory = tempfile::tempdir().unwrap();
    let sync = DictionarySync::new(
        "http://127.0.0.1:9/dav",
        USER,
        PASSWORD,
        directory.path(),
        "",
    )
    .unwrap();
    let view = sync.status().unwrap();
    assert_eq!(view.device_id, None);
    assert!(!view.approved && !view.joined);
    assert_eq!(view.last_error, None);
}

/// 密钥串这一层的错误原样交出错误码：本机没有可用的设备身份和驱动给出的结果都是 `KeyRing`，宿主照云剪贴板的码表给出说法。
#[test]
fn key_ring_errors_keep_their_codes() {
    let missing = DictionarySyncError::from_relay(RelayError::IdentityMissing);
    assert!(matches!(missing, DictionarySyncError::KeyRing(_)));
    assert_eq!(missing.code(), crate::clipboard_relay::IDENTITY_MISSING);
    assert_eq!(
        DictionarySyncError::from_relay(RelayError::NotConfigured).code(),
        "dictionary_sync_not_configured"
    );
}

#[test]
fn oversized_local_state_preserves_the_last_readable_file() {
    use crate::cloud::dictionary_sync::{WordKey, WordKind};

    let directory = tempfile::tempdir().unwrap();
    let sync = DictionarySync::new(URL, USER, PASSWORD, directory.path(), "").unwrap();
    sync.storage().create_directory().unwrap();
    let original = SyncState::new(&"a".repeat(32)).unwrap();
    sync.sink().save(&original).unwrap();
    let before = std::fs::read(directory.path().join(STATE_FILE)).unwrap();
    let mut state = serde_json::to_value(&original).unwrap();
    let words: Vec<_> = (0..10_000)
        .map(|index| {
            serde_json::to_value(
                WordKey::new(WordKind::Quick, &format!("q{index}"), &"s".repeat(4096)).unwrap(),
            )
            .unwrap()
        })
        .collect();
    state["applied"] = serde_json::json!(words);
    state["outbox"] = serde_json::Value::Array(
        words
            .into_iter()
            .map(|word| serde_json::json!([word, {"live": true, "hlc": 1024}]))
            .collect(),
    );
    let oversized: SyncState = serde_json::from_value(state).unwrap();
    assert!(serde_json::to_vec(&oversized).unwrap().len() as u64 > MAX_STATE_BYTES);
    assert_eq!(sync.sink().save(&oversized), Err(SyncError::TooLarge));
    assert_eq!(
        std::fs::read(directory.path().join(STATE_FILE)).unwrap(),
        before
    );
    assert_eq!(sync.load_state().unwrap(), Some(original.clone()));
    sync.sink().save(&original).unwrap();
    assert_eq!(sync.load_state().unwrap(), Some(original));
}
