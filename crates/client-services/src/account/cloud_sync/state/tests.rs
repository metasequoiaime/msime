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
            activation: ActivationChange::Keep,
            stall: StallChange::Keep,
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
fn the_snapshot_waiting_for_activation_is_recorded_kept_and_cleared() {
    let (_directory, store) = store();
    let mut state = bound("a");
    let round = |change: ActivationChange| {
        let mut progress = RoundProgress::default();
        progress.sections.insert(
            Section::Dictionary,
            SectionProgress {
                activation: change,
                ..SectionProgress::outcome(SectionOutcome::new(OutcomeKind::Synced, None))
            },
        );
        progress
    };
    state.finish_round(&round(ActivationChange::Set("request-1".into())), 1_000);
    assert_eq!(
        state.sections.dictionary.activation.as_deref(),
        Some("request-1")
    );
    // 状态文件和待提交进度都要带着它往返。
    store.update(|stored| *stored = state.clone()).unwrap();
    assert_eq!(store.read().unwrap(), state);
    let pending = PendingCommit {
        run_id: "run".into(),
        progress: round(ActivationChange::Clear),
        settings_document: None,
        settings_revision: None,
        modes: BTreeMap::new(),
    };
    let encoded = serde_json::to_vec(&pending).unwrap();
    assert_eq!(
        serde_json::from_slice::<PendingCommit>(&encoded).unwrap(),
        pending
    );
    state.finish_round(&round(ActivationChange::Keep), 2_000);
    assert!(state.sections.dictionary.activation.is_some());
    state.finish_round(&pending.progress, 3_000);
    assert_eq!(state.sections.dictionary.activation, None);
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
                settings_revision: Some(7),
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
    assert_eq!(pending.settings_revision, Some(7));
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
    let mut imported = bound("u");
    imported.sections.settings.cursor = Some(1);
    assert!(state.accepts_import(&imported));
    let version = state.state_version;
    state.adopt(imported.clone());
    assert_eq!(state.account_id, "u");
    assert!(state.state_version > version);
    assert!(!state.accepts_import(&imported));
}

#[test]
fn an_import_is_accepted_after_the_same_account_was_bound_but_never_synced() {
    let imported = bound("u");
    // 宿主先调过 status 或 run：对账把登录的账号写进来了，开关没开、没有游标。
    let state = bound("u");
    assert!(state.accepts_import(&imported));
    assert!(!bound("v").accepts_import(&imported));
    let mut synced = bound("u");
    synced.sections.phrases.cursor = Some(3);
    assert!(!synced.accepts_import(&imported));
}

#[test]
fn an_import_is_refused_once_the_user_switched_sync_or_an_import_was_adopted() {
    let mut imported = bound("u");
    imported.set_enabled(true).unwrap();
    imported.import_closed = false;
    // 打开过、第一轮还在等选择就关掉：状态与从没打开过一样（关着、没有游标），但不能再导入。
    let mut toggled = bound("u");
    toggled.set_enabled(true).unwrap();
    toggled.set_enabled(false).unwrap();
    assert!(!toggled.accepts_import(&imported));
    // 只关过一次也是用户自己定的。
    let mut disabled = bound("u");
    disabled.set_enabled(false).unwrap();
    assert!(!disabled.accepts_import(&imported));
    // 导入以后关掉：下次启动宿主再带同一份旧状态来，开关不会被改回打开。
    let mut adopted = bound("u");
    adopted.adopt(imported.clone());
    assert!(adopted.enabled);
    adopted.set_enabled(false).unwrap();
    assert!(!adopted.accepts_import(&imported));
    // 退出和换账号都不重新开放导入。
    let mut signed_out = SyncState::default();
    signed_out.adopt(imported.clone());
    signed_out.reset("");
    assert!(!signed_out.accepts_import(&imported));
    signed_out.reconcile(Some("u"));
    assert!(!signed_out.accepts_import(&imported));
}

#[test]
fn a_backoff_never_lasts_longer_than_a_day() {
    let mut state = bound("a");
    state.begin_run("a", 1_000, None);
    state.finish_round(
        &RoundProgress {
            not_before_ms: Some(u64::MAX),
            ..RoundProgress::default()
        },
        1_000,
    );
    assert_eq!(state.not_before_ms, Some(1_000 + MAX_BACKOFF_MS));
    assert_eq!(state.backoff_until(1_000), Some(1_000 + MAX_BACKOFF_MS));
    // 时钟往回拨以后，离现在超过一天的退避不再挡，下一轮提交时清掉。
    assert_eq!(state.backoff_until(0), None);
    state.begin_run("a", 0, None);
    state.finish_round(&RoundProgress::default(), 0);
    assert_eq!(state.not_before_ms, None);
}

#[test]
fn a_choice_kept_for_the_dictionary_and_its_backoff_are_recorded_and_cleared() {
    let mut state = bound("a");
    state.set_enabled(true).unwrap();
    let stall = Stall {
        rounds: 2,
        until_ms: 10_000,
        present: Some(3),
    };
    // 旧的状态文件里没有 `present`：读成 `None`；记下的词数原样往返。
    assert_eq!(
        serde_json::from_str::<Stall>(r#"{"rounds":2,"until_ms":10000}"#).unwrap(),
        Stall {
            present: None,
            ..stall
        }
    );
    assert_eq!(
        serde_json::from_str::<Stall>(&serde_json::to_string(&stall).unwrap()).unwrap(),
        stall
    );
    let mut progress = RoundProgress {
        consumed_choice: true,
        kept_choice: Some(Choice::UseCloud),
        ..RoundProgress::default()
    };
    progress.sections.insert(
        Section::Dictionary,
        SectionProgress {
            stall: StallChange::Set(stall),
            ..SectionProgress::outcome(SectionOutcome::new(
                OutcomeKind::Failed,
                Some("dictionary_merge_pending"),
            ))
        },
    );
    state.finish_round(&progress, 5_000);
    // 留下的选择只给词库，不进 `pending_choice`。
    assert_eq!(state.pending_choice, None);
    assert_eq!(state.sections.dictionary.choice, Some(Choice::UseCloud));
    assert_eq!(state.sections.dictionary.stall, Some(stall));
    assert!(state.sections.dictionary.stalled(9_999));
    assert!(!state.sections.dictionary.stalled(10_000));
    // 时钟往回拨以前记下的、离现在超过上限的退避不再挡。
    let far = Stall {
        rounds: 1,
        until_ms: 1_000 + MAX_STALL_MS + 1,
        present: None,
    };
    let mut rewound = state.sections.dictionary.clone();
    rewound.stall = Some(far);
    assert!(!rewound.stalled(1_000));
    assert!(rewound.stalled(1_001));
    // 词库推进了游标：选择用完，退避清掉。
    let mut progress = RoundProgress::default();
    progress.sections.insert(
        Section::Dictionary,
        SectionProgress {
            cursor: Some(4),
            stall: StallChange::Clear,
            ..SectionProgress::outcome(SectionOutcome::new(OutcomeKind::Synced, None))
        },
    );
    state.finish_round(&progress, 6_000);
    assert_eq!(state.sections.dictionary.choice, None);
    assert_eq!(state.sections.dictionary.stall, None);
}

#[test]
fn disabling_on_a_cleared_cloud_keeps_the_progress_drops_stored_choices_and_persists() {
    let (_directory, store) = store();
    store
        .update(|state| {
            state.reconcile(Some("synthetic-user"));
            state.set_enabled(true).unwrap();
            state.sections.phrases.cursor = Some(4);
            state.sections.phrases.cleared = true;
            state.sections.dictionary.cursor = Some(9);
            state.sections.dictionary.choice = Some(Choice::UseCloud);
            state.pending_choice = Some(Choice::Merge);
            state.begin_run("synthetic-user", 1_000, Some(Choice::Merge));
        })
        .unwrap();
    let before = store.read().unwrap();
    let run = before.run.clone().unwrap();
    assert!(before.lease_matches(&run.id, "synthetic-user"));
    store.update(SyncState::disable_keeping_local).unwrap();
    let state = store.read().unwrap();
    assert!(!state.enabled);
    assert!(state.disabled_on_cloud_cleared);
    assert!(state.import_closed);
    // 清空标记、游标和代数都留着，重新打开以后先问；记着的旧选择丢掉。
    assert!(state.sections.phrases.cleared);
    assert_eq!(state.sections.phrases.cursor, Some(4));
    assert_eq!(state.sections.dictionary.cursor, Some(9));
    assert_eq!(
        state.sections.phrases.generation,
        before.sections.phrases.generation
    );
    assert_eq!(state.pending_choice, None);
    assert_eq!(state.sections.dictionary.choice, None);
    // 在跑的那一轮提交不了。
    assert!(state.state_version > before.state_version);
    assert!(!state.lease_matches(&run.id, "synthetic-user"));
    let text = std::fs::read_to_string(store.directory().join("state.json")).unwrap();
    assert!(
        text.contains("\"disabled_on_cloud_cleared\":true"),
        "{text}"
    );

    // 自己再开关一次、换账号或退出都清掉它。
    let mut reopened = state.clone();
    reopened.set_enabled(true).unwrap();
    assert!(!reopened.disabled_on_cloud_cleared);
    assert!(reopened.sections.phrases.cleared);
    let mut switched_off = state.clone();
    switched_off.set_enabled(false).unwrap();
    assert!(!switched_off.disabled_on_cloud_cleared);
    let mut signed_out = state.clone();
    signed_out.reset("");
    assert!(!signed_out.disabled_on_cloud_cleared);
    // 没有这个字段的旧文件读成 false，为 false 时也不写出来。
    let text = serde_json::to_string(&reopened).unwrap();
    assert!(!text.contains("disabled_on_cloud_cleared"), "{text}");
    let old: SyncState = serde_json::from_str(&text).unwrap();
    assert!(!old.disabled_on_cloud_cleared);
}
