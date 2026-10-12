//! 云剪贴板请求：每个宿主都接受的形状（[`validate_request`]）、经水杉账号的答复（[`account_request`]），以及经自建中继的答复（[`relay_request`]，见 `docs/cloud-clipboard-relay.md`）。

use msime_client_core::account::{
    AccountApi, AccountClipboardError, AccountError, AccountSessionStorage, BackendAccountSession,
};
use msime_client_core::clipboard_relay::{
    RelayClipboard, RelayConfig, RelayError, RelayTransport, RETENTION_DAYS,
};
use msime_client_core::preferences::PreferencesStore;
use msime_client_core::{
    has_disallowed_control_with_options, is_bounded_ascii_identifier, is_bounded_text,
    is_bounded_utf16, is_lower_hex,
};
use serde_json::{json, Value};
use std::path::Path;

pub fn validate_request(request: &Value) -> Result<(), &'static str> {
    let operation = request
        .get("operation")
        .and_then(Value::as_str)
        .ok_or("invalid cloud clipboard request")?;
    match operation {
        "list" => {
            let search = request.get("search").and_then(Value::as_str).unwrap_or("");
            if !is_bounded_text(search, 1024) {
                return Err("invalid cloud clipboard request");
            }
        }
        "add" => {
            let text = request
                .get("text")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            if text.is_empty()
                || !is_bounded_utf16(text, 4000)
                || has_disallowed_control_with_options(text, true)
            {
                return Err("invalid cloud clipboard request");
            }
        }
        "delete" => {
            let id = request
                .get("id")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            if !is_bounded_ascii_identifier(id, 256) {
                return Err("invalid cloud clipboard request");
            }
        }
        "set_enabled" => {
            if !request.get("enabled").is_some_and(Value::is_boolean) {
                return Err("invalid cloud clipboard request");
            }
        }
        // 端到端加密的设备管理，见 `docs/cloud-clipboard-e2e.md` 的「共享面板的动作」。
        "devices" | "reset" => {}
        "approve" | "reject" | "remove" => {
            let device_id = request
                .get("device_id")
                .and_then(Value::as_str)
                .ok_or("invalid cloud clipboard request")?;
            // 设备编号是公钥 SHA-256 的前 16 字节，小写十六进制。
            if !is_lower_hex(device_id, 32) {
                return Err("invalid cloud clipboard request");
            }
        }
        _ => return Err("invalid cloud clipboard request"),
    }
    Ok(())
}

/// 中继分支接受的动作：[`validate_request`] 的九个，再加上只在中继分支里有的 `pin {id, pinned}`、`set_retention {days}`（0、1、7、30）和 `clear`。账号分支不接受这三个，账号路径的行为因此不变；原生的 Android、iOS 和 macOS 界面已经有置顶、保留期和清空，改走中继时靠它们保持对等。
pub fn validate_relay_request(request: &Value) -> Result<(), &'static str> {
    const INVALID: &str = "invalid cloud clipboard request";
    match request.get("operation").and_then(Value::as_str) {
        Some("pin") => {
            let id = request.get("id").and_then(Value::as_str).ok_or(INVALID)?;
            if !is_lower_hex(id, 64) || !request.get("pinned").is_some_and(Value::is_boolean) {
                return Err(INVALID);
            }
            Ok(())
        }
        Some("set_retention") => {
            let days = request.get("days").and_then(Value::as_u64).ok_or(INVALID)?;
            if !RETENTION_DAYS
                .iter()
                .any(|allowed| u64::from(*allowed) == days)
            {
                return Err(INVALID);
            }
            Ok(())
        }
        Some("clear") => Ok(()),
        _ => validate_request(request),
    }
}

/// 偏好目录里配置的中继；没有配置时为 `None`。偏好读不了时是 `storage`，配置不合规时是 `relay_not_configured`。不联网。
pub fn configured_relay(preferences_directory: &Path) -> Result<Option<RelayConfig>, &'static str> {
    let snapshot = PreferencesStore::new(preferences_directory)
        .load()
        .map_err(|_| "storage")?;
    RelayConfig::from_preferences(&snapshot.preferences.cloud_clipboard_relay)
        .map_err(|error| error.code())
}

/// 宿主据此决定云剪贴板走哪条路：配置了中继时是 `relay`，否则是 `account`。不联网。
pub fn source(preferences_directory: &Path) -> Result<&'static str, &'static str> {
    Ok(if configured_relay(preferences_directory)?.is_some() {
        "relay"
    } else {
        "account"
    })
}

/// 经自建中继答一个云剪贴板动作，动作集见 [`validate_relay_request`]，结果形状与 [`account_request`] 相同；`pin`、`clear` 答 `{}`，`set_retention` 答 `{retention_days}`。`keystore_directory` 是本机放中继密钥串的目录，同一台设备上的各个进程要传同一个。会联网，不要在界面线程上调用。动作不合规时是 `invalid_cloud_clipboard`，中继拒绝令牌时是 `relay_unauthorized`，其余是驱动或中继给出的错误码（见 `RelayError::code`）。
pub fn relay_request(
    config: RelayConfig,
    keystore_directory: &Path,
    device_name: &str,
    action: &Value,
) -> Result<Value, &'static str> {
    const INVALID: &str = "invalid_cloud_clipboard";
    validate_relay_request(action).map_err(|_| INVALID)?;
    let relay = RelayClipboard::new(config, keystore_directory, device_name)
        .map_err(|error| error.code())?;
    let field = |name: &str| action.get(name).and_then(Value::as_str).ok_or(INVALID);
    let flag = |name: &str| action.get(name).and_then(Value::as_bool).ok_or(INVALID);
    let done = |result: Result<(), RelayError>, value: Value| {
        result.map(|()| value).map_err(|error| error.code())
    };
    match action
        .get("operation")
        .and_then(Value::as_str)
        .ok_or(INVALID)?
    {
        "list" => relay_value(
            relay.list(
                action
                    .get("search")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ),
        ),
        "add" => relay_value(relay.add(field("text")?)),
        "delete" => done(relay.delete(field("id")?), json!({})),
        "set_enabled" => {
            let enabled = flag("enabled")?;
            done(relay.set_enabled(enabled), json!({ "enabled": enabled }))
        }
        "devices" => relay_value(relay.devices()),
        "approve" => relay_value(relay.approve(field("device_id")?)),
        "reject" => relay_value(relay.reject(field("device_id")?)),
        "remove" => relay_value(relay.remove(field("device_id")?)),
        "reset" => relay_value(relay.reset()),
        "pin" => done(relay.set_pinned(field("id")?, flag("pinned")?), json!({})),
        "set_retention" => {
            let days = action
                .get("days")
                .and_then(Value::as_u64)
                .and_then(|days| u32::try_from(days).ok())
                .ok_or(INVALID)?;
            done(relay.set_retention(days), json!({ "retention_days": days }))
        }
        "clear" => done(relay.clear(), json!({})),
        _ => Err(INVALID),
    }
}

/// 向中继发端到端加密驱动给出的一个请求，答 `{status, body}`，宿主把它交回驱动（`msime_client_cloud_clipboard_e2e` 的 `resume`）。给自己跑驱动、自己保存密钥串的宿主用：这里不读也不写密钥串。路径不在云剪贴板接口之下、方法不认识或正文过大时是 `invalid_cloud_clipboard`，不发出去；中继拒绝令牌时是 `relay_unauthorized`，连不上或响应过大是 `relay_unavailable`；其余状态码原样交回。会联网，不要在界面线程上调用。
pub fn relay_exchange(
    config: RelayConfig,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<Value, &'static str> {
    let code = |error: RelayError| match error {
        RelayError::Invalid => "invalid_cloud_clipboard",
        error => error.code(),
    };
    let response = RelayTransport::new(config)
        .map_err(code)?
        .send(method, path, body)
        .map_err(code)?;
    Ok(json!({ "status": response.status, "body": response.body }))
}

fn relay_value<T: serde::Serialize>(result: Result<T, RelayError>) -> Result<Value, &'static str> {
    let value = result.map_err(|error| error.code())?;
    serde_json::to_value(value).map_err(|_| RelayError::Unavailable.code())
}

/// One cloud clipboard action answered through the account session: the `list`, `set_enabled`, `add` and `delete` the 云剪贴板 panel sends, plus the device management of end-to-end encryption (`devices`, `approve`, `reject`, `remove`, `reset`, see `docs/cloud-clipboard-e2e.md`). Blocks on the network, so callers run it off their UI thread. The Tauri shells pass the session their `AccountState` holds; the native Windows settings window (`msime_client_account_cloud_clipboard`) passes one over the signed-in session file the shell writes. An action that is not one of these, or lacks its field, is `invalid_cloud_clipboard` and never reaches the backend; a failed call is the account error's code, `account_unauthorized` when no one is signed in, or the driver's code (for example `clipboard_device_pending`).
pub fn account_request<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    action: &Value,
) -> Result<Value, &'static str> {
    const INVALID: &str = "invalid_cloud_clipboard";
    let operation = action
        .get("operation")
        .and_then(Value::as_str)
        .ok_or(INVALID)?;
    let field = |name: &str| action.get(name).and_then(Value::as_str).ok_or(INVALID);
    match operation {
        "list" => {
            let search = action
                .get("search")
                .and_then(Value::as_str)
                .unwrap_or_default();
            clipboard_value(session.clipboard(search))
        }
        "set_enabled" => {
            let enabled = action
                .get("enabled")
                .and_then(Value::as_bool)
                .ok_or(INVALID)?;
            session
                .set_clipboard_enabled(enabled)
                .map(|()| json!({ "enabled": enabled }))
                .map_err(|error| error.code())
        }
        "add" => {
            let text = field("text")?;
            clipboard_value(session.add_clipboard(text))
        }
        "delete" => {
            let id = field("id")?;
            session
                .delete_clipboard(Some(id))
                .map(|()| json!({}))
                .map_err(|error| error.code())
        }
        "devices" => clipboard_value(session.clipboard_devices()),
        "approve" => clipboard_value(session.approve_clipboard_device(field("device_id")?)),
        "reject" => clipboard_value(session.reject_clipboard_device(field("device_id")?)),
        "remove" => clipboard_value(session.remove_clipboard_device(field("device_id")?)),
        "reset" => clipboard_value(session.reset_clipboard()),
        _ => Err(INVALID),
    }
}

/// 经端到端加密驱动的结果交给页面；驱动或账号的错误换成页面认得的错误码，序列化不了按服务不可用处理。
fn clipboard_value<T: serde::Serialize>(
    result: Result<T, AccountClipboardError>,
) -> Result<Value, &'static str> {
    let value = result.map_err(|error| error.code())?;
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_list_add_delete_and_toggle_requests() {
        assert!(validate_request(&json!({"operation":"list","search":"水"})).is_ok());
        assert!(validate_request(&json!({"operation":"add","text":"line\nfeed"})).is_ok());
        assert!(validate_request(&json!({"operation":"delete","id":"entry-1"})).is_ok());
        assert!(validate_request(&json!({"operation":"set_enabled","enabled":true})).is_ok());
    }

    #[test]
    fn enforces_utf16_clipboard_bound() {
        assert!(validate_request(&json!({"operation":"add","text":"界".repeat(4000)})).is_ok());
        assert!(validate_request(&json!({"operation":"add","text":"界".repeat(4001)})).is_err());
    }

    #[test]
    fn accepts_device_management_with_a_well_formed_device_id() {
        let id = "0123456789abcdef".repeat(2);
        assert!(validate_request(&json!({"operation":"devices"})).is_ok());
        assert!(validate_request(&json!({"operation":"reset"})).is_ok());
        for operation in ["approve", "reject", "remove"] {
            assert!(validate_request(&json!({"operation":operation,"device_id":id})).is_ok());
            assert!(validate_request(&json!({"operation":operation})).is_err());
            for bad in [
                id.to_uppercase(),
                format!("{id}0"),
                id[..31].to_owned(),
                "g".repeat(32),
                "../".repeat(11)[..32].to_owned(),
            ] {
                assert!(
                    validate_request(&json!({"operation":operation,"device_id":bad})).is_err(),
                    "{operation} {bad}"
                );
            }
            assert!(validate_request(&json!({"operation":operation,"device_id":32})).is_err());
        }
    }

    #[test]
    fn pin_retention_and_clear_are_relay_only() {
        let id = "0123456789abcdef".repeat(4);
        for action in [
            json!({"operation":"pin","id":id,"pinned":true}),
            json!({"operation":"set_retention","days":0}),
            json!({"operation":"set_retention","days":30}),
            json!({"operation":"clear"}),
        ] {
            assert!(validate_relay_request(&action).is_ok(), "{action}");
            assert!(validate_request(&action).is_err(), "{action}");
        }
        for action in [
            json!({"operation":"pin","id":id}),
            json!({"operation":"pin","id":"entry-1","pinned":true}),
            json!({"operation":"pin","id":id.to_uppercase(),"pinned":false}),
            json!({"operation":"set_retention","days":3}),
            json!({"operation":"set_retention","days":-1}),
            json!({"operation":"set_retention"}),
            json!({"operation":"unknown"}),
        ] {
            assert!(validate_relay_request(&action).is_err(), "{action}");
        }
        // 共用的九个动作在中继分支里照样接受。
        assert!(validate_relay_request(&json!({"operation":"list","search":""})).is_ok());
        assert!(validate_relay_request(&json!({"operation":"devices"})).is_ok());
    }

    mod relay {
        use super::super::{configured_relay, relay_request, source};
        use msime_client_core::clipboard_relay::RelayConfig;
        use msime_client_core::preferences::{Preferences, PreferencesStore};
        use serde_json::json;

        fn store_with(url: &str, token: &str) -> tempfile::TempDir {
            let directory = tempfile::tempdir().unwrap();
            let store = PreferencesStore::new(directory.path());
            let mut preferences = Preferences::default();
            preferences.cloud_clipboard_relay.url = url.into();
            preferences.cloud_clipboard_relay.token = token.into();
            store.save(0, preferences).unwrap();
            directory
        }

        #[test]
        fn the_source_follows_the_preferences() {
            let empty = tempfile::tempdir().unwrap();
            assert_eq!(source(empty.path()), Ok("account"));
            assert_eq!(configured_relay(empty.path()), Ok(None));
            let configured =
                store_with("https://relay.example.test/msime/", "synthetic-relay-token");
            assert_eq!(source(configured.path()), Ok("relay"));
            assert_eq!(
                configured_relay(configured.path()).unwrap().unwrap().url(),
                "https://relay.example.test/msime"
            );
        }

        #[test]
        fn malformed_relay_actions_never_reach_the_network() {
            let keystore = tempfile::tempdir().unwrap();
            // 端口 9 上没有服务：动作若真的发出去，得到的会是 `relay_unavailable`。
            let config =
                || RelayConfig::new("http://127.0.0.1:9", "synthetic-relay-token").unwrap();
            for action in [
                json!({}),
                json!({ "operation": "unknown" }),
                json!({ "operation": "add" }),
                json!({ "operation": "pin", "id": "entry-1", "pinned": true }),
                json!({ "operation": "set_retention", "days": 2 }),
            ] {
                assert_eq!(
                    relay_request(config(), keystore.path(), "合成设备", &action),
                    Err("invalid_cloud_clipboard"),
                    "{action}"
                );
            }
            assert_eq!(
                relay_request(
                    config(),
                    keystore.path(),
                    "合成设备",
                    &json!({ "operation": "delete", "id": "entry-1" })
                ),
                Err("invalid_cloud_clipboard")
            );
            assert_eq!(
                relay_request(
                    config(),
                    keystore.path(),
                    "合成设备",
                    &json!({ "operation": "clear" })
                ),
                Err("relay_unavailable")
            );
            assert!(std::fs::read_dir(keystore.path()).unwrap().next().is_none());
        }
    }

    #[test]
    fn rejects_unsafe_or_unknown_requests() {
        assert!(validate_request(&json!({"operation":"list","search":"bad\n"})).is_err());
        assert!(validate_request(&json!({"operation":"add","text":"bad\u{0007}"})).is_err());
        assert!(validate_request(&json!({"operation":"delete","id":"bad/id"})).is_err());
        assert!(validate_request(&json!({"operation":"unknown"})).is_err());
    }

    mod account {
        use super::super::account_request;
        use msime_client_core::account::{
            AccountApi, AccountChallenge, AccountError, AccountProfile, AccountSessionStorage,
            AccountTokens, AccountUser, BackendAccountSession, SavedAccountSession,
        };
        use msime_client_core::cloud::clipboard_e2e::{HttpRequest, HttpResponse};
        use serde_json::json;
        use std::sync::{Arc, Mutex};

        /// The session validates stored tokens as 64 lowercase hex digits, so the synthetic ones have that shape.
        const ACCESS_TOKEN: &str =
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

        /// A backend that records the clipboard calls it receives; every call outside the clipboard is a test failure.
        struct FakeApi {
            calls: Arc<Mutex<Vec<String>>>,
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
            /// 云剪贴板关着的账号：驱动先取密钥状态，看到 `enabled: false` 就停下，列表答「已关闭」，上传和设备操作答 `clipboard_sync_disabled`。
            fn clipboard_e2e(
                &self,
                request: &HttpRequest,
                token: &str,
            ) -> Result<HttpResponse, AccountError> {
                let path = request.path.split('?').next().unwrap_or_default();
                self.record(format!("{} {path}", request.method), token);
                Ok(HttpResponse {
                    status: 200,
                    body: json!({ "enabled": false, "key_id": null, "wrap": null, "devices": [] })
                        .to_string(),
                })
            }
            fn set_clipboard_enabled(
                &self,
                enabled: bool,
                token: &str,
            ) -> Result<(), AccountError> {
                self.record(format!("set_enabled:{enabled}"), token);
                Ok(())
            }
            fn delete_clipboard(&self, id: Option<&str>, token: &str) -> Result<(), AccountError> {
                self.record(format!("delete:{}", id.unwrap_or("*")), token);
                Ok(())
            }
        }

        struct MemoryStorage(Option<SavedAccountSession>);

        impl AccountSessionStorage for MemoryStorage {
            fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
                Ok(self.0.clone())
            }
            fn save(&self, _: &SavedAccountSession) -> Result<(), AccountError> {
                Ok(())
            }
            fn clear(&self) -> Result<(), AccountError> {
                Ok(())
            }
            fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
                Ok(None)
            }
            fn save_clipboard_keystore(&self, _: &str) -> Result<(), AccountError> {
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
                // An hour ahead, past the early-refresh window, so the session never asks the backend to refresh.
                expires_at_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
                    + 3_600_000,
                session_id: None,
            })
        }

        type Calls = Arc<Mutex<Vec<String>>>;

        fn session(
            saved: Option<SavedAccountSession>,
        ) -> (BackendAccountSession<FakeApi, MemoryStorage>, Calls) {
            let calls = Calls::default();
            let api = FakeApi {
                calls: Arc::clone(&calls),
            };
            (BackendAccountSession::new(api, MemoryStorage(saved)), calls)
        }

        #[test]
        fn each_operation_reaches_its_backend_call_and_answers_what_the_page_reads() {
            let (session, calls) = session(signed_in());
            let list = account_request(&session, &json!({ "operation": "list", "search": "合成" }))
                .unwrap();
            assert_eq!(list["enabled"], false);
            assert_eq!(list["items"], json!([]));
            assert_eq!(list["e2e"]["status"], "disabled");
            assert_eq!(
                account_request(&session, &json!({ "operation": "add", "text": "合成上传" })),
                Err("clipboard_sync_disabled")
            );
            let devices = account_request(&session, &json!({ "operation": "devices" })).unwrap();
            assert_eq!(devices["status"], "disabled");
            assert_eq!(
                account_request(&session, &json!({ "operation": "reset" })),
                Err("clipboard_sync_disabled")
            );
            assert_eq!(
                account_request(
                    &session,
                    &json!({ "operation": "delete", "id": "synthetic-1" })
                ),
                Ok(json!({}))
            );
            assert_eq!(
                account_request(
                    &session,
                    &json!({ "operation": "set_enabled", "enabled": true })
                ),
                Ok(json!({ "enabled": true }))
            );
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    "GET /v1/users/me/clipboard/keys",
                    "GET /v1/users/me/clipboard/keys",
                    "GET /v1/users/me/clipboard/keys",
                    "GET /v1/users/me/clipboard/keys",
                    "delete:synthetic-1",
                    "set_enabled:true"
                ]
            );
        }

        #[test]
        fn signed_out_and_malformed_requests_never_reach_the_backend() {
            let (session, calls) = session(None);
            assert_eq!(
                account_request(&session, &json!({ "operation": "add", "text": "合成上传" })),
                Err("account_unauthorized")
            );
            let (session, signed_in_calls) = self::session(signed_in());
            for action in [
                json!({}),
                json!({ "operation": "clear" }),
                json!({ "operation": "add" }),
                json!({ "operation": "approve" }),
                json!({ "operation": "set_enabled", "enabled": "yes" }),
            ] {
                assert_eq!(
                    account_request(&session, &action),
                    Err("invalid_cloud_clipboard")
                );
            }
            assert!(calls.lock().unwrap().is_empty());
            assert!(signed_in_calls.lock().unwrap().is_empty());
        }
    }
}
