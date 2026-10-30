//! C 层只测不联网的路径：请求校验、只碰状态文件的操作、开关和退出登录、旧状态导入，以及按目录和按注入存储取会话。联网的一轮在 crate 的 `cloud_sync` 里用假的后端测。只用合成数据。

use super::*;
use msime_client_services::account::{
    AccountSessionFileLayout, AccountSessionStorage, AccountTokens, AccountUser,
    FileAccountSessionStorage, SavedAccountSession,
};
use std::ffi::CStr;

fn call(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 缓冲区在调用期间有效，返回的字符串在下面释放。
    unsafe {
        let raw = msime_client_cloud_sync(bytes.as_ptr(), bytes.len());
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    }
}

/// 一份未过期的合成会话，读状态和对账都不需要联网。
fn save_session(directory: &Path, user: &str) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native)
        .save(&SavedAccountSession {
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
            expires_at_unix_ms: now + 3_600_000,
            session_id: None,
        })
        .unwrap();
}

struct Paths {
    _root: tempfile::TempDir,
    account: PathBuf,
    state: PathBuf,
    preferences: PathBuf,
}

fn paths() -> Paths {
    let root = tempfile::tempdir().unwrap();
    let account = root.path().join("account");
    let state = root.path().join("state");
    let preferences = root.path().join("preferences");
    for directory in [&account, &state, &preferences] {
        std::fs::create_dir_all(directory).unwrap();
    }
    Paths {
        _root: root,
        account,
        state,
        preferences,
    }
}

fn request(paths: &Paths, action: Value) -> Value {
    json!({
        "directory": paths.account,
        "state_directory": paths.state,
        "preferences_directory": paths.preferences,
        "action": action,
    })
}

/// 一份合规的 iOS 键盘设置，字段同 `msime_client_account_ios_settings_export` 的 `ios_native`。
fn ios_native() -> Value {
    json!({
        "inputScheme": "quanpin",
        "traditionalChineseOutput": false,
        "soundEnabled": true,
        "hapticsEnabled": false,
        "hapticStrength": "medium",
        "englishSuggestions": true,
        "candidatePaletteFollowsDesktop": false,
        "inlinePreedit": false,
        "hapticsAvailable": true,
        "dictionaryLearning": true,
        "globalTheme": "shuishan",
        "customKeyboardSkin": null,
    })
}

#[test]
fn malformed_requests_are_cloud_sync_invalid() {
    let paths = paths();
    let run = json!({
        "operation": "run",
        "trigger": "manual",
        "capabilities": { "phrases": true },
    });
    let invalid = [
        json!({ "directory": paths.account, "action": { "operation": "status" } }),
        json!({ "directory": paths.account, "state_directory": "relative", "action": { "operation": "status" } }),
        json!({ "state_directory": paths.state, "action": { "operation": "status" } }),
        json!({ "directory": paths.account, "session_store": 1, "state_directory": paths.state, "action": { "operation": "status" } }),
        request(&paths, json!({ "operation": "later" })),
        request(&paths, json!({ "operation": "status", "extra": 1 })),
        request(
            &paths,
            json!({ "operation": "mark_dirty", "section": "contacts" }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "choice", "capabilities": {} }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "dictionary": true } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "skins_library": true } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "phrases": true }, "host_inputs": { "unknown": 1 } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "settings": true }, "host_inputs": { "harmony": true, "ios_native": ios_native() } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "settings": true }, "host_inputs": { "harmony": true, "android_local": {} } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "settings": true }, "host_inputs": { "harmony": true, "custom_keyboard_skins": "[]" } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "capabilities": { "settings": true, "skins_library": true }, "host_inputs": { "harmony": true } }),
        ),
        // 「关闭同步」只回答提示，只能跟 `trigger:"choice"` 一起来；不认识的回答也不收。
        request(
            &paths,
            json!({ "operation": "run", "trigger": "manual", "choice": "disable", "capabilities": { "phrases": true } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "active", "choice": "disable", "capabilities": { "phrases": true } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "enable", "choice": "disable", "capabilities": { "phrases": true } }),
        ),
        request(
            &paths,
            json!({ "operation": "run", "trigger": "choice", "choice": "keep_local", "capabilities": { "phrases": true } }),
        ),
        json!({ "directory": paths.account, "state_directory": paths.state, "action": run }),
        request(&paths, json!({ "operation": "import_legacy" })),
        json!({ "state_directory": paths.state, "snapshot_queue_directory": "relative", "action": { "operation": "import_legacy", "ios_v1": { "version": 1 } } }),
    ];
    for request in invalid {
        assert_eq!(call(request.clone())["error"], INVALID, "{request}");
    }
    // NULL 和超长的缓冲区。
    // SAFETY: NULL 被入口拒绝，不会读。
    let raw = unsafe { msime_client_cloud_sync(std::ptr::null(), 0) };
    // SAFETY: 入口返回的字符串在这里读取并释放。
    let value: Value = unsafe {
        let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
        msime_client_string_free(raw);
        serde_json::from_str(&text).unwrap()
    };
    assert_eq!(value["error"], INVALID);
}

#[test]
fn marking_needs_no_session_and_only_counts_while_sync_is_on() {
    let paths = paths();
    let mark = json!({
        "state_directory": paths.state,
        "action": { "operation": "mark_dirty", "section": "phrases" },
    });
    assert_eq!(
        call(mark.clone()),
        json!({ "ok": true, "value": { "marked": false } })
    );
    save_session(&paths.account, "synthetic-user");
    let enabled = call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": true }),
    ));
    assert_eq!(enabled["value"]["enabled"], true, "{enabled}");
    assert_eq!(enabled["value"]["account_id"], "synthetic-user");
    assert_eq!(call(mark)["value"]["marked"], true);
    let status = call(request(&paths, json!({ "operation": "status" })));
    assert_eq!(status["value"]["sections"]["phrases"]["dirty"], true);
    assert_eq!(status["value"]["status_line"], "云同步已开启 · 尚未同步");
    // 状态文件在 state_directory/cloud-sync 下面。
    assert!(paths.state.join("cloud-sync/state.json").is_file());
}

/// 清除学习记录以后的 `dictionary_reset` 和 `mark_dirty` 一样不要会话：开关关着时不记，开着时给词库记一次改动；带上快照队列目录时一并取消排着的快照（这里队列是空的）。
#[test]
fn a_dictionary_reset_needs_no_session_and_marks_the_dictionary() {
    let paths = paths();
    let reset = json!({
        "state_directory": paths.state,
        "snapshot_queue_directory": paths.preferences.join("dictionary-snapshots"),
        "action": { "operation": "dictionary_reset" },
    });
    assert_eq!(
        call(reset.clone()),
        json!({ "ok": true, "value": { "marked": false } })
    );
    save_session(&paths.account, "synthetic-user");
    let enabled = call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": true }),
    ));
    assert_eq!(enabled["value"]["enabled"], true, "{enabled}");
    assert_eq!(
        call(reset),
        json!({ "ok": true, "value": { "marked": true } })
    );
    let status = call(request(&paths, json!({ "operation": "status" })));
    assert_eq!(status["value"]["sections"]["dictionary"]["dirty"], true);
}

#[test]
fn enabling_needs_an_account_and_sign_out_turns_sync_off() {
    let paths = paths();
    assert_eq!(
        call(request(
            &paths,
            json!({ "operation": "set_enabled", "enabled": true })
        ))["error"],
        "cloud_sync_signed_out"
    );
    let status = call(request(&paths, json!({ "operation": "status" })));
    assert_eq!(status["value"]["enabled"], false);
    assert_eq!(status["value"]["account_id"], Value::Null);
    assert_eq!(status["value"]["status_line"], "云同步已关闭");
    save_session(&paths.account, "synthetic-user");
    call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": true }),
    ));
    // 关着的同步不联网就返回。
    call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": false }),
    ));
    let skipped = call(request(
        &paths,
        json!({ "operation": "run", "trigger": "manual", "capabilities": { "phrases": true } }),
    ));
    assert_eq!(skipped["value"]["outcome"], "skipped", "{skipped}");
    assert_eq!(skipped["value"]["reason"], "disabled");
    call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": true }),
    ));
    let signed_out = call(request(&paths, json!({ "operation": "sign_out" })));
    assert_eq!(signed_out["value"]["enabled"], false, "{signed_out}");
    assert_eq!(signed_out["value"]["snapshots_cancelled"], true);
    // 没有待提交的一轮：提交是 stale_run。
    let stale = call(request(
        &paths,
        json!({ "operation": "commit", "run_id": "00000000-0000-4000-8000-000000000000" }),
    ));
    assert_eq!(stale["error"], "stale_run");
}

#[test]
fn legacy_android_state_is_imported_without_a_session() {
    let paths = paths();
    let imported = call(json!({
        "state_directory": paths.state,
        "action": {
            "operation": "import_legacy",
            "android": {
                "sync_v1": {
                    "enabled": true,
                    "account_id": "synthetic-user",
                    "login_kind": "email",
                    "binding_generation": 1,
                    "cursor_phrases": "3",
                },
                "cloud_sync_status": { "binding_generation": 1, "last_attempt": 5 },
            },
        },
    }));
    assert_eq!(imported["value"]["imported"], true, "{imported}");
    assert_eq!(
        imported["value"]["status"]["sections"]["phrases"]["cursor"],
        3
    );
    // 会话是同一个账号：对账以后进度留着。
    save_session(&paths.account, "synthetic-user");
    let status = call(request(&paths, json!({ "operation": "status" })));
    assert_eq!(status["value"]["enabled"], true);
    assert_eq!(status["value"]["sections"]["phrases"]["cursor"], 3);
    // 换了账号：清空。
    let other = paths.account.join("other");
    std::fs::create_dir_all(&other).unwrap();
    save_session(&other, "someone-else");
    let switched = call(json!({
        "directory": other,
        "state_directory": paths.state,
        "action": { "operation": "status" },
    }));
    assert_eq!(switched["value"]["account_id"], "someone-else");
    assert_eq!(
        switched["value"]["sections"]["phrases"]["cursor"],
        Value::Null
    );
}

#[test]
fn an_injected_store_supplies_the_account() {
    use super::super::session_store::tests::{leaked_store, registered, session_bytes, unregister};
    let paths = paths();
    let store = leaked_store();
    let id = registered(store, 1, 0);
    store
        .items
        .lock()
        .unwrap()
        .insert(1, session_bytes("synthetic-user", 'a', true));
    let status = call(json!({
        "session_store": id,
        "state_directory": paths.state,
        "action": { "operation": "status" },
    }));
    assert_eq!(status["value"]["account_id"], "synthetic-user", "{status}");
    unregister(id);
    let gone = call(json!({
        "session_store": id,
        "state_directory": paths.state,
        "action": { "operation": "status" },
    }));
    assert_eq!(gone["error"], INVALID);
}

#[test]
fn a_process_without_the_login_reads_the_switch_with_peek_and_clears_nothing() {
    let paths = paths();
    let peek = |directory: Option<&Path>| {
        let mut request = json!({
            "state_directory": paths.state,
            "action": { "operation": "peek" },
        });
        if let Some(directory) = directory {
            request["directory"] = json!(directory);
        }
        call(request)
    };
    // 还没有状态：全新的默认值，不建状态目录。
    let fresh = peek(None);
    assert_eq!(fresh["value"]["enabled"], false, "{fresh}");
    assert!(!paths.state.join("cloud-sync").exists());
    save_session(&paths.account, "synthetic-user");
    call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": true }),
    ));
    // Android 的 :ime 进程：给的会话目录里看不到登录。只读，不对账，共享的进度和开关都留着。
    let ime = paths.account.join("ime");
    std::fs::create_dir_all(&ime).unwrap();
    for directory in [Some(ime.as_path()), None] {
        let seen = peek(directory);
        assert_eq!(seen["value"]["enabled"], true, "{seen}");
        assert_eq!(seen["value"]["account_id"], "synthetic-user");
    }
    let owner = call(request(&paths, json!({ "operation": "status" })));
    assert_eq!(owner["value"]["enabled"], true, "{owner}");
    assert_eq!(owner["value"]["sections"]["phrases"]["dirty"], true);
}

#[test]
fn turning_sync_off_on_a_cleared_cloud_goes_through_run_and_shows_in_peek() {
    let paths = paths();
    save_session(&paths.account, "synthetic-user");
    call(request(
        &paths,
        json!({ "operation": "set_enabled", "enabled": true }),
    ));
    // 不联网，也不读写本机，所以不带 preferences_directory 也行。
    let disable = json!({
        "directory": paths.account,
        "state_directory": paths.state,
        "action": { "operation": "run", "trigger": "choice", "choice": "disable", "capabilities": { "phrases": true } },
    });
    let refused = call(disable.clone());
    assert_eq!(refused["value"]["outcome"], "skipped", "{refused}");
    assert_eq!(refused["value"]["reason"], "not_cloud_cleared");
    assert_eq!(refused["value"]["status"]["enabled"], true);
    // 上一轮发现云端被清空、记在了状态里。
    SyncStateStore::new(&paths.state)
        .update(|state| state.sections.phrases.cleared = true)
        .unwrap();
    let disabled = call(disable);
    assert_eq!(disabled["value"]["outcome"], "disabled", "{disabled}");
    assert_eq!(disabled["value"]["status"]["enabled"], false);
    let peeked = call(json!({
        "state_directory": paths.state,
        "action": { "operation": "peek" },
    }));
    assert_eq!(peeked["value"]["enabled"], false, "{peeked}");
    assert_eq!(peeked["value"]["disabled_on_cloud_cleared"], true);
    assert_eq!(peeked["value"]["sections"]["phrases"]["cleared"], true);
    assert_eq!(
        peeked["value"]["status_line"],
        "云端数据已被清空，已关闭同步，本机数据没有改动"
    );
}
