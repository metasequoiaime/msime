//! 清除前持久屏障失败时不能删记录，重试与离线认领仍遵守同一屏障。

use super::*;
use msime_client_services::account::learning_sync::state::StateStore;
use rusqlite::Connection;

const FUTURE: u64 = 10_000_000_000;

fn progress(harness: &Harness) -> StateStore {
    StateStore::new(harness._root.path())
}

fn account(harness: &Harness) -> &str {
    harness.login["user_id"].as_str().unwrap()
}

fn journal(harness: &Harness) -> Connection {
    Connection::open(harness.paths.user("msime_user.db")).unwrap()
}

fn floor(harness: &Harness) -> i64 {
    journal(harness)
        .query_row("SELECT reset_floor FROM learning_sync_clock", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap()
}

#[test]
fn c_reset_is_offline_without_keys_and_covers_local_and_observed_logical_time() {
    let harness = Harness::new();
    let mut local = state(4, FUTURE);
    // 槽位冲突中未显示的候选也属于本机已观察的时间。
    for (value, updated_at) in [("SyntheticA", FUTURE + 1), ("SyntheticB", FUTURE + 2)] {
        local
            .insert(LearningSyncRecord::Position {
                context: "synthetic".into(),
                key: "sample".into(),
                value: value.into(),
                position: 1,
                updated_at,
            })
            .unwrap();
    }
    msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
    progress(&harness)
        .observe(account(&harness), FUTURE + 3)
        .unwrap();
    let mut expired = harness.storage.load().unwrap().unwrap();
    expired.expires_at_unix_ms = 0;
    harness.storage.save(&expired).unwrap();
    assert_eq!(
        harness.call("learning_reset_local"),
        json!({"ok": true, "value": {"reset": true, "collections_requeued": 0}})
    );
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE + 3
    );
    assert_eq!(floor(&harness), (FUTURE + 3) as i64);
    assert_eq!(
        journal(&harness)
            .query_row(
                "SELECT count(*) FROM fixed_candidate_positions",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    // 走原有词库写入入口，新写的本机行必须跨过屏障。
    let request = json!({"options": harness.options, "action": {"operation": "edit",
        "replacement": {"kind": "english", "key": "fresh", "value": "Fresh", "weight": 2},
        "request_id": "synthetic-after-reset"}});
    crate::dictionary::dictionary_request_json(&serde_json::to_vec(&request).unwrap()).unwrap();
    assert_eq!(
        journal(&harness)
            .query_row(
                "SELECT updated_at FROM user_dictionary_operations WHERE key='fresh'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        (FUTURE + 4) as i64
    );
    assert!(harness.storage.load_learning_keystore().unwrap().is_none());
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn c_reset_uses_local_maximum_even_before_any_sync() {
    let harness = Harness::new();
    msime_engine::merge_learning_sync_state(&harness.paths, false, &state(4, FUTURE), 0).unwrap();
    assert_eq!(harness.call("learning_reset_local")["ok"], true);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE
    );
    assert_eq!(floor(&harness), FUTURE as i64);
    // 退出和同账号重登录不会清账号屏障；重复清除也不能降低它。
    assert_eq!(
        call(&harness.source, json!({"operation": "forget"}))["ok"],
        true
    );
    let mut saved: SavedAccountSession =
        serde_json::from_slice(&session_bytes("synthetic-user", 'b', true)).unwrap();
    saved.session_id = None;
    harness.storage.save(&saved).unwrap();
    let login = binding(&harness.source);
    assert_ne!(login, harness.login);
    assert_eq!(
        call(
            &harness.source,
            json!({"operation": "learning_reset_local",
        "login": login, "options": harness.options})
        )["ok"],
        true
    );
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE
    );
    assert_eq!(floor(&harness), FUTURE as i64);
}

#[test]
fn c_reset_rejects_stale_login_and_busy_before_saving_barrier_or_clearing() {
    let harness = Harness::new();
    let local = state(4, FUTURE);
    msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
    let lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &harness.paths.user_data,
        &harness.paths.dictionaries,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        harness.call("learning_reset_local"),
        json!({"ok": false, "error": "learning_busy"})
    );
    drop(lease);
    let mut saved = harness.storage.load().unwrap().unwrap();
    saved.session_id = Some(msime_client_core::uuid::Uuid::new_v4());
    harness.storage.save(&saved).unwrap();
    assert_eq!(
        harness.call("learning_reset_local"),
        json!({"ok": false, "error": "account_cancelled"})
    );
    assert!(!harness
        ._root
        .path()
        .join("cloud-sync/learning.json")
        .exists());
    assert_eq!(harness.exported(), local);
    assert!(harness.server.state.lock().unwrap().trace.is_empty());
}

#[test]
fn c_reset_corrupt_or_unwritable_progress_never_clears_local_learning() {
    for corrupt in [false, true] {
        let harness = Harness::new();
        let local = state(4, FUTURE);
        msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
        let cloud = harness._root.path().join("cloud-sync");
        let bytes = b"synthetic-invalid-state";
        if corrupt {
            std::fs::create_dir(&cloud).unwrap();
            std::fs::write(cloud.join("learning.json"), bytes).unwrap();
        } else {
            std::fs::write(&cloud, bytes).unwrap();
        }
        let response = harness.call("learning_reset_local");
        assert_eq!(
            response,
            json!({"ok": false,
            "error": if corrupt {"learning_state_invalid"} else {"learning_state_unavailable"}})
        );
        assert_eq!(harness.exported(), local);
        let file = if corrupt {
            cloud.join("learning.json")
        } else {
            cloud
        };
        assert_eq!(std::fs::read(file).unwrap(), bytes);
        assert!(harness.storage.load_learning_keystore().unwrap().is_none());
        assert!(harness.server.state.lock().unwrap().trace.is_empty());
    }
}

#[test]
fn c_reset_clear_failure_keeps_durable_barrier_for_retry() {
    let harness = Harness::new();
    let local = state(4, FUTURE);
    msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
    // 资源项是目录，Engine 必须拒绝重置；不能借替换资源绕过错误。
    let packaged = harness.paths.resource("msime-english.db");
    std::fs::create_dir(&packaged).unwrap();
    assert_eq!(
        harness.call("learning_reset_local"),
        json!({"ok": false, "error": "learning_local_unavailable"})
    );
    assert_eq!(harness.exported(), local);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE
    );
    assert_eq!(floor(&harness), FUTURE as i64);
    std::fs::remove_dir(packaged).unwrap();
    assert_eq!(harness.call("learning_reset_local")["ok"], true);
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(floor(&harness), FUTURE as i64);
}

#[test]
fn c_reset_clock_failure_keeps_rows_and_saved_barrier_then_retries() {
    let harness = Harness::new();
    let local = state(4, FUTURE);
    msime_engine::merge_learning_sync_state(&harness.paths, false, &local, 0).unwrap();
    journal(&harness)
        .execute_batch(
            "CREATE TRIGGER synthetic_clock_failure BEFORE INSERT ON learning_sync_clock \
         BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;",
        )
        .unwrap();
    assert_eq!(
        harness.call("learning_reset_local"),
        json!({"ok": false, "error": "learning_local_unavailable"})
    );
    assert_eq!(harness.exported(), local);
    assert_eq!(
        progress(&harness)
            .read(account(&harness))
            .unwrap()
            .reset_floor,
        FUTURE
    );
    journal(&harness)
        .execute_batch("DROP TRIGGER synthetic_clock_failure")
        .unwrap();
    assert_eq!(harness.call("learning_reset_local")["ok"], true);
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(floor(&harness), FUTURE as i64);
}

#[test]
fn c_reset_filters_previously_accepted_pending_ciphertext_on_offline_merge() {
    let harness = Harness::new();
    harness.remote(2);
    harness.server.state.lock().unwrap().lease_on_download = Some(harness.paths.clone());
    assert_eq!(
        harness.call("learning_sync_once")["value"]["downloads"][0]["outcome"],
        "pending"
    );
    harness.server.state.lock().unwrap().lease.take();
    let count = harness.server.state.lock().unwrap().trace.len();
    assert_eq!(harness.call("learning_reset_local")["ok"], true);
    let response = harness.call("learning_merge_pending");
    assert_eq!(
        response["value"]["pending"][0]["outcome"], "applied",
        "{response}"
    );
    assert_eq!(harness.exported().records().len(), 0);
    assert_eq!(harness.server.state.lock().unwrap().trace.len(), count);
}
