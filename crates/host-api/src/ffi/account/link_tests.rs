//! `msime_client_account` 的 `purpose:"link"`（给已登录账号添加登录方式，#8201 的决定 9）在 C 上的形状：只用登录会话，不碰匿名账号；不合法的请求在联网之前就是 `account_invalid`。真正发出去的请求、需要最近登录和换成新会话由 client-services 的 `account::link_tests` 用本机假服务器覆盖。

use super::super::session_store::tests::{leaked_store, registered, session_bytes, unregister};
use super::*;
use std::ffi::CStr;

const SHARED: u32 = 1;
const VERIFIER: &str = "vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv";
const GRANT: &str = "ggggggggggggggggggggggggggggggggggggggggggA";
const ID_TOKEN: &str = "eyJhbGciOiJSUzI1NiJ9.eyJub25jZSI6ImZpeHR1cmUifQ.c2lnbmF0dXJl";

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串在下面释放。
    unsafe {
        let raw = msime_client_account(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

/// 六个绑定步骤，形状都合格。
fn link_steps() -> [Value; 6] {
    [
        json!({ "operation": "request_code", "provider": "email", "target": "person@example.test", "purpose": "link" }),
        json!({ "operation": "login", "challenge_id": "c".repeat(64), "code": "123456", "purpose": "link" }),
        json!({ "operation": "google_challenge", "purpose": "link" }),
        json!({ "operation": "google_token_login", "challenge_id": "d".repeat(64), "id_token": ID_TOKEN, "purpose": "link" }),
        json!({ "operation": "apple_web_start", "app": "app.msime.android", "purpose": "link" }),
        json!({ "operation": "apple_web_login", "grant": GRANT, "code_verifier": VERIFIER, "purpose": "link" }),
    ]
}

#[test]
fn linking_needs_the_signed_in_account_and_never_falls_back_to_the_anonymous_one() {
    // 按目录：目录里没有登录会话。
    let directory = tempfile::tempdir().unwrap();
    for step in link_steps() {
        assert_eq!(
            call(json!({ "directory": directory.path(), "action": step.clone() })),
            json!({ "ok": false, "error": "account_unauthorized" }),
            "{step}"
        );
    }

    // 注入：只有匿名会话（ANONYMOUS 项）时也一样，不会拿匿名账号去绑定。
    let store = leaked_store();
    let id = registered(store, SHARED, 0);
    store
        .items
        .lock()
        .unwrap()
        .insert(3, session_bytes("synthetic-anonymous", 'c', true));
    for step in link_steps() {
        assert_eq!(
            call(json!({ "session_store": id, "action": step.clone() })),
            json!({ "ok": false, "error": "account_unauthorized" }),
            "{step}"
        );
    }
    // 没有登录会话可读：没有加锁、没有写入，匿名会话原样留着。
    assert!(store.locks.lock().unwrap().is_empty());
    assert!(store.saves.lock().unwrap().is_empty());
    assert!(store.items.lock().unwrap().contains_key(&3));
    unregister(id);
}

#[test]
fn a_link_request_that_cannot_be_sent_is_refused_before_the_network() {
    // 已登录（令牌在有效期内），所以下面每一个都只能是在本地被拒；漏了检查的会去连正式服务端。
    let store = leaked_store();
    let id = registered(store, SHARED, 0);
    store
        .items
        .lock()
        .unwrap()
        .insert(1, session_bytes("synthetic-user", 'a', true));
    for action in [
        // 用途只有 login 和 link。
        json!({ "operation": "request_code", "provider": "email", "target": "person@example.test", "purpose": "bind" }),
        // 只能绑邮箱。
        json!({ "operation": "request_code", "provider": "phone", "target": "+8613800000000", "purpose": "link" }),
        json!({ "operation": "login", "challenge_id": "c".repeat(64), "code": "12345", "purpose": "link" }),
        // 回环流程不能绑定。
        json!({ "operation": "google_challenge", "target": "http://127.0.0.1:53682/callback", "purpose": "link" }),
        json!({ "operation": "google_token_login", "challenge_id": "d".repeat(64), "id_token": "not-a-jwt", "purpose": "link" }),
        json!({ "operation": "apple_web_start", "app": "App.Msime", "purpose": "link" }),
        json!({ "operation": "apple_web_login", "grant": GRANT, "code_verifier": "short", "purpose": "link" }),
        // 其余登录方式没有 purpose 字段。
        json!({ "operation": "apple_challenge", "purpose": "link" }),
        json!({ "operation": "apple_login", "challenge_id": "e".repeat(64), "identity_token": "token", "purpose": "link" }),
        json!({ "operation": "google_code_login", "challenge_id": "e".repeat(64), "code": "4/0Acode", "purpose": "link" }),
        // google_login 带了 purpose 也不会打开浏览器做一次普通登录，google_cancel 同样不收多余字段。
        json!({ "operation": "google_login", "purpose": "link" }),
        json!({ "operation": "google_cancel", "purpose": "link" }),
    ] {
        assert_eq!(
            call(json!({ "session_store": id, "action": action.clone() })),
            json!({ "ok": false, "error": "account_invalid" }),
            "{action}"
        );
    }
    // 显式的 purpose:"login" 与不带一样，走原来的登录。
    assert_eq!(
        call(json!({ "session_store": id, "action": {
            "operation": "request_code", "provider": "google", "target": "", "purpose": "login",
        } })),
        json!({ "ok": false, "error": "account_invalid" })
    );
    assert!(store.saves.lock().unwrap().is_empty());
    unregister(id);
}
