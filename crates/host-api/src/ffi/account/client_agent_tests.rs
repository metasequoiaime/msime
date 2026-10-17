//! 目录模式的 `client_agent`（#8201 的决定 1）：带 `directory` 的请求可以自报 User-Agent，校验与注入存储的 `client_agent` 相同；不带时与以前相同。发出去的 User-Agent 由 client-services 的 `with_user_agent` 测试核对，这里核对请求的形状和会话对象怎么按 `client_agent` 分开。

use super::super::chat_stream::msime_client_account_chat_stream;
use super::super::session_store::tests::{leaked_store, registered, unregister};
use super::*;
use msime_client_services::account::{
    AccountSessionStorage, AccountTokens, AccountUser, AnonymousSessionStorage, SavedAccountSession,
};
use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};

const AGENT: &str = "msime-windows/1.4.0 (Surface Pro; Windows 11; edition=full)";
const OTHER_AGENT: &str = "msime-linux/1.4.0";

fn read(raw: *mut c_char) -> Value {
    // SAFETY: 导出函数返回的字符串，读完释放。
    unsafe {
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效。
    read(unsafe { msime_client_account(bytes.as_ptr(), bytes.len()) })
}

fn chat_stream(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，没有增量回调。
    read(unsafe {
        msime_client_account_chat_stream(bytes.as_ptr(), bytes.len(), None, std::ptr::null_mut())
    })
}

/// 一份未过期的合成会话。
fn saved(user: &str) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: "a".repeat(64),
            refresh_token: "b".repeat(64),
            token_type: "Bearer".into(),
            expires_in: 3600,
            user: AccountUser {
                id: user.into(),
                display_name: "合成用户".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                email: None,
                avatar_url: None,
            },
        },
        expires_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000,
        session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
    }
}

#[test]
fn a_directory_request_may_name_its_client_agent() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    std::fs::create_dir(&directory).unwrap();
    FileAccountSessionStorage::new(&directory, AccountSessionFileLayout::Native)
        .save(&saved("synthetic-user"))
        .unwrap();
    // 同一份会话文件，带不带 client_agent 读到的都是它。
    for agent in [Some(AGENT), None] {
        let mut request = json!({ "directory": directory, "action": { "operation": "status" } });
        if let Some(agent) = agent {
            request["client_agent"] = json!(agent);
        }
        assert_eq!(call(request)["value"]["user"]["id"], "synthetic-user");
    }
    // 匿名账号已经注册过：带 client_agent 的 ensure_anonymous 只读一次会话文件，不联网。
    AnonymousSessionStorage::new(&directory)
        .save(&saved("synthetic-anonymous"))
        .unwrap();
    assert_eq!(
        call(json!({
            "directory": directory,
            "client_agent": AGENT,
            "action": { "operation": "ensure_anonymous" },
        })),
        json!({ "ok": true, "value": true })
    );
    // 新操作也收它：没有登录会话时与不带时一样是 account_unauthorized。
    let empty = root.path().join("empty");
    assert_eq!(
        call(json!({
            "directory": empty,
            "client_agent": AGENT,
            "action": { "operation": "account_delete" },
        })),
        json!({ "ok": false, "error": "account_unauthorized" })
    );
    // 流式对话的会话字段与 msime_client_account 相同。
    assert_eq!(
        chat_stream(json!({
            "directory": empty,
            "client_agent": AGENT,
            "request_id": "client-agent-test",
            "messages": [{ "role": "user", "content": "合成的问题" }],
            "model": "synthetic-model",
        })),
        json!({ "ok": false, "error": "account_unauthorized" })
    );
}

#[test]
fn each_client_agent_has_its_own_session_object_for_a_directory() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path();
    let default = session_for(directory, None).unwrap();
    let named = session_with_agent(directory, None, Some(AGENT)).unwrap();
    let other = session_with_agent(directory, None, Some(OTHER_AGENT)).unwrap();
    assert!(!Arc::ptr_eq(&default, &named));
    assert!(!Arc::ptr_eq(&named, &other));
    // 同一目录、同一个 client_agent 是同一个对象，google_login 和 google_cancel 落在它上面。
    assert!(Arc::ptr_eq(
        &named,
        &session_with_agent(directory, None, Some(AGENT)).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &default,
        &session_with_agent(directory, None, None).unwrap()
    ));
    let anonymous = anonymous_session_with_agent(directory, Some(AGENT)).unwrap();
    assert!(!Arc::ptr_eq(
        &anonymous,
        &anonymous_session_for(directory).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &anonymous,
        &anonymous_session_with_agent(directory, Some(AGENT)).unwrap()
    ));
}

#[test]
fn a_client_agent_must_have_the_shape_the_server_reads_and_needs_a_directory() {
    let root = tempfile::tempdir().unwrap();
    for agent in [
        "MSIME/Android",
        "msime-web/1.0",
        "msime-windows/",
        "msime-windows/1.0 (nested (parens))",
        "msime-windows/1.0 (\u{4e2d})",
        "msime-windows/1.0\t(tab)",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain([format!("msime-windows/1.0 ({})", "x".repeat(256))])
    {
        assert_eq!(
            call(json!({
                "directory": root.path(),
                "client_agent": agent,
                "action": { "operation": "status" },
            })),
            json!({ "ok": false, "error": "account_invalid" }),
            "{agent}"
        );
        assert_eq!(
            chat_stream(json!({
                "directory": root.path(),
                "client_agent": agent,
                "request_id": "client-agent-invalid",
                "messages": [{ "role": "user", "content": "合成的问题" }],
                "model": "synthetic-model",
            }))["error"],
            "account_invalid",
            "{agent}"
        );
    }
    // 注入存储的 User-Agent 是注册时给的那个，请求里不能再给。
    let store = leaked_store();
    let id = registered(store, 1, 0);
    assert_eq!(
        call(json!({
            "session_store": id,
            "client_agent": AGENT,
            "action": { "operation": "status" },
        })),
        json!({ "ok": false, "error": "account_invalid" })
    );
    assert_eq!(
        chat_stream(json!({
            "session_store": id,
            "client_agent": AGENT,
            "request_id": "client-agent-store",
            "messages": [{ "role": "user", "content": "合成的问题" }],
            "model": "synthetic-model",
        }))["error"],
        "account_invalid"
    );
    // 不带时与以前一样。
    assert_eq!(
        call(json!({ "session_store": id, "action": { "operation": "status" } })),
        json!({ "ok": true, "value": { "user": null } })
    );
    unregister(id);
}
