//! `session_adopt`（#8201 的 P3）经 C 入口：鸿蒙键盘导入设置应用发布的会话、设置应用导入键盘的，规则本身的各种情形在 client-services 的 `session::adopt` 测试里，这里核对请求形状、两份会话各落到哪个文件、扁平布局的发布文件，以及注入存储和坏请求被拒绝。

use super::super::session_store::tests::{leaked_store, registered};
use super::*;
use msime_client_services::account::{
    AccountSessionStorage, AccountTokens, AccountUser, AnonymousSessionStorage,
    SavedAccountSession, ACCOUNT_SESSION_FILE, ANONYMOUS_ACCOUNT_FILE, ANONYMOUS_SESSION_FILE,
};
use std::ffi::CStr;
use std::time::{SystemTime, UNIX_EPOCH};

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串读完释放。
    unsafe {
        let raw = msime_client_account(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

fn adopt(directory: &Path, action: Value) -> Value {
    call(json!({ "directory": directory, "action": action }))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn saved(user: &str, refresh: char) -> SavedAccountSession {
    SavedAccountSession {
        tokens: AccountTokens {
            access_token: "a".repeat(64),
            refresh_token: refresh.to_string().repeat(64),
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
        expires_at_unix_ms: now_ms() + 3_600_000,
        session_id: Some(msime_client_core::uuid::Uuid::new_v4()),
    }
}

fn account_storage(directory: &Path) -> FileAccountSessionStorage {
    FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native)
}

fn refresh_token(saved: Option<SavedAccountSession>) -> Option<String> {
    saved.map(|saved| saved.tokens.refresh_token)
}

#[test]
fn the_keyboard_takes_the_settings_apps_session_and_its_sign_out() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("settings").join("state");
    let keyboard = root.path().join("keyboard").join("state");
    account_storage(&keyboard)
        .save(&saved("person-a", 'b'))
        .unwrap();
    account_storage(&published)
        .save(&saved("person-b", 'c'))
        .unwrap();

    let path = published.join(ACCOUNT_SESSION_FILE);
    let request = json!({ "operation": "session_adopt", "path": path, "item": "account", "from": "settings" });
    assert_eq!(
        adopt(&keyboard, request.clone()),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    assert_eq!(
        refresh_token(account_storage(&keyboard).load().unwrap()),
        Some("c".repeat(64))
    );
    assert_eq!(
        adopt(&keyboard, json!({ "operation": "status" }))["value"]["user"]["id"],
        "person-b"
    );
    // 再导入同一份不再写。
    assert_eq!(
        adopt(&keyboard, request),
        json!({ "ok": true, "value": { "outcome": "unchanged" } })
    );

    // 设置应用退出了：发布方没有这个文件（或者镜像清单说它不在、不给 path），键盘清掉本地会话。
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        adopt(
            &keyboard,
            json!({ "operation": "session_adopt", "path": path, "item": "account", "from": "settings" })
        ),
        json!({ "ok": true, "value": { "outcome": "cleared" } })
    );
    assert!(!keyboard.join(ACCOUNT_SESSION_FILE).exists());
    assert_eq!(
        adopt(
            &keyboard,
            json!({ "operation": "session_adopt", "item": "account", "from": "settings" })
        ),
        json!({ "ok": true, "value": { "outcome": "unchanged" } })
    );
}

#[test]
fn a_flat_harmony_session_published_before_the_upgrade_is_adopted_as_native() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("settings").join("state");
    let keyboard = root.path().join("keyboard").join("state");
    account_storage(&published)
        .save(&saved("person-a", 'b'))
        .unwrap();
    // 设置应用还没被 Rust 写过：发布的是 ArkTS 的扁平布局，访问令牌已经过期。
    let path = published.join(ACCOUNT_SESSION_FILE);
    std::fs::write(
        &path,
        format!(
            r#"{{"access_token":"{}","refresh_token":"{}","token_type":"Bearer","expires_at":{},"user":{{"id":"person-a","display_name":"","created_at":"2026-01-01T00:00:00Z"}}}}"#,
            "1".repeat(64),
            "2".repeat(64),
            now_ms() - 60_000
        ),
    )
    .unwrap();

    assert_eq!(
        adopt(
            &keyboard,
            json!({ "operation": "session_adopt", "path": path, "item": "account", "from": "settings" })
        ),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    let written: Value =
        serde_json::from_slice(&std::fs::read(keyboard.join(ACCOUNT_SESSION_FILE)).unwrap())
            .unwrap();
    assert_eq!(written["tokens"]["refresh_token"], "2".repeat(64));
    assert_eq!(written["tokens"]["expires_in"], 1);
    assert!(written.get("access_token").is_none());
}

#[test]
fn the_anonymous_item_goes_to_the_anonymous_session_and_keeps_the_identity() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("keyboard").join("state");
    let settings = root.path().join("settings").join("state");
    let mut local = saved("msime-anonymous", 'b');
    local.expires_at_unix_ms -= 600_000;
    AnonymousSessionStorage::new(&settings)
        .save(&local)
        .unwrap();
    std::fs::write(settings.join(ANONYMOUS_ACCOUNT_FILE), b"synthetic-identity").unwrap();
    // 键盘刷新过同一次登录的匿名会话。
    let mut refreshed = local.clone();
    refreshed.tokens.refresh_token = "c".repeat(64);
    refreshed.expires_at_unix_ms += 1_200_000;
    AnonymousSessionStorage::new(&published)
        .save(&refreshed)
        .unwrap();

    let path = published.join(ANONYMOUS_SESSION_FILE);
    assert_eq!(
        adopt(
            &settings,
            json!({ "operation": "session_adopt", "path": path, "item": "anonymous", "from": "keyboard" })
        ),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    assert_eq!(
        refresh_token(AnonymousSessionStorage::new(&settings).load().unwrap()),
        Some("c".repeat(64))
    );
    // 登录会话没有被碰。
    assert!(!settings.join(ACCOUNT_SESSION_FILE).exists());

    // 匿名会话的「来源不存在」只清会话，身份留着。
    let keyboard = root.path().join("keyboard-local").join("state");
    AnonymousSessionStorage::new(&keyboard)
        .save(&local)
        .unwrap();
    std::fs::write(keyboard.join(ANONYMOUS_ACCOUNT_FILE), b"synthetic-identity").unwrap();
    assert_eq!(
        adopt(
            &keyboard,
            json!({ "operation": "session_adopt", "item": "anonymous", "from": "settings" })
        ),
        json!({ "ok": true, "value": { "outcome": "cleared" } })
    );
    assert!(!keyboard.join(ANONYMOUS_SESSION_FILE).exists());
    assert!(keyboard.join(ANONYMOUS_ACCOUNT_FILE).exists());
}

#[test]
fn bad_adopt_requests_are_refused_without_touching_the_session() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard");
    account_storage(&keyboard)
        .save(&saved("person-a", 'b'))
        .unwrap();
    let invalid = json!({ "ok": false, "error": "account_invalid" });
    for action in [
        json!({ "operation": "session_adopt", "path": "relative/account-session.json", "item": "account", "from": "settings" }),
        json!({ "operation": "session_adopt", "item": "account" }),
        json!({ "operation": "session_adopt", "item": "both", "from": "settings" }),
        json!({ "operation": "session_adopt", "item": "account", "from": "server" }),
        json!({ "operation": "session_adopt", "item": "account", "from": "settings", "extra": 1 }),
    ] {
        assert_eq!(adopt(&keyboard, action.clone()), invalid, "{action}");
    }
    // 发布的文件不是会话：account_storage，本地会话留着。
    let published = root.path().join("published");
    std::fs::create_dir_all(&published).unwrap();
    let path = published.join(ACCOUNT_SESSION_FILE);
    std::fs::write(&path, b"{}").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert_eq!(
        adopt(
            &keyboard,
            json!({ "operation": "session_adopt", "path": path, "item": "account", "from": "settings" })
        ),
        json!({ "ok": false, "error": "account_storage" })
    );
    assert_eq!(
        refresh_token(account_storage(&keyboard).load().unwrap()),
        Some("b".repeat(64))
    );
    // 注入存储的宿主没有这个操作。
    let id = registered(leaked_store(), 0, 0);
    assert_eq!(
        call(
            json!({ "session_store": id, "action": { "operation": "session_adopt", "item": "account", "from": "settings" } })
        ),
        invalid
    );
}
