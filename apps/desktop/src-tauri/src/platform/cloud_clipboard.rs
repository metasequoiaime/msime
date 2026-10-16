//! Cloud clipboard requests over the account session, for every Tauri host that holds one.
//!
//! iOS 和 Android 只经账号会话答这个页面，共享偏好里配置了自建中继时一律拒绝（[`require_no_relay`]）；三个桌面外壳在 provider socket、自建中继和输入法原生会话都没有答复时才退回账号会话。分派本身在 `msime_host_api::cloud_clipboard::account_request`，Windows 原生设置窗口也经 C ABI 调用它；这里只是带上本目标 `AccountState` 持有的会话，把它挪出异步运行时。
//!
//! 列表、上传和设备管理（`devices`、`approve`、`reject`、`remove`、`reset`）经端到端加密驱动，结果形状见 `docs/cloud-clipboard-e2e.md` 的「共享面板的动作」；驱动给出的错误码（例如 `clipboard_device_pending`）原样作为 `CommandError` 的 `code` 交给面板。

use msime_client_core::preferences::PreferencesStore;
use msime_client_services::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
use msime_client_services::clipboard_relay::RelayConfig;
use serde_json::Value;
use std::sync::Arc;

/// 共享偏好里配置的自建中继（`docs/cloud-clipboard-relay.md`）；没有配置时为 `None`。偏好读不了时是 `storage`，配置不合规时是 `relay_not_configured`。不联网。
pub(crate) fn configured_relay(
    store: &PreferencesStore,
) -> Result<Option<RelayConfig>, crate::CommandError> {
    let snapshot = store
        .load()
        .map_err(|_| crate::CommandError { code: "storage" })?;
    RelayConfig::from_preferences(&snapshot.preferences.cloud_clipboard_relay)
        .map_err(|error| crate::CommandError { code: error.code() })
}

/// 只能经水杉账号答云剪贴板的外壳（iOS、Android 的 Tauri 工程）在发出每个请求之前调用：共享偏好里配置了自建中继时拒绝，错误码 `relay_unsupported`；读不出偏好或中继配置不合规时同样拒绝（`storage`、`relay_not_configured`）。用户选了中继，内容就不能发往水杉账号的服务。这两个外壳不经中继答：Android 的中继由原生宿主跑，密钥串经 AndroidKeyStore 加密保存，这个进程里没有那份密钥串，也不该另存一份明文的。不联网。
#[cfg(any(target_os = "android", target_os = "ios", test))]
pub(crate) fn require_no_relay(store: &PreferencesStore) -> Result<(), crate::CommandError> {
    match configured_relay(store)? {
        Some(_) => Err(crate::CommandError {
            code: "relay_unsupported",
        }),
        None => Ok(()),
    }
}

/// 经自建中继答一个云剪贴板动作，`directory` 是本机放中继密钥串的目录。会联网，只在阻塞线程上调用。
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) fn relay_cloud_clipboard_request(
    config: RelayConfig,
    directory: &std::path::Path,
    action: &Value,
) -> Result<Value, crate::CommandError> {
    msime_host_api::cloud_clipboard::relay_request(
        config,
        directory,
        &msime_client_services::account::cloud_clipboard_device_name(),
        action,
    )
    .map_err(|code| crate::CommandError { code })
}

pub(crate) async fn cloud_clipboard_request<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    action: Value,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(session);
    tauri::async_runtime::spawn_blocking(move || {
        msime_host_api::cloud_clipboard::account_request(&session, &action)
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "account_unavailable",
    })?
    .map_err(|code| crate::CommandError { code })
}

#[cfg(test)]
mod tests {
    use super::cloud_clipboard_request;
    use msime_client_core::cloud::clipboard_e2e::{HttpRequest, HttpResponse};
    use msime_client_services::account::{
        AccountApi, AccountChallenge, AccountError, AccountProfile, AccountSessionStorage,
        AccountTokens, AccountUser, BackendAccountSession, SavedAccountSession,
    };
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    /// The session validates stored tokens as 64 lowercase hex digits, so the synthetic ones have that shape.
    const ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    /// 合成的现有剪贴板密钥编号：账号已由另一台设备开启，本机登记后等待批准。
    const OTHER_KEY_ID: &str = "0123456789abcdef0123456789abcdef";

    /// 云剪贴板服务端的最小合成实现：登记设备、建钥和删除登记，其余经驱动的请求都不应出现。
    #[derive(Default)]
    struct FakeServer {
        enabled: bool,
        key_id: Option<String>,
        devices: Vec<(String, String)>,
        wraps: Vec<String>,
    }

    impl FakeServer {
        fn handle(&mut self, request: &HttpRequest) -> HttpResponse {
            let reply = |status: u16, body: Value| HttpResponse {
                status,
                body: body.to_string(),
            };
            let body: Value = request
                .body
                .as_deref()
                .map(|body| serde_json::from_str(body).unwrap())
                .unwrap_or(Value::Null);
            let path = request.path.as_str();
            match request.method.as_str() {
                "GET" if path.starts_with("/v1/users/me/clipboard/keys?device_id=") => reply(
                    200,
                    json!({
                        "enabled": self.enabled,
                        "key_id": self.key_id,
                        "wrap": null,
                        "devices": self.devices.iter().map(|(id, key)| json!({
                            "device_id": id,
                            "public_key": key,
                            "name": "合成设备",
                            "created_at": "2026-10-11T08:00:00Z",
                            "approved": self.wraps.contains(id),
                        })).collect::<Vec<_>>(),
                    }),
                ),
                "PUT" if path.starts_with("/v1/users/me/clipboard/devices/") => {
                    let id = path.rsplit('/').next().unwrap().to_owned();
                    self.devices
                        .push((id, body["public_key"].as_str().unwrap().to_owned()));
                    reply(204, Value::Null)
                }
                "DELETE" if path.starts_with("/v1/users/me/clipboard/devices/") => reply(
                    404,
                    json!({ "error": { "code": "not_found", "message": "not found" } }),
                ),
                "POST" if path == "/v1/users/me/clipboard/keys" => {
                    self.key_id = body["key_id"].as_str().map(str::to_owned);
                    self.wraps = body["wraps"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|wrap| wrap["device_id"].as_str().unwrap().to_owned())
                        .collect();
                    reply(204, Value::Null)
                }
                _ => panic!("unexpected request {} {path}", request.method),
            }
        }
    }

    /// A backend that records the clipboard calls it receives. Every call outside the clipboard is a test failure.
    struct FakeApi {
        calls: Arc<Mutex<Vec<String>>>,
        server: Arc<Mutex<FakeServer>>,
    }

    impl FakeApi {
        fn record(&self, call: String, token: &str) {
            assert_eq!(token, ACCESS_TOKEN);
            self.calls.lock().unwrap().push(call);
        }
    }

    impl AccountApi for FakeApi {
        fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
            unreachable!("providers")
        }
        fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
            unreachable!("challenge")
        }
        fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
            unreachable!("login")
        }
        fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
            unreachable!("refresh")
        }
        fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
            unreachable!("profile")
        }
        fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
            unreachable!("rename")
        }
        fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
            unreachable!("logout")
        }
        fn delete_account(&self, _: &str) -> Result<(), AccountError> {
            unreachable!("delete_account")
        }
        fn clipboard_e2e(
            &self,
            request: &HttpRequest,
            token: &str,
        ) -> Result<HttpResponse, AccountError> {
            self.record(
                format!(
                    "{} {}",
                    request.method,
                    request.path.split('?').next().unwrap()
                ),
                token,
            );
            Ok(self.server.lock().unwrap().handle(request))
        }
        fn set_clipboard_enabled(&self, enabled: bool, token: &str) -> Result<(), AccountError> {
            self.record(format!("set_enabled:{enabled}"), token);
            Ok(())
        }
        fn delete_clipboard(&self, id: Option<&str>, token: &str) -> Result<(), AccountError> {
            self.record(format!("delete:{}", id.unwrap_or("*")), token);
            Ok(())
        }
    }

    struct MemoryStorage {
        session: Option<SavedAccountSession>,
        keystore: Arc<Mutex<Option<String>>>,
    }

    impl AccountSessionStorage for MemoryStorage {
        fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
            Ok(self.session.clone())
        }
        fn save(&self, _: &SavedAccountSession) -> Result<(), AccountError> {
            Ok(())
        }
        fn clear(&self) -> Result<(), AccountError> {
            Ok(())
        }
        fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
            Ok(self.keystore.lock().unwrap().clone())
        }
        fn save_clipboard_keystore(&self, value: &str) -> Result<(), AccountError> {
            *self.keystore.lock().unwrap() = Some(value.to_owned());
            Ok(())
        }
    }

    fn signed_in() -> Option<SavedAccountSession> {
        Some(SavedAccountSession {
            tokens: AccountTokens {
                access_token: ACCESS_TOKEN.into(),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "synthetic-user".into(),
                    display_name: "合成用户".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            // An hour ahead: inside the thirty days a stored session may claim, and past the early-refresh window, so the session never asks the backend to refresh.
            expires_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000,
            session_id: None,
        })
    }

    type Session = BackendAccountSession<FakeApi, MemoryStorage>;
    type Calls = Arc<Mutex<Vec<String>>>;

    struct Fixture {
        session: Arc<Session>,
        calls: Calls,
        server: Arc<Mutex<FakeServer>>,
        keystore: Arc<Mutex<Option<String>>>,
    }

    fn shell(saved: Option<SavedAccountSession>, server: FakeServer) -> Fixture {
        let calls = Calls::default();
        let server = Arc::new(Mutex::new(server));
        let keystore = Arc::new(Mutex::new(None));
        let api = FakeApi {
            calls: Arc::clone(&calls),
            server: Arc::clone(&server),
        };
        let storage = MemoryStorage {
            session: saved,
            keystore: Arc::clone(&keystore),
        };
        Fixture {
            session: Arc::new(BackendAccountSession::new(api, storage)),
            calls,
            server,
            keystore,
        }
    }

    /// 账号已由另一台设备开启并建好密钥。
    fn enabled_elsewhere() -> FakeServer {
        FakeServer {
            enabled: true,
            key_id: Some(OTHER_KEY_ID.into()),
            ..FakeServer::default()
        }
    }

    fn request(session: &Arc<Session>, action: Value) -> Result<Value, &'static str> {
        tauri::async_runtime::block_on(cloud_clipboard_request(session, action))
            .map_err(|error| error.code)
    }

    #[test]
    fn list_returns_the_driver_shape_with_this_device_pending() {
        let fixture = shell(signed_in(), enabled_elsewhere());
        let page = request(
            &fixture.session,
            json!({ "operation": "list", "search": "合成" }),
        )
        .unwrap();
        assert_eq!(page["enabled"], true);
        assert_eq!(page["retention_days"], 0);
        assert_eq!(page["items"], json!([]));
        assert_eq!(page["e2e"]["status"], "pending");
        assert_eq!(page["e2e"]["pending_devices"], 0);
        assert_eq!(page["e2e"]["undecryptable"], 0);
        assert_eq!(page["e2e"]["code"].as_str().unwrap().len(), 19);
        // 登记了本机，并在发请求之前保存了它的密钥串。
        let calls = fixture.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0], "GET /v1/users/me/clipboard/keys");
        assert!(calls[1].starts_with("PUT /v1/users/me/clipboard/devices/"));
        assert_eq!(calls[2], "GET /v1/users/me/clipboard/keys");
        assert!(fixture.keystore.lock().unwrap().is_some());
    }

    #[test]
    fn driver_errors_reach_the_page_with_their_own_code() {
        let fixture = shell(signed_in(), enabled_elsewhere());
        assert_eq!(
            request(
                &fixture.session,
                json!({ "operation": "add", "text": "合成上传" })
            ),
            Err("clipboard_device_pending")
        );
        let other = "f".repeat(32);
        assert_eq!(
            request(
                &fixture.session,
                json!({ "operation": "approve", "device_id": other })
            ),
            Err("clipboard_device_pending")
        );
        assert_eq!(
            request(
                &fixture.session,
                json!({ "operation": "reject", "device_id": other })
            ),
            Err("not_found")
        );

        let disabled = shell(signed_in(), FakeServer::default());
        assert_eq!(
            request(&disabled.session, json!({ "operation": "reset" })),
            Err("clipboard_sync_disabled")
        );
    }

    #[test]
    fn devices_and_reset_return_the_status_shape() {
        let fixture = shell(signed_in(), enabled_elsewhere());
        let status = request(&fixture.session, json!({ "operation": "devices" })).unwrap();
        assert_eq!(status["enabled"], true);
        assert_eq!(status["status"], "pending");
        let own = status["device_id"].as_str().unwrap().to_owned();
        assert_eq!(status["devices"][0]["device_id"], own.as_str());
        assert_eq!(status["devices"][0]["current"], true);
        assert_eq!(status["devices"][0]["approved"], false);
        assert_eq!(status["devices"][0]["code"], status["code"]);
        assert_eq!(status["devices"][0]["name"], "合成设备");

        let reset = request(&fixture.session, json!({ "operation": "reset" })).unwrap();
        assert_eq!(reset["status"], "ready");
        assert_eq!(reset["device_id"], own.as_str());
        assert_eq!(fixture.server.lock().unwrap().wraps, [own]);
        assert_eq!(
            fixture
                .server
                .lock()
                .unwrap()
                .key_id
                .as_deref()
                .map(str::len),
            Some(32)
        );
    }

    #[test]
    fn delete_and_set_enabled_keep_calling_the_backend_directly() {
        let fixture = shell(signed_in(), enabled_elsewhere());
        assert_eq!(
            request(
                &fixture.session,
                json!({ "operation": "delete", "id": "synthetic-1" })
            )
            .unwrap(),
            json!({})
        );
        assert_eq!(
            request(
                &fixture.session,
                json!({ "operation": "set_enabled", "enabled": false })
            )
            .unwrap(),
            json!({ "enabled": false })
        );
        assert_eq!(
            *fixture.calls.lock().unwrap(),
            ["delete:synthetic-1", "set_enabled:false"]
        );
    }

    #[test]
    fn a_signed_out_session_reports_the_account_code_without_calling_the_backend() {
        let fixture = shell(None, enabled_elsewhere());
        let device_id = "f".repeat(32);
        for action in [
            json!({ "operation": "list", "search": "" }),
            json!({ "operation": "add", "text": "合成上传" }),
            json!({ "operation": "delete", "id": "synthetic-1" }),
            json!({ "operation": "set_enabled", "enabled": true }),
            json!({ "operation": "devices" }),
            json!({ "operation": "approve", "device_id": device_id }),
            json!({ "operation": "reject", "device_id": device_id }),
            json!({ "operation": "remove", "device_id": device_id }),
            json!({ "operation": "reset" }),
        ] {
            assert_eq!(
                request(&fixture.session, action),
                Err("account_unauthorized")
            );
        }
        assert!(fixture.calls.lock().unwrap().is_empty());
        assert!(fixture.keystore.lock().unwrap().is_none());
    }

    #[test]
    fn malformed_actions_are_rejected_before_the_backend() {
        let fixture = shell(signed_in(), enabled_elsewhere());
        for action in [
            json!({}),
            json!({ "operation": "unknown" }),
            // 账号分派也收置顶和保留期（Tauri 命令先按 `validate_request` 把它们拦在中继以外的路上），形状不对的同样不到后端。
            json!({ "operation": "pin", "id": "short", "pinned": true }),
            json!({ "operation": "set_retention", "days": 5 }),
            json!({ "operation": "add" }),
            json!({ "operation": "delete" }),
            json!({ "operation": "set_enabled", "enabled": "yes" }),
            json!({ "operation": "approve" }),
            json!({ "operation": "remove", "device_id": 7 }),
        ] {
            assert_eq!(
                request(&fixture.session, action),
                Err("invalid_cloud_clipboard")
            );
        }
        assert_eq!(
            request(
                &fixture.session,
                json!({ "operation": "remove", "device_id": "not-a-device" })
            ),
            Err("account_invalid")
        );
        assert!(fixture.calls.lock().unwrap().is_empty());
    }

    /// 只经账号答的外壳：没配置中继时放行；配置了中继、或中继配置读不出时一律拒绝，不会退回账号。
    #[test]
    fn account_only_shells_refuse_while_a_relay_is_configured() {
        use super::require_no_relay;
        use msime_client_core::preferences::PreferencesStore;

        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        assert_eq!(require_no_relay(&store).map_err(|error| error.code), Ok(()));
        let mut preferences = store.load().unwrap().preferences;
        preferences.cloud_clipboard_relay.url = "https://relay.example.test/msime".into();
        preferences.cloud_clipboard_relay.token = "synthetic-relay-token".into();
        store.save(0, preferences).unwrap();
        assert_eq!(
            require_no_relay(&store).map_err(|error| error.code),
            Err("relay_unsupported")
        );
        // 偏好文件坏了读不出来：不知道用户选的是哪条路，同样不发。
        std::fs::write(directory.path().join("preferences.json"), b"{").unwrap();
        assert!(require_no_relay(&store).is_err());
    }

    /// 中继按共享偏好选路：没配置时为 `None`，交给账号；配置了就给出规范化的地址。请求会联网，由 `msime-clipboard-relay` 的端到端测试覆盖。
    #[test]
    fn the_relay_route_reads_the_shared_preferences() {
        use super::configured_relay;
        use msime_client_core::preferences::PreferencesStore;

        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        assert_eq!(configured_relay(&store).unwrap(), None);
        let mut preferences = store.load().unwrap().preferences;
        preferences.cloud_clipboard_relay.url = "https://relay.example.test/msime/".into();
        preferences.cloud_clipboard_relay.token = "synthetic-relay-token".into();
        store.save(0, preferences).unwrap();
        assert_eq!(
            configured_relay(&store).unwrap().unwrap().url(),
            "https://relay.example.test/msime"
        );
    }
}
