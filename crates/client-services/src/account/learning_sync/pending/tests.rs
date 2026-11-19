use super::*;

fn binding() -> String {
    "a".repeat(64)
}
fn source() -> String {
    "b".repeat(32)
}
fn envelope() -> Vec<u8> {
    vec![42; 84]
}

#[test]
fn restart_recovers_claim_without_resetting_attempts_or_losing_newer_pending() {
    let root = tempfile::tempdir().unwrap();
    let queue = Pending::open(root.path(), binding()).unwrap();
    queue.publish(&source(), 1, &envelope()).unwrap();
    let mut first = queue.claim(&source()).unwrap().unwrap();
    queue.started(&source(), &mut first).unwrap();
    queue.publish(&source(), 1, &envelope()).unwrap();
    queue.publish(&source(), 2, &envelope()).unwrap();
    drop(queue);
    let queue = Pending::open(root.path(), binding()).unwrap();
    let recovered = queue.claim(&source()).unwrap().unwrap();
    assert_eq!((recovered.generation, recovered.attempts), (1, 1));
    queue.discard(&source()).unwrap();
    let next = queue.claim(&source()).unwrap().unwrap();
    assert_eq!((next.generation, next.attempts), (2, 0));
    queue.discard(&source()).unwrap();
    assert!(queue.sources().unwrap().is_empty());
}

#[test]
fn new_binding_discards_only_old_ciphertext_and_keeps_account_progress() {
    let root = tempfile::tempdir().unwrap();
    let store = super::super::state::StateStore::new(root.path());
    store.prepare_reset("synthetic-account", 100, 0).unwrap();
    let queue = Pending::open(root.path(), binding()).unwrap();
    queue.publish(&source(), 1, &envelope()).unwrap();
    queue.claim(&source()).unwrap();
    queue.publish(&source(), 2, &envelope()).unwrap();
    drop(queue);
    let next = Pending::open(root.path(), "c".repeat(64)).unwrap();
    assert!(next.sources().unwrap().is_empty());
    assert_eq!(store.read("synthetic-account").unwrap().reset_floor, 100);
}

#[test]
fn source_capacity_refuses_new_data_without_evicting_an_existing_file() {
    let root = tempfile::tempdir().unwrap();
    let queue = Pending::open(root.path(), binding()).unwrap();
    for id in 0..MAX_SOURCES {
        queue
            .publish(&format!("{id:032x}"), 1, &envelope())
            .unwrap();
    }
    assert_eq!(
        queue.publish(&source(), 1, &envelope()),
        Err(PendingError::Full)
    );
    assert_eq!(queue.sources().unwrap().len(), MAX_SOURCES);
    for id in 0..MAX_SOURCES {
        assert_eq!(
            queue
                .claim(&format!("{id:032x}"))
                .unwrap()
                .unwrap()
                .envelope,
            envelope()
        );
    }
}

#[test]
fn invalid_publication_does_not_replace_the_existing_pending() {
    let root = tempfile::tempdir().unwrap();
    let queue = Pending::open(root.path(), binding()).unwrap();
    queue.publish(&source(), 1, &envelope()).unwrap();
    assert_eq!(
        queue.publish(&source(), 1, b"short"),
        Err(PendingError::Invalid)
    );
    assert_eq!(
        queue.publish(&source(), 2, b"short"),
        Err(PendingError::Invalid)
    );
    let entry = queue.claim(&source()).unwrap().unwrap();
    assert_eq!(entry.generation, 1);
    assert_eq!(entry.envelope, envelope());
    assert_eq!(
        queue.publish("../synthetic-escape", 2, &envelope()),
        Err(PendingError::Invalid)
    );
}

#[cfg(unix)]
#[test]
fn queue_files_are_private_and_do_not_follow_file_or_directory_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let queue = Pending::open(root.path(), binding()).unwrap();
    queue.publish(&source(), 1, &envelope()).unwrap();
    let file = root
        .path()
        .join("cloud-sync/learning-pending")
        .join(queue.name(&source(), false));
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_file(&file).unwrap();
    let target = outside.path().join("synthetic-file");
    std::fs::write(&target, b"synthetic-original").unwrap();
    symlink(&target, &file).unwrap();
    assert!(queue.publish(&source(), 2, &envelope()).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-original");
    drop(queue);
    let second = tempfile::tempdir().unwrap();
    std::fs::create_dir(second.path().join("cloud-sync")).unwrap();
    symlink(
        outside.path(),
        second.path().join("cloud-sync/learning-pending"),
    )
    .unwrap();
    assert!(Pending::open(second.path(), binding()).is_err());
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn opened_queue_remains_on_its_directory_when_path_is_replaced() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let queue = Pending::open(root.path(), binding()).unwrap();
    let original = root.path().join("cloud-sync/learning-pending");
    let moved = root.path().join("cloud-sync/moved-synthetic-queue");
    std::fs::rename(&original, &moved).unwrap();
    symlink(outside.path(), &original).unwrap();
    queue.publish(&source(), 1, &envelope()).unwrap();
    assert_eq!(queue.claim(&source()).unwrap().unwrap().generation, 1);
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(moved.join(queue.name(&source(), true)).exists());
}
