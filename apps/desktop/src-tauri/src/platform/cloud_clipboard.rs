//! Cloud clipboard requests over the account session, written once for every host that holds one.
//!
//! iOS and Android serve the page through their account session alone; the three desktop shells fall back to theirs when no provider socket or native input-method session answers. The body is generic over the session's backend and storage so each target passes the session its own `AccountState` holds, and so the tests can drive it against a synthetic backend.

use msime_client_core::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
use serde_json::Value;
use std::sync::Arc;

use super::account_helpers::{account_value, call_session};

pub(crate) async fn cloud_clipboard_request<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    action: Value,
) -> Result<Value, crate::CommandError> {
    let operation = action
        .get("operation")
        .and_then(Value::as_str)
        .ok_or(crate::CommandError {
            code: "invalid_cloud_clipboard",
        })?;
    match operation {
        "list" => {
            let search = action
                .get("search")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            call_session(session, move |session| {
                session.clipboard(&search).and_then(account_value)
            })
            .await
        }
        "set_enabled" => {
            let enabled =
                action
                    .get("enabled")
                    .and_then(Value::as_bool)
                    .ok_or(crate::CommandError {
                        code: "invalid_cloud_clipboard",
                    })?;
            call_session(session, move |session| {
                session
                    .set_clipboard_enabled(enabled)
                    .map(|()| serde_json::json!({ "enabled": enabled }))
            })
            .await
        }
        "add" => {
            let text = action
                .get("text")
                .and_then(Value::as_str)
                .ok_or(crate::CommandError {
                    code: "invalid_cloud_clipboard",
                })?
                .to_owned();
            call_session(session, move |session| {
                session.add_clipboard(&text).and_then(account_value)
            })
            .await
        }
        "delete" => {
            let id = action
                .get("id")
                .and_then(Value::as_str)
                .ok_or(crate::CommandError {
                    code: "invalid_cloud_clipboard",
                })?
                .to_owned();
            call_session(session, move |session| {
                session
                    .delete_clipboard(Some(&id))
                    .map(|()| serde_json::json!({}))
            })
            .await
        }
        _ => Err(crate::CommandError {
            code: "invalid_cloud_clipboard",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::cloud_clipboard_request;
    use msime_client_core::account::{
        AccountApi, AccountChallenge, AccountClipboardItem, AccountClipboardPage, AccountError,
        AccountProfile, AccountSessionStorage, AccountTokens, AccountUser, BackendAccountSession,
        SavedAccountSession,
    };
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    /// The session validates stored tokens as 64 lowercase hex digits, so the synthetic ones have that shape.
    const ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// A backend that records the clipboard calls it receives and answers them from a synthetic page. Every call outside the clipboard is a test failure.
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
        fn clipboard(
            &self,
            search: &str,
            token: &str,
        ) -> Result<AccountClipboardPage, AccountError> {
            self.record(format!("list:{search}"), token);
            Ok(AccountClipboardPage {
                enabled: true,
                items: vec![item("synthetic-1", "合成文本")],
            })
        }
        fn set_clipboard_enabled(&self, enabled: bool, token: &str) -> Result<(), AccountError> {
            self.record(format!("set_enabled:{enabled}"), token);
            Ok(())
        }
        fn add_clipboard(
            &self,
            text: &str,
            token: &str,
        ) -> Result<AccountClipboardItem, AccountError> {
            self.record(format!("add:{text}"), token);
            Ok(item("synthetic-2", text))
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
    }

    fn item(id: &str, text: &str) -> AccountClipboardItem {
        AccountClipboardItem {
            id: id.into(),
            text: text.into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
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

    fn session(saved: Option<SavedAccountSession>) -> (Arc<Session>, Calls) {
        let calls = Calls::default();
        let api = FakeApi {
            calls: Arc::clone(&calls),
        };
        (
            Arc::new(BackendAccountSession::new(api, MemoryStorage(saved))),
            calls,
        )
    }

    fn request(session: &Arc<Session>, action: Value) -> Result<Value, &'static str> {
        tauri::async_runtime::block_on(cloud_clipboard_request(session, action))
            .map_err(|error| error.code)
    }

    #[test]
    fn list_returns_the_enabled_flag_and_items_the_page_reads() {
        let (session, calls) = session(signed_in());
        let page = request(&session, json!({ "operation": "list", "search": "合成" })).unwrap();
        assert_eq!(
            page,
            json!({
                "enabled": true,
                "items": [{ "id": "synthetic-1", "text": "合成文本", "updated_at": "2026-01-01T00:00:00Z" }],
            })
        );
        assert_eq!(*calls.lock().unwrap(), ["list:合成"]);
    }

    #[test]
    fn add_delete_and_set_enabled_reach_the_matching_backend_call() {
        let (session, calls) = session(signed_in());
        let added = request(&session, json!({ "operation": "add", "text": "合成上传" })).unwrap();
        assert_eq!(added["id"], "synthetic-2");
        assert_eq!(added["text"], "合成上传");
        assert_eq!(
            request(
                &session,
                json!({ "operation": "delete", "id": "synthetic-1" })
            )
            .unwrap(),
            json!({})
        );
        assert_eq!(
            request(
                &session,
                json!({ "operation": "set_enabled", "enabled": false })
            )
            .unwrap(),
            json!({ "enabled": false })
        );
        assert_eq!(
            *calls.lock().unwrap(),
            ["add:合成上传", "delete:synthetic-1", "set_enabled:false"]
        );
    }

    #[test]
    fn a_signed_out_session_reports_the_account_code_without_calling_the_backend() {
        let (session, calls) = session(None);
        for action in [
            json!({ "operation": "list", "search": "" }),
            json!({ "operation": "add", "text": "合成上传" }),
            json!({ "operation": "delete", "id": "synthetic-1" }),
            json!({ "operation": "set_enabled", "enabled": true }),
        ] {
            assert_eq!(request(&session, action), Err("account_unauthorized"));
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn malformed_actions_are_rejected_before_the_backend() {
        let (session, calls) = session(signed_in());
        for action in [
            json!({}),
            json!({ "operation": "clear" }),
            json!({ "operation": "add" }),
            json!({ "operation": "delete" }),
            json!({ "operation": "set_enabled", "enabled": "yes" }),
        ] {
            assert_eq!(request(&session, action), Err("invalid_cloud_clipboard"));
        }
        assert!(calls.lock().unwrap().is_empty());
    }
}
