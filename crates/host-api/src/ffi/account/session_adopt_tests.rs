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

/// 设置应用本地的登录会话读不了时，键盘发布的那份按规则不用，但回答里带 local_unreadable：本地恢复可读以后同一份发布物就该采用（同一个账号、键盘那份是刷新过的），宿主不能把它记成不用再试。
#[test]
fn an_unreadable_local_session_is_reported_so_the_same_publication_is_tried_again() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("keyboard").join("state");
    let settings = root.path().join("settings").join("state");
    let mut refreshed = saved("person-a", 'c');
    refreshed.expires_at_unix_ms += 60_000;
    account_storage(&published).save(&refreshed).unwrap();
    private_write(&settings.join(ACCOUNT_SESSION_FILE), b"not a session");

    let path = published.join(ACCOUNT_SESSION_FILE);
    let request = json!({ "operation": "session_adopt", "path": path, "item": "account", "from": "keyboard" });
    assert_eq!(
        adopt(&settings, request.clone()),
        json!({ "ok": true, "value": { "outcome": "unchanged", "local_unreadable": true } })
    );
    assert_eq!(
        std::fs::read(settings.join(ACCOUNT_SESSION_FILE)).unwrap(),
        b"not a session"
    );

    // 本地恢复成同一个账号刷新前的那份：同一份发布物这次采用。
    account_storage(&settings)
        .save(&saved("person-a", 'b'))
        .unwrap();
    assert_eq!(
        adopt(&settings, request.clone()),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    assert_eq!(
        refresh_token(account_storage(&settings).load().unwrap()),
        Some("c".repeat(64))
    );
    // 本地读得了时规则给出的 unchanged 不带这个字段。
    assert_eq!(
        adopt(&settings, request),
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

    // 匿名会话的发布方没有会话时什么也不清：匿名账号没有退出登录，清掉只会逼着重新注册、用掉每个 IP 每天的注册额度。
    let keyboard = root.path().join("keyboard-local").join("state");
    AnonymousSessionStorage::new(&keyboard)
        .save(&local)
        .unwrap();
    std::fs::write(keyboard.join(ANONYMOUS_ACCOUNT_FILE), b"synthetic-identity").unwrap();
    for request in [
        json!({ "operation": "session_adopt", "item": "anonymous", "from": "settings" }),
        json!({ "operation": "session_adopt", "path": keyboard.join("missing").join(ANONYMOUS_SESSION_FILE), "item": "anonymous", "from": "settings" }),
        json!({ "operation": "session_adopt", "item": "anonymous", "from": "keyboard" }),
    ] {
        assert_eq!(
            adopt(&keyboard, request),
            json!({ "ok": true, "value": { "outcome": "unchanged" } })
        );
    }
    assert_eq!(
        refresh_token(AnonymousSessionStorage::new(&keyboard).load().unwrap()),
        Some("b".repeat(64))
    );
    assert!(keyboard.join(ANONYMOUS_ACCOUNT_FILE).exists());
}

/// 键盘先于设置应用启用：设置应用第一次启动、本地还没有匿名会话时采用键盘注册的那个匿名账号，不再自己注册一个；登录会话照旧不用键盘发布的。
#[test]
fn the_settings_app_takes_the_keyboards_anonymous_session_when_it_has_none() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("keyboard").join("state");
    let settings = root.path().join("settings").join("state");
    AnonymousSessionStorage::new(&published)
        .save(&saved("msime-keyboard00000000", 'b'))
        .unwrap();
    account_storage(&published)
        .save(&saved("person-a", 'c'))
        .unwrap();

    assert_eq!(
        adopt(
            &settings,
            json!({ "operation": "session_adopt", "path": published.join(ANONYMOUS_SESSION_FILE), "item": "anonymous", "from": "keyboard" })
        ),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    let adopted = AnonymousSessionStorage::new(&settings)
        .load()
        .unwrap()
        .unwrap();
    assert_eq!(adopted.tokens.user.id, "msime-keyboard00000000");
    assert_eq!(adopted.tokens.refresh_token, "b".repeat(64));
    // 身份文件不动：session_adopt 只写会话。
    assert!(!settings.join(ANONYMOUS_ACCOUNT_FILE).exists());

    assert_eq!(
        adopt(
            &settings,
            json!({ "operation": "session_adopt", "path": published.join(ACCOUNT_SESSION_FILE), "item": "account", "from": "keyboard" })
        ),
        json!({ "ok": true, "value": { "outcome": "unchanged" } })
    );
    assert!(!settings.join(ACCOUNT_SESSION_FILE).exists());
}

#[test]
fn both_items_take_the_later_copy_whatever_the_login_id() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("keyboard").join("state");
    let settings = root.path().join("settings").join("state");
    // 设置应用里是 Rust 注册或登录时写的 v4 会话；旧版键盘的 ArkTS 桥接把它轮换过，写回时丢了 session_id。
    let registration = saved("msime-anonymous", 'b');
    let mut rotated = saved("msime-anonymous", 'c');
    rotated.session_id = None;
    rotated.expires_at_unix_ms += 600_000;
    for directory in [&settings, &published] {
        AnonymousSessionStorage::new(directory)
            .save(&registration)
            .unwrap();
        account_storage(directory).save(&registration).unwrap();
    }
    AnonymousSessionStorage::new(&published)
        .save(&rotated)
        .unwrap();
    account_storage(&published).save(&rotated).unwrap();

    // 同一个 user.id 只比 expires_at：两份会话都采用更新的那份，不留着用过的 `b`。
    for (item, file) in [
        ("account", ACCOUNT_SESSION_FILE),
        ("anonymous", ANONYMOUS_SESSION_FILE),
    ] {
        assert_eq!(
            adopt(
                &settings,
                json!({ "operation": "session_adopt", "path": published.join(file), "item": item, "from": "keyboard" })
            ),
            json!({ "ok": true, "value": { "outcome": "adopted" } }),
            "{item}"
        );
    }
    assert_eq!(
        refresh_token(account_storage(&settings).load().unwrap()),
        Some("c".repeat(64))
    );
    assert_eq!(
        refresh_token(AnonymousSessionStorage::new(&settings).load().unwrap()),
        Some("c".repeat(64))
    );
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

fn identity_json(fill: char) -> String {
    format!(
        r#"{{"subject":"msime-{}","secret":"{}"}}"#,
        fill.to_string().repeat(16),
        fill.to_string().repeat(48)
    )
}

fn private_write(path: &Path, contents: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// 带 `identity_path` 时身份跟着匿名会话走：设置应用第一次注册断网失败、留着身份 S 没有会话，采用键盘注册的 K 时连同 K 的身份一起采用。
#[test]
fn with_an_identity_path_the_identity_follows_the_adopted_anonymous_session() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("keyboard").join("state");
    let settings = root.path().join("settings").join("state");
    AnonymousSessionStorage::new(&published)
        .save(&saved("msime-keyboard00000000", 'b'))
        .unwrap();
    private_write(
        &published.join(ANONYMOUS_ACCOUNT_FILE),
        identity_json('k').as_bytes(),
    );
    private_write(
        &settings.join(ANONYMOUS_ACCOUNT_FILE),
        identity_json('s').as_bytes(),
    );

    let request = json!({
        "operation": "session_adopt",
        "path": published.join(ANONYMOUS_SESSION_FILE),
        "identity_path": published.join(ANONYMOUS_ACCOUNT_FILE),
        "item": "anonymous",
        "from": "keyboard",
    });
    assert_eq!(
        adopt(&settings, request.clone()),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    assert_eq!(
        std::fs::read_to_string(settings.join(ANONYMOUS_ACCOUNT_FILE)).unwrap(),
        identity_json('k')
    );
    assert_eq!(
        AnonymousSessionStorage::new(&settings)
            .load()
            .unwrap()
            .unwrap()
            .tokens
            .user
            .id,
        "msime-keyboard00000000"
    );
    // 同一份发布物再采用一次是 unchanged，身份不再写。
    assert_eq!(
        adopt(&settings, request),
        json!({ "ok": true, "value": { "outcome": "unchanged" } })
    );

    // 键盘采用设置应用发布的另一个匿名账号、设置应用那边没有身份文件：键盘的身份属于原来的账号，删掉。
    let keyboard = root.path().join("keyboard").join("state");
    AnonymousSessionStorage::new(&keyboard)
        .save(&saved("msime-keyboard00000000", 'b'))
        .unwrap();
    private_write(
        &keyboard.join(ANONYMOUS_ACCOUNT_FILE),
        identity_json('k').as_bytes(),
    );
    let from_settings = root.path().join("group").join("settings").join("state");
    AnonymousSessionStorage::new(&from_settings)
        .save(&saved("msime-settings00000000", 'c'))
        .unwrap();
    assert_eq!(
        adopt(
            &keyboard,
            json!({
                "operation": "session_adopt",
                "path": from_settings.join(ANONYMOUS_SESSION_FILE),
                "identity_path": from_settings.join(ANONYMOUS_ACCOUNT_FILE),
                "item": "anonymous",
                "from": "settings",
            })
        ),
        json!({ "ok": true, "value": { "outcome": "adopted" } })
    );
    assert!(!keyboard.join(ANONYMOUS_ACCOUNT_FILE).exists());
}

#[test]
fn a_bad_identity_path_is_refused_without_touching_anything() {
    let root = tempfile::tempdir().unwrap();
    let published = root.path().join("group").join("keyboard").join("state");
    let settings = root.path().join("settings").join("state");
    AnonymousSessionStorage::new(&published)
        .save(&saved("msime-keyboard00000000", 'b'))
        .unwrap();
    private_write(
        &settings.join(ANONYMOUS_ACCOUNT_FILE),
        identity_json('s').as_bytes(),
    );
    // 发布的身份不合法：account_storage，会话和身份都不动。
    private_write(
        &published.join(ANONYMOUS_ACCOUNT_FILE),
        br#"{"subject":"someone","secret":"x"}"#,
    );
    assert_eq!(
        adopt(
            &settings,
            json!({
                "operation": "session_adopt",
                "path": published.join(ANONYMOUS_SESSION_FILE),
                "identity_path": published.join(ANONYMOUS_ACCOUNT_FILE),
                "item": "anonymous",
                "from": "keyboard",
            })
        ),
        json!({ "ok": false, "error": "account_storage" })
    );
    assert!(!settings.join(ANONYMOUS_SESSION_FILE).exists());
    assert_eq!(
        std::fs::read_to_string(settings.join(ANONYMOUS_ACCOUNT_FILE)).unwrap(),
        identity_json('s')
    );
    // 身份只跟着匿名会话走：和登录会话一起给、或者给相对路径，都是请求不合法。
    let invalid = json!({ "ok": false, "error": "account_invalid" });
    for action in [
        json!({ "operation": "session_adopt", "identity_path": published.join(ANONYMOUS_ACCOUNT_FILE), "item": "account", "from": "keyboard" }),
        json!({ "operation": "session_adopt", "identity_path": "relative/anonymous-account.json", "item": "anonymous", "from": "keyboard" }),
    ] {
        assert_eq!(adopt(&settings, action.clone()), invalid, "{action}");
    }
}
