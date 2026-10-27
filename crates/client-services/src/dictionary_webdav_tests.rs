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
fn the_key_ring_lives_under_its_own_prefix_and_its_requests_are_counted() {
    let recorder = Arc::new(Recorder::default());
    let requests = Arc::new(AtomicUsize::new(0));
    let prefixed = Prefixed {
        inner: Box::new(recorder.clone()),
        prefix: KEYRING,
        requests: requests.clone(),
    };
    let clock = WallClock;
    let dav = Dav::new(&prefixed, &clock);
    dav.mkcol("msime-clipboard/").unwrap();
    dav.create("msime-clipboard/v1/epoch-000000/e-000000000001.json", b"{}")
        .unwrap();
    let sent = recorder.sent.lock().unwrap().clone();
    assert!(!sent.is_empty());
    for (_, path, destination) in &sent {
        assert!(
            path.starts_with("msime-dictionary/keyring/msime-clipboard/"),
            "{path}"
        );
        if let Some(destination) = destination {
            assert!(destination.starts_with("msime-dictionary/keyring/msime-clipboard/"));
        }
    }
    assert_eq!(requests.load(Ordering::SeqCst), sent.len());
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
