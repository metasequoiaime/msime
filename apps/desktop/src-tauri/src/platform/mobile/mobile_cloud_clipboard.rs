//! Cloud clipboard requests over the account session, written once for iOS and Android.
//!
//! Both targets used to carry an identical copy of this body next to their account commands. It lives apart from [`super::mobile_community`] because it reads nothing from the community services: it talks to the account session itself, which each target's account state exposes through `AccountState::session`.

use serde_json::Value;
use std::sync::Arc;

use super::mobile_account_helpers::{account_value, call_session};
use super::MobileSession;

pub(crate) async fn cloud_clipboard_request(
    session: &Arc<MobileSession>,
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
