//! 从账号 C 入口执行真实学习轮次与离线合并，不以设备操作替代共享行为测试。

use super::*;
use msime_client_core::cloud::learning_e2e::{self, Keystore, Persisted};
use msime_engine::{LearningSyncRecord, LearningSyncState, RuntimePaths};

mod contexts;
mod server;
use server::Server;

struct Harness {
    _root: tempfile::TempDir,
    source: Value,
    storage: FileAccountSessionStorage,
    options: Value,
    login: Value,
    paths: RuntimePaths,
    content: String,
    server: Server,
}

impl Harness {
    fn new() -> Self {
        let (root, source, storage) = fixture();
        let content = Edition::by_id("vietnamese")
            .unwrap()
            .resource_set()
            .unwrap()
            .generation()
            .unwrap();
        let paths = RuntimePaths {
            resources: root.path().join("resources"),
            user_data: root.path().join("user"),
            cache: root.path().join("cache"),
            dictionaries: root.path().join("user/dictionaries").join(&content),
        };
        for path in [
            &paths.resources,
            &paths.user_data,
            &paths.cache,
            &paths.dictionaries,
        ] {
            std::fs::create_dir_all(path).unwrap();
        }
        msime_engine::ensure_english_schema(&paths.dictionary("msime-english.db")).unwrap();
        let options = json!({"api_version": 1, "edition": "vietnamese",
            "preferences": Preferences::default(), "preferences_directory": root.path(),
            "resources": paths.resources, "user_data": paths.user_data,
            "cache": paths.cache, "dictionaries": paths.dictionaries});
        let server = Server::new();
        let login = binding(&source);
        Self {
            _root: root,
            source,
            storage,
            options,
            login,
            paths,
            content,
            server,
        }
    }

    fn call(&self, operation: &str) -> Value {
        let mut action =
            json!({"operation": operation, "login": self.login, "options": self.options});
        if operation == "learning_sync_once" {
            action["device_name"] = json!("合成 A");
        }
        call(&self.source, action)
    }

    fn ready(&self) {
        let response = call(
            &self.source,
            action(self.login.clone(), json!({"op": "status"})),
        );
        assert_eq!(response["value"]["status"], "ready", "{response}");
    }

    fn keys(&self) -> Keystore {
        let value: Value =
            serde_json::from_str(&self.storage.load_learning_keystore().unwrap().unwrap()).unwrap();
        Keystore::parse(value["keystore"].as_str().unwrap()).unwrap()
    }

    fn remote(&self, generation: u64) -> String {
        self.ready();
        let mut saved = None;
        let mut run = |operation| {
            let previous = saved.clone();
            learning_e2e::run::<()>(
                operation,
                previous.as_deref(),
                "合成 B",
                |request| Ok(self.server.state.lock().unwrap().control(request)),
                |value| {
                    saved = Some(value.to_owned());
                    Ok(Persisted::Saved)
                },
                || (),
            )
            .unwrap()
            .unwrap()
        };
        let pending = run(Operation::Status);
        let id = pending["device_id"].as_str().unwrap().to_owned();
        assert_eq!(
            call(
                &self.source,
                action(
                    self.login.clone(),
                    json!({"op": "approve", "device_id": id})
                )
            )["ok"],
            true
        );
        let confirm = run(Operation::Status);
        run(Operation::Confirm {
            key_id: confirm["confirm"]["key_id"].as_str().unwrap().to_owned(),
            approver_device_id: confirm["confirm"]["approver_device_id"]
                .as_str()
                .unwrap()
                .to_owned(),
        });
        let keys = Keystore::parse(saved.as_deref().unwrap()).unwrap();
        let mut stream = Vec::new();
        msime_engine::write_learning_sync(&state(9, 100), &self.content, &mut stream).unwrap();
        self.server.state.lock().unwrap().blobs.insert(
            id.clone(),
            (generation, keys.seal_blob(generation, &stream).unwrap()),
        );
        id
    }

    fn exported(&self) -> LearningSyncState {
        let mut state = LearningSyncState::default();
        msime_engine::stream_learning_sync_state(&self.paths, &mut |record| {
            state.insert(record.clone()).unwrap();
            true
        })
        .unwrap();
        state
    }
}

fn state(weight: u64, updated_at: u64) -> LearningSyncState {
    let mut state = LearningSyncState::default();
    state
        .insert(LearningSyncRecord::Weight {
            dictionary: "english".into(),
            key: "sample".into(),
            value: "Sample".into(),
            display: "Sample".into(),
            weight,
            updated_at,
        })
        .unwrap();
    state
}

#[test]
fn c_round_uploads_authenticated_stream_then_downloads_and_merges_real_sqlite() {
    let harness = Harness::new();
    let remote = harness.remote(3);
    msime_engine::merge_learning_sync_state(&harness.paths, false, &state(4, 10), 0).unwrap();
    let response = harness.call("learning_sync_once");
    assert_eq!(response["ok"], true, "{response}");
    let report = &response["value"];
    assert_eq!(report["uploaded"], 1);
    assert_eq!(
        report["downloads"][0],
        json!({"device_id": remote, "generation": 3,
        "outcome": "applied", "dictionary_mismatch": false})
    );
    assert_eq!(harness.exported(), state(9, 100));
    let server = harness.server.state.lock().unwrap();
    let opened = harness
        .keys()
        .open_blob(&harness.keys().device_id(), &server.uploads[0])
        .unwrap();
    let stream = msime_engine::read_learning_sync(std::io::Cursor::new(opened.stream)).unwrap();
    assert_eq!(stream.content_id, harness.content);
    assert_eq!(stream.state, state(4, 10));
    assert!(server.uploads[0].starts_with(b"MSL1"));
    let upload = server
        .trace
        .iter()
        .position(|path| path.starts_with("PUT /v1/users/me/learning/blobs/"))
        .unwrap();
    let download = server
        .trace
        .iter()
        .position(|path| path == &format!("GET /v1/users/me/learning/blobs/{remote}"))
        .unwrap();
    assert!(upload < download);
    drop(server);
    assert_eq!(harness.call("learning_sync_once")["value"]["uploaded"], 2);
    let unchanged = harness.call("learning_sync_once");
    assert_eq!(unchanged["value"]["upload_unchanged"], true, "{unchanged}");
    for private in [
        "sample",
        "Sample",
        "keystore",
        "synthetic-sensitive-error-body",
    ] {
        assert!(!response.to_string().contains(private));
    }
}

#[test]
fn c_pending_merge_reauthenticates_ciphertext_offline_with_expired_token() {
    let harness = Harness::new();
    harness.remote(2);
    harness.server.state.lock().unwrap().lease_on_download = Some(harness.paths.clone());
    let queued = harness.call("learning_sync_once");
    assert_eq!(
        queued["value"]["downloads"][0]["outcome"], "pending",
        "{queued}"
    );
    assert_eq!(harness.exported().records().len(), 0);
    harness.server.state.lock().unwrap().lease.take();
    let mut expired = harness.storage.load().unwrap().unwrap();
    expired.expires_at_unix_ms = 0;
    harness.storage.save(&expired).unwrap();
    let count = harness.server.state.lock().unwrap().trace.len();
    let merged = harness.call("learning_merge_pending");
    assert_eq!(
        merged["value"]["pending"][0]["outcome"], "applied",
        "{merged}"
    );
    assert_eq!(harness.exported(), state(9, 100));
    assert_eq!(
        harness.call("learning_merge_pending")["value"]["pending"],
        json!([])
    );
    assert_eq!(harness.server.state.lock().unwrap().trace.len(), count);
}

#[test]
fn c_round_reports_http_status_and_retry_without_raw_server_body() {
    for status in [413, 429] {
        let harness = Harness::new();
        harness.ready();
        harness.server.state.lock().unwrap().upload_status = Some(status);
        let response = harness.call("learning_sync_once");
        if status == 429 {
            assert_eq!(
                response,
                json!({"ok": false, "error": "learning_http",
                "http_status": status, "retry_after_seconds": 17})
            );
        } else {
            assert_eq!(response["ok"], true, "{response}");
            assert_eq!(
                response["value"]["upload_error"],
                json!({"error": "learning_http",
                "http_status": status, "retry_after_seconds": 17})
            );
        }
        assert!(!response
            .to_string()
            .contains("synthetic-sensitive-error-body"));
    }
}

#[test]
fn c_round_rejects_busy_and_stale_login_without_changing_learning_data() {
    let harness = Harness::new();
    harness.ready();
    let _lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &harness.paths.user_data,
        &harness.paths.dictionaries,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        harness.call("learning_sync_once"),
        json!({"ok": false, "error": "learning_busy"})
    );
    let mut saved = harness.storage.load().unwrap().unwrap();
    saved.session_id = Some(msime_client_core::uuid::Uuid::new_v4());
    harness.storage.save(&saved).unwrap();
    let count = harness.server.state.lock().unwrap().trace.len();
    for operation in ["learning_sync_once", "learning_merge_pending"] {
        assert_eq!(
            harness.call(operation),
            json!({"ok": false, "error": "account_cancelled"})
        );
    }
    assert_eq!(harness.server.state.lock().unwrap().trace.len(), count);
    assert_eq!(harness.exported().records().len(), 0);
}

#[test]
fn c_data_requests_reject_invalid_options_and_do_not_create_learning_identity() {
    let harness = Harness::new();
    let mut cases = Vec::new();
    for field in [
        "resources",
        "user_data",
        "cache",
        "dictionaries",
        "preferences_directory",
    ] {
        let mut options = harness.options.clone();
        options[field] = json!("relative");
        cases.push(options);
    }
    let mut missing = harness.options.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("preferences_directory");
    cases.push(missing);
    let mut newer = harness.options.clone();
    newer["api_version"] = json!(2);
    cases.push(newer);
    let mut stale = harness.options.clone();
    stale["dictionaries"] = json!(harness.paths.user_data.join("dictionaries/old-generation"));
    cases.push(stale);
    let mut unknown = harness.options.clone();
    unknown["unexpected"] = json!(true);
    cases.push(unknown);
    for options in cases {
        for operation in ["learning_sync_once", "learning_merge_pending"] {
            let mut request =
                json!({"operation": operation, "login": harness.login, "options": options});
            if operation == "learning_sync_once" {
                request["device_name"] = json!("合成 A");
            }
            assert_eq!(
                call(&harness.source, request),
                json!({"ok": false, "error": "learning_invalid_options"})
            );
        }
    }
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}
