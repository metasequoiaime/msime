//! 账号适配层的完整密钥流程、登录隔离、刷新与保存时序。

use super::*;
use crate::account::{
    AccountChallenge, AccountProfile, AccountSessionFileLayout, AccountTokens, AccountUser,
    FileAccountSessionStorage, SavedAccountSession, ACCOUNT_SESSION_FILE, LEARNING_KEYSTORE_FILE,
};
use learning_e2e::{HttpRequest, HttpResponse};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Server {
    caller: Option<String>,
    key: Option<String>,
    devices: BTreeMap<String, Value>,
    wraps: BTreeMap<String, Value>,
    requests: Vec<(String, String)>,
    blob_tokens: Vec<String>,
    reject: Option<String>,
    refreshes: usize,
    status_override: Option<u16>,
}

impl Server {
    fn handle(&mut self, request: &HttpRequest, token: &str) -> HttpResponse {
        self.requests.push((request.path.clone(), token.into()));
        if self.reject.as_deref() == Some(token) {
            return response(401, json!({"error": {"code": "unauthorized"}}));
        }
        if let Some(status) = self.status_override {
            return response(
                status,
                json!({"error": {"code": "learning_synthetic_error"}}),
            );
        }
        let body: Value = request
            .body
            .as_deref()
            .map(|body| serde_json::from_str(body).unwrap())
            .unwrap_or(Value::Null);
        let path = request.path.as_str();
        let api = "/v1/users/me/learning";
        match request.method.as_str() {
            "GET" => {
                let id = path.strip_prefix(&format!("{api}?device_id=")).unwrap();
                self.caller = Some(id.into());
                let devices: Vec<_> = self.devices.iter().map(|(id, device)| json!({
                    "device_id": id, "public_key": device["public_key"], "name": device["name"],
                    "created_at": "2026-11-01T00:00:00Z", "last_seen": "2026-11-01T00:00:00Z",
                    "approved": self.wraps.contains_key(id),
                })).collect();
                response(
                    200,
                    json!({"key_id": self.key, "devices": devices, "wrap": self.wraps.get(id), "blobs": []}),
                )
            }
            "PUT" if path.starts_with(&format!("{api}/devices/")) => {
                self.devices
                    .insert(path.rsplit('/').next().unwrap().into(), body);
                response(204, Value::Null)
            }
            "POST" => {
                let caller = self.caller.clone().unwrap();
                if path.ends_with("/rekey") {
                    for id in body["revoke"].as_array().unwrap() {
                        self.devices.remove(id.as_str().unwrap());
                    }
                    self.wraps.clear();
                }
                self.key = body["key_id"].as_str().map(str::to_owned);
                self.wraps.insert(
                    caller.clone(),
                    json!({"wrap": body["wrap"], "approver_device_id": caller}),
                );
                response(204, Value::Null)
            }
            "PUT" if path.starts_with(&format!("{api}/wraps/")) => {
                self.wraps.insert(
                    path.rsplit('/').next().unwrap().into(),
                    json!({"wrap": body["wrap"], "approver_device_id": body["approver_device_id"]}),
                );
                response(204, Value::Null)
            }
            "DELETE" => {
                let id = path.rsplit('/').next().unwrap();
                self.devices.remove(id);
                self.wraps.remove(id);
                if self.devices.is_empty() {
                    self.key = None;
                }
                response(204, Value::Null)
            }
            _ => panic!("unexpected synthetic learning request"),
        }
    }
}

fn response(status: u16, value: Value) -> HttpResponse {
    HttpResponse {
        status,
        body: if value.is_null() {
            String::new()
        } else {
            value.to_string()
        },
    }
}

#[derive(Clone)]
struct Api {
    server: Arc<Mutex<Server>>,
    after_request: Option<Arc<dyn Fn() + Send + Sync>>,
}

fn tokens() -> AccountTokens {
    AccountTokens {
        access_token: "a".repeat(64),
        refresh_token: "b".repeat(64),
        token_type: "Bearer".into(),
        expires_in: 900,
        user: AccountUser {
            id: "synthetic-user".into(),
            display_name: "合成账号".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            email: None,
            avatar_url: None,
        },
    }
}

impl AccountApi for Api {
    fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
        Ok(Default::default())
    }
    fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
        Ok(tokens())
    }
    fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
        self.server.lock().unwrap().refreshes += 1;
        let mut tokens = tokens();
        tokens.access_token = "c".repeat(64);
        Ok(tokens)
    }
    fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
        Err(AccountError::Unavailable)
    }
    fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
        Ok(())
    }
    fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
        Ok(())
    }
    fn delete_account(&self, _: &str) -> Result<(), AccountError> {
        Ok(())
    }
    fn learning_e2e(
        &self,
        request: &HttpRequest,
        token: &str,
    ) -> Result<HttpResponse, AccountError> {
        let result = self.server.lock().unwrap().handle(request, token);
        if let Some(after) = &self.after_request {
            after();
        }
        Ok(result)
    }
    fn learning_blob(
        &self,
        _: &BlobRequest<'_>,
        token: &str,
    ) -> Result<BlobResponse, AccountError> {
        let mut server = self.server.lock().unwrap();
        server.blob_tokens.push(token.into());
        let status = if server.reject.as_deref() == Some(token) {
            401
        } else {
            server.status_override.unwrap_or(200)
        };
        drop(server);
        if let Some(after) = &self.after_request {
            after();
        }
        Ok(BlobResponse {
            status,
            body: vec![0, 0xff, 2],
            retry_after_seconds: Some(17),
        })
    }
}

fn store(path: &std::path::Path) -> FileAccountSessionStorage {
    FileAccountSessionStorage::new(path, AccountSessionFileLayout::Native)
}

fn session(
    server: &Arc<Mutex<Server>>,
    storage: &FileAccountSessionStorage,
) -> BackendAccountSession<Api, FileAccountSessionStorage> {
    BackendAccountSession::new(
        Api {
            server: server.clone(),
            after_request: None,
        },
        storage.clone(),
    )
}

fn sign_in(session: &BackendAccountSession<Api, FileAccountSessionStorage>) -> AccountLoginBinding {
    session.sign_in("synthetic-challenge", "123456").unwrap();
    session.login_binding().unwrap().unwrap()
}

fn status(session: &BackendAccountSession<Api, FileAccountSessionStorage>) -> Value {
    session
        .learning_operation(Operation::Status, "合成设备", None)
        .unwrap()
}

#[test]
fn two_devices_create_approve_confirm_reset_and_disable_with_separate_clipboard_keys() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Mutex::new(Server::default()));
    let storage_a = store(&root.path().join("a"));
    let storage_b = store(&root.path().join("b"));
    let a = session(&server, &storage_a);
    let b = session(&server, &storage_b);
    sign_in(&a);
    sign_in(&b);
    storage_a
        .save_clipboard_keystore("synthetic-clipboard")
        .unwrap();
    assert_eq!(status(&a)["status"], "ready");
    let pending = status(&b);
    assert_eq!(pending["status"], "pending");
    let device_b = pending["device_id"].as_str().unwrap().to_owned();
    let approved = a
        .learning_operation(
            Operation::Approve {
                device_id: device_b.clone(),
            },
            "合成 A",
            None,
        )
        .unwrap();
    let confirm = status(&b);
    assert_eq!(confirm["status"], "confirm");
    assert_eq!(approved["approved"]["code"], confirm["confirm"]["code"]);
    let confirmed = b
        .learning_operation(
            Operation::Confirm {
                key_id: confirm["confirm"]["key_id"].as_str().unwrap().into(),
                approver_device_id: confirm["confirm"]["approver_device_id"]
                    .as_str()
                    .unwrap()
                    .into(),
            },
            "合成 B",
            None,
        )
        .unwrap();
    assert_eq!(confirmed["status"], "ready");
    let saved_a = key_for_login(
        storage_a.load_learning_keystore().unwrap(),
        &a.login_binding().unwrap().unwrap(),
    )
    .unwrap()
    .unwrap();
    let key_a = Keystore::parse(&saved_a)
        .unwrap()
        .ready_key_id()
        .unwrap()
        .to_owned();
    assert_eq!(
        a.learning_operation(Operation::Reset, "合成 A", None)
            .unwrap()["status"],
        "ready"
    );
    let saved = key_for_login(
        storage_a.load_learning_keystore().unwrap(),
        &a.login_binding().unwrap().unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_ne!(
        Keystore::parse(&saved).unwrap().ready_key_id(),
        Some(key_a.as_str())
    );
    assert_eq!(
        b.learning_operation(Operation::Disable, "合成 B", None)
            .unwrap()["status"],
        "disabled"
    );
    assert!(!server.lock().unwrap().devices.contains_key(&device_b));
    assert_eq!(
        storage_a.load_clipboard_keystore().unwrap().as_deref(),
        Some("synthetic-clipboard")
    );
}

#[test]
fn same_account_relogin_cancels_old_work_before_network_and_uses_a_new_key_identity() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Mutex::new(Server::default()));
    let storage = store(root.path());
    let settings = session(&server, &storage);
    let keyboard = session(&server, &storage);
    let old_login = sign_in(&settings);
    let old_device = status(&keyboard)["device_id"].clone();
    let new_login = sign_in(&settings);
    let before = server.lock().unwrap().requests.len();
    let result = keyboard.learning_operation(Operation::Status, "合成旧任务", Some(&old_login));
    assert!(matches!(
        result,
        Err(AccountLearningError::Account(AccountError::Cancelled))
    ));
    let request = BlobRequest::Metadata {
        device_id: "0123456789abcdef0123456789abcdef",
    };
    assert!(matches!(
        keyboard.learning_blob_for_login(&request, &old_login),
        Err(AccountError::Cancelled)
    ));
    assert_eq!(server.lock().unwrap().requests.len(), before);
    assert!(server.lock().unwrap().blob_tokens.is_empty());
    assert_ne!(old_device, status(&keyboard)["device_id"]);
    assert_eq!(keyboard.login_binding().unwrap(), Some(new_login));
}

#[test]
fn control_and_blob_retry_a_401_once_without_changing_login() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Mutex::new(Server::default()));
    let storage = store(root.path());
    let account = session(&server, &storage);
    let login = sign_in(&account);
    server.lock().unwrap().reject = Some(tokens().access_token);
    assert_eq!(status(&account)["status"], "ready");
    assert_eq!(server.lock().unwrap().refreshes, 1);
    assert_eq!(account.login_binding().unwrap(), Some(login.clone()));
    // 新登录让二进制路径也从被拒令牌起步。
    let login = sign_in(&account);
    let response = account
        .learning_blob_for_login(
            &BlobRequest::Metadata {
                device_id: "0123456789abcdef0123456789abcdef",
            },
            &login,
        )
        .unwrap();
    assert_eq!(
        (response.status, response.body, response.retry_after_seconds),
        (200, vec![0, 0xff, 2], Some(17))
    );
    assert_eq!(
        server.lock().unwrap().blob_tokens,
        ["a".repeat(64), "c".repeat(64)]
    );
    assert_eq!(server.lock().unwrap().refreshes, 2);
}

#[test]
fn a_second_401_is_returned_without_an_infinite_refresh_loop() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Mutex::new(Server::default()));
    let storage = store(root.path());
    let account = session(&server, &storage);
    let login = sign_in(&account);
    server.lock().unwrap().status_override = Some(401);
    assert_eq!(
        account
            .learning_operation(Operation::Status, "合成设备", Some(&login))
            .unwrap_err()
            .code(),
        "learning_synthetic_error"
    );
    assert_eq!(server.lock().unwrap().requests.len(), 2);
    assert_eq!(server.lock().unwrap().refreshes, 1);
    assert_eq!(
        account
            .learning_blob_for_login(
                &BlobRequest::Metadata {
                    device_id: "0123456789abcdef0123456789abcdef"
                },
                &login
            )
            .unwrap()
            .status,
        401
    );
    assert_eq!(server.lock().unwrap().blob_tokens.len(), 2);
    assert_eq!(server.lock().unwrap().refreshes, 2);
}

#[test]
fn switched_login_discards_control_and_blob_responses_without_writing_old_keys() {
    for (blob, reject) in [(false, false), (false, true), (true, false), (true, true)] {
        let root = tempfile::tempdir().unwrap();
        let server = Arc::new(Mutex::new(Server::default()));
        let storage = store(root.path());
        let initial = session(&server, &storage);
        let login = sign_in(&initial);
        if reject {
            server.lock().unwrap().reject = Some(tokens().access_token);
        }
        let replacement = Arc::new(session(&server, &storage));
        let account = BackendAccountSession::new(
            Api {
                server: server.clone(),
                after_request: Some(Arc::new(move || {
                    sign_in(&replacement);
                })),
            },
            storage.clone(),
        );
        if blob {
            assert!(matches!(
                account.learning_blob_for_login(
                    &BlobRequest::Metadata {
                        device_id: "0123456789abcdef0123456789abcdef"
                    },
                    &login
                ),
                Err(AccountError::Cancelled)
            ));
            assert_eq!(storage.load_learning_keystore().unwrap(), None);
        } else {
            assert!(matches!(
                account.learning_operation(Operation::Status, "合成设备", Some(&login)),
                Err(AccountLearningError::Account(AccountError::Cancelled))
            ));
            // 发请求前已保存的旧身份不能在新登录下使用。
            let current = initial.login_binding().unwrap().unwrap();
            assert_eq!(
                key_for_login(storage.load_learning_keystore().unwrap(), &current).unwrap(),
                None
            );
            assert_eq!(server.lock().unwrap().requests.len(), 1);
        }
        assert_eq!(server.lock().unwrap().refreshes, 0);
    }
}

#[test]
fn failure_to_persist_identity_prevents_any_request() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Mutex::new(Server::default()));
    let storage = store(root.path());
    struct FailingStore(FileAccountSessionStorage);
    impl AccountSessionStorage for FailingStore {
        fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
            self.0.load()
        }
        fn save(&self, saved: &SavedAccountSession) -> Result<(), AccountError> {
            self.0.save(saved)
        }
        fn clear(&self) -> Result<(), AccountError> {
            self.0.clear()
        }
        fn load_learning_keystore(&self) -> Result<Option<String>, AccountError> {
            self.0.load_learning_keystore()
        }
        fn save_learning_keystore(&self, _: &str) -> Result<(), AccountError> {
            Err(AccountError::Storage)
        }
    }
    let account = BackendAccountSession::new(
        Api {
            server: server.clone(),
            after_request: None,
        },
        FailingStore(storage),
    );
    account.sign_in("synthetic-challenge", "123456").unwrap();
    assert!(matches!(
        account.learning_operation(Operation::Status, "合成设备", None),
        Err(AccountLearningError::Account(AccountError::Storage))
    ));
    assert!(server.lock().unwrap().requests.is_empty());
    assert!(root.path().join(ACCOUNT_SESSION_FILE).exists());
    assert!(!root.path().join(LEARNING_KEYSTORE_FILE).exists());
}

#[test]
fn logout_and_other_account_cancel_old_bindings_without_learning_network() {
    for other_account in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let server = Arc::new(Mutex::new(Server::default()));
        let storage = store(root.path());
        let account = session(&server, &storage);
        let login = sign_in(&account);
        if other_account {
            let mut saved = storage.load().unwrap().unwrap();
            saved.tokens.user.id = "synthetic-other-user".into();
            saved.session_id = Some(uuid::Uuid::new_v4());
            storage.save(&saved).unwrap();
        } else {
            storage.clear().unwrap();
        }
        assert!(matches!(
            account.learning_operation(Operation::Status, "合成旧任务", Some(&login)),
            Err(AccountLearningError::Account(AccountError::Cancelled))
        ));
        assert!(matches!(
            account.learning_blob_for_login(
                &BlobRequest::Metadata {
                    device_id: "0123456789abcdef0123456789abcdef"
                },
                &login
            ),
            Err(AccountError::Cancelled)
        ));
        let server = server.lock().unwrap();
        assert!(server.requests.is_empty());
        assert!(server.blob_tokens.is_empty());
        assert_eq!(server.refreshes, 0);
    }
}

#[test]
fn persisted_key_conflict_adopts_the_existing_identity_before_sending() {
    let root = tempfile::tempdir().unwrap();
    let server = Arc::new(Mutex::new(Server::default()));
    let storage = store(root.path());
    let account = session(&server, &storage);
    let login = sign_in(&account);
    let winning = match learning_e2e::start(Operation::Status, None, "合成并发进程").unwrap()
    {
        learning_e2e::Step::Request { keystore, .. } => keystore,
        _ => panic!("expected identity"),
    };
    let winner_id = Keystore::parse(&winning).unwrap().device_id();
    let encoded = serde_json::to_string(&BoundKeystore {
        login,
        keystore: winning,
    })
    .unwrap();
    // 首次读取为空，但进入原子保存段时另一进程已经保存。
    #[derive(Clone)]
    struct RacingStore {
        inner: FileAccountSessionStorage,
        encoded: String,
        reads: Arc<Mutex<usize>>,
    }
    impl AccountSessionStorage for RacingStore {
        fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
            self.inner.load()
        }
        fn save(&self, saved: &SavedAccountSession) -> Result<(), AccountError> {
            self.inner.save(saved)
        }
        fn clear(&self) -> Result<(), AccountError> {
            self.inner.clear()
        }
        fn shared_across_processes(&self) -> bool {
            true
        }
        fn with_refresh_lock<T>(
            &self,
            body: impl FnOnce() -> Result<T, AccountError>,
        ) -> Result<T, AccountError> {
            self.inner.with_refresh_lock(body)
        }
        fn load_learning_keystore(&self) -> Result<Option<String>, AccountError> {
            let mut reads = self.reads.lock().unwrap();
            *reads += 1;
            if *reads == 2 {
                self.inner.save_learning_keystore(&self.encoded)?;
            }
            self.inner.load_learning_keystore()
        }
        fn save_learning_keystore(&self, value: &str) -> Result<(), AccountError> {
            self.inner.save_learning_keystore(value)
        }
    }
    let racing = BackendAccountSession::new(
        Api {
            server: server.clone(),
            after_request: None,
        },
        RacingStore {
            inner: storage,
            encoded,
            reads: Arc::new(Mutex::new(0)),
        },
    );
    assert_eq!(
        racing
            .learning_operation(Operation::Status, "合成并发进程", None)
            .unwrap()["device_id"],
        winner_id
    );
    assert!(server.lock().unwrap().requests[0].0.ends_with(&winner_id));
}
