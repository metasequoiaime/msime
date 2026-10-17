//! `msime_client_account_cloud_clipboard` 的 `device_name`（#8201 的 P6）：账号路径与中继路径用同一条规则，名称一直传到端到端加密驱动登记本机的那个请求。

use super::*;
use msime_client_core::cloud::clipboard_e2e::{HttpRequest, HttpResponse};
use msime_client_services::account::{
    AccountApi, AccountChallenge, AccountError, AccountProfile, AccountSessionStorage,
    AccountTokens, AccountUser, BackendAccountSession, SavedAccountSession,
};
use std::ffi::CStr;
use std::sync::{Arc, Mutex};

fn cloud_clipboard(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串在下面释放。
    unsafe {
        let raw = msime_client_account_cloud_clipboard(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

#[test]
fn the_account_path_accepts_a_device_name_and_falls_back_on_an_unusable_one() {
    // 目录里没有会话：请求本身合法时是 account_unauthorized，不联网。
    let directory = tempfile::tempdir().unwrap();
    let list = json!({ "operation": "list", "search": "" });
    for device_name in [
        json!("HUAWEI Mate 70"),
        json!("   "),
        json!("名".repeat(86)),
        json!("bad\u{7}name"),
        Value::Null,
    ] {
        let reply = cloud_clipboard(json!({
            "directory": directory.path(),
            "device_name": device_name,
            "action": list,
        }));
        assert_eq!(
            reply["error"], "account_unauthorized",
            "{device_name}: {reply}"
        );
    }
    let reply = cloud_clipboard(json!({
        "directory": directory.path(),
        "device_name": 7,
        "action": list,
    }));
    assert_eq!(reply["error"], "invalid_cloud_clipboard", "{reply}");
}

#[test]
fn only_a_usable_name_is_passed_on() {
    assert_eq!(
        host_device_name(Some("HUAWEI Mate 70".into())).as_deref(),
        Some("HUAWEI Mate 70")
    );
    assert_eq!(
        host_device_name(Some("名".repeat(85))),
        Some("名".repeat(85))
    );
    for unusable in [
        None,
        Some(String::new()),
        Some(" \t ".into()),
        Some("名".repeat(86)),
        Some("bad\u{7}name".into()),
    ] {
        assert_eq!(host_device_name(unusable.clone()), None, "{unusable:?}");
    }
}

const ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// 云剪贴板开着、本机还没登记的账号：记下驱动发来的每个请求体里的 `name`，登记请求一律回 503，让操作在登记这一步结束。
struct RegisteringApi {
    names: Arc<Mutex<Vec<String>>>,
}

impl AccountApi for RegisteringApi {
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
    fn clipboard_e2e(&self, request: &HttpRequest, _: &str) -> Result<HttpResponse, AccountError> {
        if let Some(name) = request
            .body
            .as_deref()
            .and_then(|body| serde_json::from_str::<Value>(body).ok())
            .and_then(|body| body.get("name").and_then(Value::as_str).map(str::to_owned))
        {
            self.names.lock().unwrap().push(name);
            return Ok(HttpResponse {
                status: 503,
                body: String::new(),
            });
        }
        Ok(HttpResponse {
            status: 200,
            body: json!({ "enabled": true, "key_id": null, "wrap": null, "devices": [] })
                .to_string(),
        })
    }
}

struct SignedIn;

impl AccountSessionStorage for SignedIn {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        Ok(Some(SavedAccountSession {
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
            expires_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000,
            session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
        }))
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

#[test]
fn the_name_reaches_the_registration_request() {
    let names = Arc::new(Mutex::new(Vec::new()));
    let session = BackendAccountSession::new(
        RegisteringApi {
            names: Arc::clone(&names),
        },
        SignedIn,
    );
    let list = json!({ "operation": "list", "search": "" });
    for device_name in [Some("HUAWEI Mate 70"), None] {
        assert!(crate::cloud_clipboard::account_request(&session, &list, device_name).is_err());
    }
    assert_eq!(
        *names.lock().unwrap(),
        [
            "HUAWEI Mate 70".to_owned(),
            msime_client_services::account::cloud_clipboard_device_name()
        ]
    );
}
