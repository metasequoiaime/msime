//! 状态文件的测试：读写往返、两个描述符互斥、损坏和超长时按全新处理、租约与提交、旧版本导入。只用合成数据。

use super::*;
use crate::account::AccountPreferenceValue;

fn store() -> (tempfile::TempDir, SyncStateStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = SyncStateStore::new(directory.path());
    (directory, store)
}

fn bound(account: &str) -> SyncState {
    let mut state = SyncState::default();
    state.reconcile(Some(account));
    state
}

#[test]
fn a_missing_file_reads_as_a_fresh_state_and_writes_round_trip() {
    let (_directory, store) = store();
    assert_eq!(store.read().unwrap(), SyncState::default());
    store
        .update(|state| {
            state.reconcile(Some("synthetic-user"));
            state.set_enabled(true).unwrap();
            state.sections.settings.cursor = Some(7);
            state.sections.settings.base = Some(SectionBase::Keys(
                [("input.schema".to_owned(), "ab".repeat(32))].into(),
            ));
            state.sections.phrases.base = Some(SectionBase::Texts(["cd".repeat(32)].into()));
            state.sections.phrases.unheld = vec!["ef".repeat(32)];
            state.pending_choice = Some(Choice::UseCloud);
            state.not_before_ms = Some(42);
        })
        .unwrap();
    let state = store.read().unwrap();
    assert!(state.enabled);
    assert_eq!(state.account_id, "synthetic-user");
    assert_eq!(state.sections.settings.cursor, Some(7));
    assert!(matches!(
        state.sections.settings.base,
        Some(SectionBase::Keys(_))
    ));
    assert!(matches!(
        state.sections.phrases.base,
        Some(SectionBase::Texts(_))
    ));
    assert_eq!(state.sections.phrases.unheld.len(), 1);
    assert_eq!(state.pending_choice, Some(Choice::UseCloud));
    assert_eq!(state.not_before_ms, Some(42));
    // 写出的是第 2 版，文件里只有摘要。
    let text = std::fs::read_to_string(store.directory().join("state.json")).unwrap();
    assert!(text.contains("\"version\":2"));
}

#[test]
fn a_corrupt_oversized_or_old_file_reads_as_fresh() {
    let (_directory, store) = store();
    store.update(|state| state.reconcile(Some("u"))).unwrap();
    let path = store.directory().join("state.json");
    std::fs::write(&path, b"{not json").unwrap();
    assert_eq!(store.read().unwrap(), SyncState::default());
    std::fs::write(&path, br#"{"version":1,"enabled":true}"#).unwrap();
    assert_eq!(store.read().unwrap(), SyncState::default());
    std::fs::write(&path, vec![b' '; MAX_STATE_BYTES + 1]).unwrap();
    assert_eq!(store.read().unwrap(), SyncState::default());
    // 新版本多出来的键不让旧版本把状态当成损坏。
    std::fs::write(
        &path,
        br#"{"version":2,"enabled":true,"account_id":"u","future":1}"#,
    )
    .unwrap();
    assert!(store.read().unwrap().enabled);
}

#[cfg(unix)]
#[test]
fn the_state_lock_excludes_a_second_descriptor() {
    let (_directory, store) = store();
    let held = store.lock().unwrap();
    let second = file_lock::open_private_lock_file(store.directory().join("state.lock")).unwrap();
    assert!(file_lock::ScopedFileLock::try_exclusive(second)
        .unwrap()
        .is_none());
    drop(held);
    let second = file_lock::open_private_lock_file(store.directory().join("state.lock")).unwrap();
    assert!(file_lock::ScopedFileLock::try_exclusive(second)
        .unwrap()
        .is_some());
}

#[test]
fn reset_keeps_the_generations_increasing() {
    let mut state = bound("a");
    state.set_enabled(true).unwrap();
    state.sections.phrases.cursor = Some(3);
    let version = state.state_version;
    let binding = state.binding_generation;
    assert!(state.reconcile(Some("b")));
    assert!(!state.enabled);
    assert_eq!(state.sections.phrases.cursor, None);
    assert!(state.state_version > version);
    assert_eq!(state.binding_generation, binding + 1);
    // 同一个账号重新登录保留进度。
    state.sections.phrases.cursor = Some(4);
    assert!(!state.reconcile(Some("b")));
    assert_eq!(state.sections.phrases.cursor, Some(4));
    assert!(state.reconcile(None));
    assert!(state.account_id.is_empty());
}

#[test]
fn enabling_needs_an_account_and_marks_every_section() {
    let mut state = SyncState::default();
    assert_eq!(state.set_enabled(true), Err(NotSignedIn));
    let mut state = bound("a");
    assert!(!state.mark_dirty(Section::Phrases));
    state.set_enabled(true).unwrap();
    assert!(Section::ALL
        .iter()
        .all(|section| state.sections.get(*section).dirty()));
    let version = state.state_version;
    assert!(state.mark_dirty(Section::Settings));
    assert_eq!(state.sections.settings.generation, 2);
    // 打标记不推进版本，进行中的那一轮照样能提交。
    assert_eq!(state.state_version, version);
}

#[test]
fn a_lease_blocks_until_it_expires_and_commits_only_when_nothing_else_wrote() {
    let mut state = bound("a");
    state.set_enabled(true).unwrap();
    let lease = state.begin_run("a", 1_000, None);
    assert!(state.lease_active(1_000));
    assert!(state.lease_active(1_000 + LEASE_MS - 1));
    assert!(!state.lease_active(1_000 + LEASE_MS));
    // 时钟往回拨时不再挡住。
    assert!(!state.lease_active(999));
    assert!(state.lease_matches(&lease.id, "a"));
    assert!(!state.lease_matches(&lease.id, "b"));
    assert!(!state.lease_matches("other", "a"));
    state.mark_dirty(Section::Phrases);
    assert!(state.lease_matches(&lease.id, "a"));
    state.set_enabled(false).unwrap();
    assert!(!state.lease_matches(&lease.id, "a"));
}

#[test]
fn finishing_a_round_advances_only_what_the_round_saw() {
    let mut state = bound("a");
    state.set_enabled(true).unwrap();
    let read = state.sections.phrases.generation;
    let lease = state.begin_run("a", 5_000, Some(Choice::Merge));
    state.pending_choice = Some(Choice::Merge);
    // 这一轮进行中用户又改了一次常用语。
    state.mark_dirty(Section::Phrases);
    let mut progress = RoundProgress {
        synced: true,
        consumed_choice: true,
        ..RoundProgress::default()
    };
    progress.sections.insert(
        Section::Phrases,
        SectionProgress {
            cursor: Some(9),
            clean_to: Some(read),
            mark_dirty: false,
            base: BaseChange::Set(SectionBase::Texts(["ab".repeat(32)].into())),
            unheld: Some(vec!["cd".repeat(32)]),
            cleared: Some(false),
            outcome: SectionOutcome::new(OutcomeKind::Synced, None),
        },
    );
    assert!(state.lease_matches(&lease.id, "a"));
    state.finish_round(&progress, 6_000);
    let phrases = &state.sections.phrases;
    assert_eq!(phrases.cursor, Some(9));
    assert!(
        phrases.dirty(),
        "the edit made during the round stays marked"
    );
    assert_eq!(phrases.unheld.len(), 1);
    assert_eq!(state.last_synced_ms, Some(6_000));
    assert_eq!(state.pending_choice, None);
    assert!(state.run.is_none());
    assert!(!state.lease_matches(&lease.id, "a"));
}

#[test]
fn a_backoff_is_recorded_and_cleared_once_it_passes() {
    let mut state = bound("a");
    state.begin_run("a", 1_000, None);
    state.finish_round(
        &RoundProgress {
            not_before_ms: Some(61_000),
            failure: Some(SyncFailure {
                section: Section::Phrases,
                reason: "rate_limit_exceeded".into(),
                status: Some(429),
                code: Some("rate_limit_exceeded".into()),
                message: "同步太频繁，稍后自动重试".into(),
            }),
            ..RoundProgress::default()
        },
        1_000,
    );
    assert_eq!(state.not_before_ms, Some(61_000));
    assert!(state.failure.is_some());
    state.begin_run("a", 70_000, None);
    state.finish_round(
        &RoundProgress {
            synced: true,
            ..RoundProgress::default()
        },
        70_000,
    );
    assert_eq!(state.not_before_ms, None);
    assert_eq!(state.failure, None);
}

#[test]
fn pending_progress_round_trips_and_is_not_written_when_the_change_fails() {
    let (_directory, store) = store();
    let document = AccountPreferences {
        revision: 3,
        settings: [(
            "input.schema".to_owned(),
            AccountPreferenceValue::String("wubi".into()),
        )]
        .into(),
    };
    store
        .update_with_pending(|state, pending| {
            state.reconcile(Some("a"));
            *pending = Some(PendingCommit {
                run_id: "run".into(),
                progress: RoundProgress::default(),
                settings_document: Some(document.clone()),
                modes: [(Section::Settings, Mode::Download)].into(),
            });
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
    let failed = store
        .update_with_pending(|state, pending| {
            state.reconcile(Some("b"));
            *pending = None;
            Err::<(), _>("refused")
        })
        .unwrap();
    assert_eq!(failed, Err("refused"));
    let (state, pending) = store
        .update_with_pending(|state, pending| Ok::<_, ()>((state.clone(), pending.clone())))
        .unwrap()
        .unwrap();
    assert_eq!(state.account_id, "a");
    let pending = pending.unwrap();
    assert_eq!(pending.settings_document, Some(document));
    assert_eq!(pending.modes[&Section::Settings], Mode::Download);
    store.update_always(|_, pending| *pending = None).unwrap();
    assert!(!store.directory().join("pending.json").exists());
}

#[test]
fn ios_v1_state_is_imported_with_cursors_marks_and_digests() {
    let record = serde_json::json!({
        "version": 1,
        "enabled": true,
        "accountID": "synthetic-user",
        "sessionID": "00000000-0000-4000-8000-000000000000",
        "bindingGeneration": 4,
        "cursors": {"settings": "12", "phrases": ""},
        "generations": {"settings": 3, "phrases": 1},
        "cleans": {"settings": 2, "phrases": 1},
        "unheldPhrases": ["很长的一条"],
        "lastAttempt": "2026-10-01T08:00:00Z",
        "lastSynced": "2026-10-01T07:59:00Z",
        "error": "网络不可用，稍后自动重试",
    });
    let state = import_ios_v1(&record).unwrap();
    assert!(state.enabled);
    assert_eq!(state.account_id, "synthetic-user");
    assert_eq!(state.sections.settings.cursor, Some(12));
    assert!(state.sections.settings.dirty());
    assert_eq!(state.sections.phrases.cursor, None);
    assert!(!state.sections.phrases.dirty());
    assert_eq!(
        state.sections.phrases.unheld,
        vec![super::super::policy::text_digest("很长的一条")]
    );
    assert_eq!(state.last_synced_ms, Some(1_790_841_540_000));
    assert!(state.sections.settings.base.is_none());
    assert!(import_ios_v1(&serde_json::json!({"version": 2})).is_none());
    assert!(import_ios_v1(&serde_json::json!("x")).is_none());
}

#[test]
fn android_preferences_are_imported_and_a_stale_status_is_ignored() {
    let sync = serde_json::json!({
        "enabled": true,
        "account_id": "synthetic-user",
        "login_kind": "google",
        "binding_generation": 6,
        "cursor_settings": "5",
        "cursor_skins": "5",
        "cursor_dictionary": "40",
        "gen_dictionary": 2,
        "clean_dictionary": 1,
        "phrases_unheld": ["一", "二"],
        "last_synced_at": 1_700_000_000_000u64,
    });
    let status = serde_json::json!({
        "binding_generation": 6,
        "last_attempt": 1_700_000_100_000u64,
        "prompted_at": 1_700_000_050_000u64,
        "skins_trimmed": true,
    });
    let state = import_android(&sync, &status).unwrap();
    assert!(state.enabled);
    assert_eq!(state.sections.dictionary.cursor, Some(40));
    assert!(state.sections.dictionary.dirty());
    assert_eq!(state.sections.skins.cursor, Some(5));
    assert_eq!(state.sections.phrases.unheld.len(), 2);
    assert_eq!(state.last_attempt_ms, Some(1_700_000_100_000));
    assert!(state.notices.skins_trimmed);
    let stale = serde_json::json!({"binding_generation": 5, "last_attempt": 1u64});
    let state = import_android(&sync, &stale).unwrap();
    assert_eq!(state.last_attempt_ms, None);
    let mut anonymous = sync.clone();
    anonymous["login_kind"] = serde_json::json!("anonymous");
    assert!(!import_android(&anonymous, &status).unwrap().enabled);
}

#[test]
fn adopting_an_import_keeps_the_generations_increasing() {
    let mut state = SyncState::default();
    state.bump();
    assert!(state.is_pristine());
    let version = state.state_version;
    let mut imported = bound("u");
    imported.sections.settings.cursor = Some(1);
    state.adopt(imported);
    assert_eq!(state.account_id, "u");
    assert!(state.state_version > version);
    assert!(!state.is_pristine());
}
