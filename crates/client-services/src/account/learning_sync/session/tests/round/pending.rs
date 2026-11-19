use super::*;

fn queued() -> Pair {
    let pair = Pair::new();
    pair.remote(2, &[weight(9, 100)], "synthetic-v1");
    let paths = pair.paths.clone();
    let held = Arc::new(Mutex::new(None));
    let captured = held.clone();
    let session = pair.after_blob("download:", move || {
        *captured.lock().unwrap() = crate::dictionary::access::DictionaryAccess::try_session(
            &paths.user_data,
            &paths.dictionaries,
        )
        .unwrap();
    });
    assert_eq!(
        session
            .learning_sync_once(&pair.login, "合成 A", &pair.context())
            .unwrap()
            .downloads[0]
            .outcome,
        DownloadOutcome::Pending
    );
    held.lock().unwrap().take();
    pair
}

fn file(pair: &Pair) -> std::path::PathBuf {
    std::fs::read_dir(pair.state.join("cloud-sync/learning-pending"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|ext| ext == "pending" || ext == "claimed")
        })
        .unwrap()
}

#[test]
fn queued_reset_barrier_is_reread_and_observed_time_survives_cache_removal() {
    let pair = queued();
    let store = StateStore::new(&pair.state);
    assert_eq!(store.read(&pair.login.user_id).unwrap().observed_max, 100);
    assert_eq!(
        store.prepare_reset(&pair.login.user_id, 50, 0).unwrap(),
        100
    );
    assert_eq!(
        pair.a
            .learning_merge_pending(&pair.login, &pair.context())
            .unwrap()[0]
            .outcome,
        DownloadOutcome::Applied
    );
    assert_eq!(pair.exported().records().len(), 0);
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 2,
            applied: 2
        }
    );
    assert_eq!(store.read(&pair.login.user_id).unwrap().reset_floor, 100);
}

#[test]
fn queue_contains_original_ciphertext_and_corruption_never_advances_applied() {
    let pair = queued();
    let path = file(&pair);
    let mut bytes = std::fs::read(&path).unwrap();
    let source = pair.keys(&pair.storage_b).device_id();
    let original = pair.server.lock().unwrap().round.as_ref().unwrap().blobs[&source]
        .1
        .clone();
    assert_eq!(&bytes[15..], original);
    assert_eq!(&bytes[15..19], b"MSL1");
    bytes[15 + 70] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        pair.a
            .learning_merge_pending(&pair.login, &pair.context())
            .unwrap()[0]
            .outcome,
        DownloadOutcome::Undecryptable
    );
    assert!(!path.exists());
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 2,
            applied: 0
        }
    );
    assert_eq!(pair.exported().records().len(), 0);
    assert_eq!(
        StateStore::new(&pair.state)
            .read(&pair.login.user_id)
            .unwrap()
            .observed_max,
        100
    );
}

#[test]
fn invalid_local_header_is_discarded_without_applying_or_forgetting_verified() {
    let pair = queued();
    let path = file(&pair);
    std::fs::write(&path, b"synthetic broken queue").unwrap();
    assert_eq!(
        pair.a
            .learning_merge_pending(&pair.login, &pair.context())
            .unwrap()[0]
            .outcome,
        DownloadOutcome::Undecryptable
    );
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 2,
            applied: 0
        }
    );
    assert!(!path.exists());
}

#[test]
fn valid_ciphertext_injected_without_verified_progress_cannot_promote_itself() {
    let pair = queued();
    let path = file(&pair);
    let mut stream = Vec::new();
    msime_engine::write_learning_sync(&state(&[weight(99, 900)]), "synthetic-v1", &mut stream)
        .unwrap();
    let envelope = pair.keys(&pair.storage_b).seal_blob(100, &stream).unwrap();
    let mut bytes = std::fs::read(&path).unwrap()[..15].to_vec();
    bytes[7..15].copy_from_slice(&100u64.to_le_bytes());
    bytes.extend_from_slice(&envelope);
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(
        pair.a
            .learning_merge_pending(&pair.login, &pair.context())
            .unwrap()[0]
            .outcome,
        DownloadOutcome::Undecryptable
    );
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 2,
            applied: 0
        }
    );
    assert_eq!(
        StateStore::new(&pair.state)
            .read(&pair.login.user_id)
            .unwrap()
            .observed_max,
        100
    );
    assert_eq!(pair.exported().records().len(), 0);
}

#[test]
fn third_failed_merge_abandons_only_ciphertext_and_never_acknowledges_application() {
    let pair = queued();
    let english = pair.paths.dictionary("msime-english.db");
    std::fs::remove_file(&english).unwrap();
    for expected in [
        DownloadOutcome::MergeFailed,
        DownloadOutcome::MergeFailed,
        DownloadOutcome::Abandoned,
    ] {
        let restarted = super::super::session(&pair.server, &pair.storage_a);
        assert_eq!(
            restarted
                .learning_merge_pending(&pair.login, &pair.context())
                .unwrap()[0]
                .outcome,
            expected
        );
        assert_eq!(
            pair.progress(),
            RemoteProgress {
                verified: 2,
                applied: 0
            }
        );
    }
    msime_engine::ensure_english_schema(&english).unwrap();
    assert!(pair
        .a
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap()
        .is_empty());
    assert_eq!(pair.exported().records().len(), 0);
    assert_eq!(
        StateStore::new(&pair.state)
            .read(&pair.login.user_id)
            .unwrap()
            .observed_max,
        100
    );
}

#[test]
fn rekey_or_relogin_never_applies_an_old_bindings_queue() {
    for rekey in [false, true] {
        let pair = queued();
        if rekey {
            pair.a
                .learning_operation(Operation::Reset, "合成 A", Some(&pair.login))
                .unwrap();
            assert!(pair
                .a
                .learning_merge_pending(&pair.login, &pair.context())
                .unwrap()
                .is_empty());
        } else {
            let login = sign_in(&pair.a);
            assert!(matches!(
                pair.a.learning_merge_pending(&pair.login, &pair.context()),
                Err(RoundError::Account(AccountError::Cancelled))
            ));
            pair.a
                .learning_operation(Operation::Reset, "合成 A", Some(&login))
                .unwrap();
            assert!(pair
                .a
                .learning_merge_pending(&login, &pair.context())
                .unwrap()
                .is_empty());
        }
        assert_eq!(pair.exported().records().len(), 0);
        assert_eq!(
            StateStore::new(&pair.state)
                .read(&pair.login.user_id)
                .unwrap()
                .observed_max,
            100
        );
    }
}

#[test]
fn newer_verified_pending_supersedes_an_old_claim_and_is_applied_in_the_same_idle_call() {
    use crate::account::learning_sync::pending::Pending;
    let pair = queued();
    let path = file(&pair);
    let binding = path.file_name().unwrap().to_str().unwrap()[..64].to_owned();
    let keys = pair.keys(&pair.storage_b);
    let source = keys.device_id();
    let queue = Pending::open(&pair.state, binding).unwrap();
    queue.claim(&source).unwrap().unwrap();
    let mut stream = Vec::new();
    let expected = state(&[weight(13, 300)]);
    msime_engine::write_learning_sync(&expected, "synthetic-v1", &mut stream).unwrap();
    let envelope = keys.seal_blob(3, &stream).unwrap();
    StateStore::new(&pair.state)
        .accept_verified(
            &pair.login.user_id,
            keys.ready_key_id().unwrap(),
            &source,
            3,
            300,
        )
        .unwrap();
    queue.publish(&source, 3, &envelope).unwrap();
    drop(queue);
    let reports = pair
        .a
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap();
    assert_eq!(reports.len(), 2);
    assert_eq!(
        (reports[0].generation, &reports[0].outcome),
        (Some(2), &DownloadOutcome::Rollback)
    );
    assert_eq!(
        (reports[1].generation, &reports[1].outcome),
        (Some(3), &DownloadOutcome::Applied)
    );
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 3,
            applied: 3
        }
    );
    assert_eq!(pair.exported(), expected);
}

#[test]
fn a_crash_after_recording_the_third_attempt_does_not_allow_a_fourth_merge() {
    let pair = queued();
    let path = file(&pair);
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[6] = 3;
    let claimed = path.with_extension("claimed");
    std::fs::write(&path, bytes).unwrap();
    std::fs::rename(&path, &claimed).unwrap();
    let restarted = super::super::session(&pair.server, &pair.storage_a);
    let reports = restarted
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap();
    assert_eq!(reports[0].outcome, DownloadOutcome::Abandoned);
    assert!(!claimed.exists());
    assert_eq!(pair.exported().records().len(), 0);
    assert_eq!(
        pair.progress(),
        RemoteProgress {
            verified: 2,
            applied: 0
        }
    );
}

#[test]
fn a_network_round_uses_the_queued_result_without_redownloading_or_retrying_twice() {
    let pair = queued();
    let english = pair.paths.dictionary("msime-english.db");
    std::fs::remove_file(&english).unwrap();
    for expected in [
        DownloadOutcome::MergeFailed,
        DownloadOutcome::MergeFailed,
        DownloadOutcome::Abandoned,
    ] {
        pair.edit_round(|round| round.trace.clear());
        let report = pair.run().unwrap();
        assert_eq!(report.downloads[0].generation, Some(2));
        assert_eq!(report.downloads[0].outcome, expected);
        assert_eq!(
            pair.progress(),
            RemoteProgress {
                verified: 2,
                applied: 0
            }
        );
        pair.edit_round(|round| {
            assert!(!round.trace.iter().any(|step| step.starts_with("download:")))
        });
    }
    // 放弃的是这份缓存；修复本机之后，同代次仍可重新下载并成功应用。
    msime_engine::ensure_english_schema(&english).unwrap();
    pair.edit_round(|round| round.trace.clear());
    assert_eq!(
        pair.run().unwrap().downloads[0].outcome,
        DownloadOutcome::Applied
    );
    pair.edit_round(|round| {
        assert_eq!(
            round
                .trace
                .iter()
                .filter(|step| step.starts_with("download:"))
                .count(),
            1
        )
    });
    assert_eq!(pair.exported(), state(&[weight(9, 100)]));
}

#[test]
fn expired_access_token_does_not_refresh_or_contact_any_service_during_idle_merge() {
    let pair = queued();
    let mut saved = pair.storage_a.load().unwrap().unwrap();
    saved.expires_at_unix_ms = 0;
    pair.storage_a.save(&saved).unwrap();
    let restarted = super::super::session(&pair.server, &pair.storage_a);
    let before = {
        let server = pair.server.lock().unwrap();
        (
            server.refreshes,
            server.requests.len(),
            server.blob_tokens.len(),
        )
    };
    let reports = restarted
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap();
    assert_eq!(reports[0].outcome, DownloadOutcome::Applied);
    assert_eq!(pair.exported(), state(&[weight(9, 100)]));
    let server = pair.server.lock().unwrap();
    assert_eq!(
        (
            server.refreshes,
            server.requests.len(),
            server.blob_tokens.len()
        ),
        before
    );
    drop(server);
    assert_eq!(
        pair.storage_a.load().unwrap().unwrap().expires_at_unix_ms,
        0
    );
}

#[test]
fn a_clock_database_failure_also_consumes_the_persisted_attempt_budget() {
    let pair = queued();
    let journal = pair.paths.user_data.join("msime_user.db");
    if journal.exists() {
        std::fs::remove_file(&journal).unwrap();
    }
    std::fs::create_dir(&journal).unwrap();
    for expected in [
        DownloadOutcome::MergeFailed,
        DownloadOutcome::MergeFailed,
        DownloadOutcome::Abandoned,
    ] {
        let restarted = super::super::session(&pair.server, &pair.storage_a);
        assert_eq!(
            restarted
                .learning_merge_pending(&pair.login, &pair.context())
                .unwrap()[0]
                .outcome,
            expected
        );
        assert_eq!(
            pair.progress(),
            RemoteProgress {
                verified: 2,
                applied: 0
            }
        );
    }
    std::fs::remove_dir(&journal).unwrap();
    assert!(pair
        .a
        .learning_merge_pending(&pair.login, &pair.context())
        .unwrap()
        .is_empty());
    assert_eq!(pair.exported().records().len(), 0);
}
