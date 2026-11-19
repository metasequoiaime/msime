use super::*;
use std::fs;

const ACCOUNT: &str = "synthetic-account";
const KEY: &str = "11111111111111111111111111111111";
const OTHER_KEY: &str = "22222222222222222222222222222222";
const DEVICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_DEVICE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn upload(generation: u64, key_id: &str) -> Uploaded {
    Uploaded {
        generation,
        key_id: key_id.to_owned(),
        sha256: "c".repeat(64),
    }
}

#[test]
fn empty_reads_do_not_create_progress_and_legacy_state_is_independent() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    assert_eq!(store.read(ACCOUNT).unwrap(), AccountProgress::default());
    assert!(!store.directory.join(STATE_FILE).exists());
    fs::write(
        store.directory.join("state.json"),
        b"synthetic legacy state",
    )
    .unwrap();
    store.observe(ACCOUNT, 103).unwrap();
    fs::write(
        store.directory.join("state.json"),
        b"older client rewrote this",
    )
    .unwrap();
    assert_eq!(
        StateStore::new(root.path())
            .read(ACCOUNT)
            .unwrap()
            .observed_max,
        103
    );
}

#[test]
fn verified_survives_restart_and_failed_application_can_retry_the_same_generation() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    assert_eq!(
        store.accept_verified(ACCOUNT, KEY, DEVICE, 5, 103),
        Ok(Acceptance::Merge)
    );
    drop(store);
    let store = StateStore::new(root.path());
    assert_eq!(
        store.accept_verified(ACCOUNT, KEY, DEVICE, 4, 200),
        Err(StateError::Rollback)
    );
    assert_eq!(store.read(ACCOUNT).unwrap().observed_max, 103);
    assert_eq!(
        store.accept_verified(ACCOUNT, KEY, DEVICE, 5, 103),
        Ok(Acceptance::Merge)
    );
    store.record_applied(ACCOUNT, KEY, DEVICE, 5).unwrap();
    assert_eq!(
        StateStore::new(root.path()).accept_verified(ACCOUNT, KEY, DEVICE, 5, 103),
        Ok(Acceptance::AlreadyApplied)
    );
}

#[test]
fn application_cannot_advance_before_verification_or_after_a_newer_batch() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    assert_eq!(
        store.record_applied(ACCOUNT, KEY, DEVICE, 5),
        Err(StateError::NotVerified)
    );
    store.accept_verified(ACCOUNT, KEY, DEVICE, 5, 100).unwrap();
    assert_eq!(
        store.record_applied(ACCOUNT, KEY, DEVICE, 6),
        Err(StateError::NotVerified)
    );
    store.accept_verified(ACCOUNT, KEY, DEVICE, 6, 101).unwrap();
    assert_eq!(
        store.record_applied(ACCOUNT, KEY, DEVICE, 5),
        Err(StateError::Rollback)
    );
    assert_eq!(
        store.read(ACCOUNT).unwrap().sources[KEY].0[DEVICE],
        RemoteProgress {
            verified: 6,
            applied: 0
        }
    );
}

#[test]
fn reset_covers_verified_but_unapplied_rows_and_keeps_the_floor_on_retry() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.accept_verified(ACCOUNT, KEY, DEVICE, 1, 103).unwrap();
    assert_eq!(store.prepare_reset(ACCOUNT, 100, 102), Ok(103));
    drop(store);
    let store = StateStore::new(root.path());
    assert_eq!(store.prepare_reset(ACCOUNT, 90, 0), Ok(103));
    store.observe(ACCOUNT, 10).unwrap();
    assert_eq!(store.read(ACCOUNT).unwrap().observed_max, 103);
    assert_eq!(store.prepare_reset(ACCOUNT, 101, 104), Ok(104));
    assert_eq!(store.prepare_reset(ACCOUNT, u64::MAX, 0), Ok(MAX_TIMESTAMP));
}

#[test]
fn accounts_keys_and_sources_are_isolated_without_erasing_old_barriers() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.accept_verified(ACCOUNT, KEY, DEVICE, 9, 103).unwrap();
    store.record_applied(ACCOUNT, KEY, DEVICE, 9).unwrap();
    store.prepare_reset(ACCOUNT, 100, 0).unwrap();
    assert_eq!(
        store.accept_verified(ACCOUNT, OTHER_KEY, DEVICE, 1, 100),
        Ok(Acceptance::Merge)
    );
    assert_eq!(
        store.accept_verified(ACCOUNT, KEY, OTHER_DEVICE, 1, 100),
        Ok(Acceptance::Merge)
    );
    store.observe("synthetic-second-account", 999).unwrap();
    let progress = StateStore::new(root.path()).read(ACCOUNT).unwrap();
    assert_eq!(progress.reset_floor, 103);
    assert_eq!(progress.observed_max, 103);
    assert_eq!(
        progress.sources[KEY].0[DEVICE],
        RemoteProgress {
            verified: 9,
            applied: 9
        }
    );
    assert_eq!(progress.sources[OTHER_KEY].0[DEVICE].verified, 1);
    assert_eq!(
        store.read("synthetic-second-account").unwrap().reset_floor,
        0
    );
}

#[test]
fn upload_generations_survive_remote_rollback_rekey_and_late_responses() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    assert_eq!(store.plan_upload(ACCOUNT, DEVICE, 0).unwrap().generation, 1);
    store
        .record_uploaded(ACCOUNT, DEVICE, upload(7, KEY))
        .unwrap();
    assert_eq!(store.plan_upload(ACCOUNT, DEVICE, 3).unwrap().generation, 8);
    assert_eq!(
        store.plan_upload(ACCOUNT, DEVICE, 9).unwrap().generation,
        10
    );
    store
        .record_uploaded(ACCOUNT, DEVICE, upload(8, OTHER_KEY))
        .unwrap();
    store
        .record_uploaded(ACCOUNT, DEVICE, upload(7, KEY))
        .unwrap();
    assert_eq!(
        StateStore::new(root.path()).read(ACCOUNT).unwrap().uploads[DEVICE],
        upload(8, OTHER_KEY)
    );
    assert_eq!(
        store.plan_upload(ACCOUNT, DEVICE, MAX_GENERATION),
        Ok(UploadPlan {
            generation: 9,
            delete_remote_first: true
        })
    );
    assert_eq!(
        store.plan_upload(ACCOUNT, DEVICE, u64::MAX),
        Ok(UploadPlan {
            generation: 9,
            delete_remote_first: true
        })
    );
    store
        .record_uploaded(ACCOUNT, DEVICE, upload(MAX_GENERATION, KEY))
        .unwrap();
    assert_eq!(
        store.plan_upload(ACCOUNT, DEVICE, 0),
        Err(StateError::Exhausted)
    );
}

#[test]
fn conflicting_upload_acknowledgments_do_not_replace_a_recorded_digest() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    let original = upload(3, KEY);
    store
        .record_uploaded(ACCOUNT, DEVICE, original.clone())
        .unwrap();
    store
        .record_uploaded(ACCOUNT, DEVICE, original.clone())
        .unwrap();
    let mut different = original.clone();
    different.sha256 = "d".repeat(64);
    assert_eq!(
        store.record_uploaded(ACCOUNT, DEVICE, different),
        Err(StateError::Invalid)
    );
    assert_eq!(store.read(ACCOUNT).unwrap().uploads[DEVICE], original);
}

#[test]
fn invalid_inputs_do_not_change_the_saved_barrier() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.prepare_reset(ACCOUNT, 100, 103).unwrap();
    let path = store.directory.join(STATE_FILE);
    let before = fs::read(&path).unwrap();
    assert_eq!(
        store.observe(ACCOUNT, MAX_TIMESTAMP + 1),
        Err(StateError::Invalid)
    );
    assert_eq!(
        store.prepare_reset(ACCOUNT, 100, MAX_TIMESTAMP + 1),
        Err(StateError::Invalid)
    );
    for generation in [0, MAX_GENERATION + 1, u64::MAX] {
        assert_eq!(
            store.accept_verified(ACCOUNT, KEY, DEVICE, generation, 104),
            Err(StateError::Invalid)
        );
        assert_eq!(
            store.record_applied(ACCOUNT, KEY, DEVICE, generation),
            Err(StateError::Invalid)
        );
        assert_eq!(
            store.record_uploaded(ACCOUNT, DEVICE, upload(generation, KEY)),
            Err(StateError::Invalid)
        );
    }
    assert_eq!(
        store.accept_verified(ACCOUNT, KEY, DEVICE, 1, MAX_TIMESTAMP + 1),
        Err(StateError::Invalid)
    );
    assert_eq!(
        store.accept_verified(ACCOUNT, "invalid", DEVICE, 1, 104),
        Err(StateError::Invalid)
    );
    assert_eq!(
        store.plan_upload(ACCOUNT, "invalid", 1),
        Err(StateError::Invalid)
    );
    assert_eq!(store.observe("", 104), Err(StateError::Invalid));
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn malformed_unknown_and_oversized_state_never_fall_back_to_zero() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.observe(ACCOUNT, 103).unwrap();
    let path = store.directory.join(STATE_FILE);
    let saved = fs::read_to_string(&path).unwrap();
    let mut cases = vec![
        "{".to_owned(),
        saved.replace("\"version\":1", "\"version\":2"),
        saved.replace("\"reset_floor\":0", "\"reset_floor\":104"),
        saved.replace("\"observed_max\":103", "\"observed_max\":1099511627777"),
        saved.replace(
            "\"observed_max\":103",
            "\"observed_max\":103,\"observed_max\":0",
        ),
        " ".repeat(MAX_STATE_BYTES + 1),
    ];
    // 映射各层的重复身份同样拒绝，避免后写的低进度掩盖前一份。
    let account = serde_json::to_string(&store.read(ACCOUNT).unwrap()).unwrap();
    cases.push(format!(
        "{{\"version\":1,\"accounts\":{{\"{ACCOUNT}\":{account},\"{ACCOUNT}\":{account}}}}}"
    ));
    let remote = "{\"verified\":9,\"applied\":1}";
    let sources = format!("{{\"{DEVICE}\":{remote},\"{DEVICE}\":{remote}}}");
    cases.push(saved.replace(
        "\"sources\":{}",
        &format!("\"sources\":{{\"{KEY}\":{sources}}}"),
    ));
    cases.push(saved.replace(
        "\"sources\":{}",
        &format!("\"sources\":{{\"{KEY}\":{{}},\"{KEY}\":{{}}}}"),
    ));
    let uploaded = serde_json::to_string(&upload(1, KEY)).unwrap();
    cases.push(saved.replace(
        "\"uploads\":{}",
        &format!("\"uploads\":{{\"{DEVICE}\":{uploaded},\"{DEVICE}\":{uploaded}}}"),
    ));
    for bytes in cases {
        fs::write(&path, &bytes).unwrap();
        assert_eq!(store.read(ACCOUNT), Err(StateError::Invalid));
        assert_eq!(
            store.prepare_reset(ACCOUNT, 100, 0),
            Err(StateError::Invalid)
        );
        assert_eq!(
            store.accept_verified(ACCOUNT, KEY, DEVICE, 10, 105),
            Err(StateError::Invalid)
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    }
}

#[test]
fn corrupt_remote_and_upload_progress_are_rejected_without_repair() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.accept_verified(ACCOUNT, KEY, DEVICE, 3, 103).unwrap();
    store
        .record_uploaded(ACCOUNT, DEVICE, upload(5, KEY))
        .unwrap();
    let path = store.directory.join(STATE_FILE);
    let saved = fs::read_to_string(&path).unwrap();
    for bytes in [
        saved.replace("\"applied\":0", "\"applied\":4"),
        saved.replace("\"verified\":3", "\"verified\":9007199254740992"),
        saved.replace("\"generation\":5", "\"generation\":0"),
        saved.replace(&"c".repeat(64), "invalid"),
        saved.replace(DEVICE, "invalid"),
    ] {
        fs::write(&path, &bytes).unwrap();
        assert_eq!(store.read(ACCOUNT), Err(StateError::Invalid));
        assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
    }
}

#[test]
fn unavailable_storage_does_not_acknowledge_a_reset_or_verified_batch() {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    fs::write(&store.directory, b"synthetic obstruction").unwrap();
    assert_eq!(
        store.prepare_reset(ACCOUNT, 100, 103),
        Err(StateError::Unavailable)
    );
    assert_eq!(
        store.accept_verified(ACCOUNT, KEY, DEVICE, 1, 103),
        Err(StateError::Unavailable)
    );
    assert_eq!(
        fs::read(&store.directory).unwrap(),
        b"synthetic obstruction"
    );
}

#[cfg(unix)]
#[test]
fn private_state_rejects_symlinks_and_is_written_with_owner_only_permissions() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(root.path());
    store.observe(ACCOUNT, 103).unwrap();
    let path = store.directory.join(STATE_FILE);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let outside = root.path().join("synthetic-outside.json");
    fs::rename(&path, &outside).unwrap();
    let before = fs::read(&outside).unwrap();
    symlink(&outside, &path).unwrap();
    assert_eq!(store.observe(ACCOUNT, 104), Err(StateError::Unavailable));
    assert_eq!(fs::read(&outside).unwrap(), before);
}

#[test]
fn separate_processes_keep_all_source_progress_and_the_maximum_observed_time() {
    let root = tempfile::tempdir().unwrap();
    let executable = std::env::current_exe().unwrap();
    let mut children = Vec::new();
    for worker in 1..=6 {
        children.push(
            std::process::Command::new(&executable)
                .args([
                    "--exact",
                    "account::learning_sync::state::tests::child_progress_writer",
                ])
                .env("MSIME_LEARNING_STATE_TEST_ROOT", root.path())
                .env("MSIME_LEARNING_STATE_TEST_WORKER", worker.to_string())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let saved = StateStore::new(root.path()).read(ACCOUNT).unwrap();
    assert_eq!(saved.observed_max, 620);
    assert_eq!(saved.sources[KEY].0.len(), 6);
    for worker in 1..=6 {
        assert_eq!(
            saved.sources[KEY].0[&format!("{worker:032x}")],
            RemoteProgress {
                verified: 20,
                applied: 20
            }
        );
    }
}

#[test]
fn child_progress_writer() {
    let Some(root) = std::env::var_os("MSIME_LEARNING_STATE_TEST_ROOT") else {
        return;
    };
    let worker: u64 = std::env::var("MSIME_LEARNING_STATE_TEST_WORKER")
        .unwrap()
        .parse()
        .unwrap();
    let source = format!("{worker:032x}");
    let store = StateStore::new(Path::new(&root));
    for generation in 1..=20 {
        store
            .accept_verified(ACCOUNT, KEY, &source, generation, worker * 100 + generation)
            .unwrap();
        store
            .record_applied(ACCOUNT, KEY, &source, generation)
            .unwrap();
    }
}
