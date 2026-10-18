//! 用例里的 `state.bin` 是用 Android 原来的 `DictionarySnapshotQueue.java`（连同它用到的 `SafePaths`、`TextPolicy`、`HttpBodyPolicy`、`DigestPolicy`、`BoundsPolicy`）在 JDK 21 上经公开方法生成的真实字节，不是照格式手写的：
//! - `PREPARING`：`publishLocalVersion` 旧版本号，`enqueue` 账号 `fixture-账号-𝄞`（三字节字符和补充平面字符都走 modified UTF-8）、修订号 7，再 `acquireWorkerLease` 加 `claim`。
//! - `LOCAL_ONLY`：只 `publishLocalVersion` 一个代次 UUID 的版本号，没有请求。
//! - `LOCAL_BACKUP`：本地备份恢复排进来的请求（账号是 `local-backup`），修订号 0。
//! - `CANCELLED`：`enqueue` 后 `cancel` 掉的请求。

use super::super::{
    DictionarySnapshotQueue, SnapshotQueueError, SnapshotQueueState, SnapshotRequest,
    SnapshotRequestStatus, LOCAL_RESTORE_OWNER, STATE_NAME as JSON_STATE_NAME,
};
use super::STATE_NAME;
use std::fs;
use std::path::Path;
use uuid::Uuid;

const PREPARING: &str = "4d535131000000010100506c6f63616c2d76313a6c65676163793a61616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161018cf668408c0a4bf791657d0f5457cb710015666978747572652de8b4a6e58fb72deda0b4edb49e00506c6f63616c2d76313a6c65676163793a61616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161004061373936386162383465303366306635366563663736626465373465363462363434323565613163346431346437323564346537646138656464346334623562000000000000000701";
const PREPARING_ID: &str = "8cf66840-8c0a-4bf7-9165-7d0f5457cb71";
const LOCAL_ONLY: &str = "4d5351310000000101006e6c6f63616c2d76313a30623666396134652d336332642d346531662d386137622d3663356434653366326131623a6262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626200";
const LOCAL_BACKUP: &str = "4d535131000000010100506c6f63616c2d76313a6c65676163793a6161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616101802478538ffb4db6a49c509d2c2a4c8e000c6c6f63616c2d6261636b757000506c6f63616c2d76313a6c65676163793a61616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161004061373936386162383465303366306635366563663736626465373465363462363434323565613163346431346437323564346537646138656464346334623562000000000000000000";
const LOCAL_BACKUP_ID: &str = "80247853-8ffb-4db6-a49c-509d2c2a4c8e";
const CANCELLED: &str = "4d535131000000010100506c6f63616c2d76313a6c65676163793a6161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616101ca7f52ec3bbc4ca888360a7ce654539f00076669787475726500506c6f63616c2d76313a6c65676163793a61616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161004061373936386162383465303366306635366563663736626465373465363462363434323565613163346431346437323564346537646138656464346334623562000000000000000305";
const CANCELLED_ID: &str = "ca7f52ec-3bbc-4ca8-8836-0a7ce654539f";
const FILE_SHA256: &str = "a7968ab84e03f0f56ecf76bde74e64b64425ea1c4d14d725d4e7da8edd4c4b5b";
const SNAPSHOT: &[u8] = b"synthetic snapshot\n";

fn legacy_version() -> String {
    format!("local-v1:legacy:{}", "a".repeat(64))
}

fn request(
    id: &str,
    account_id: &str,
    cloud_revision: i64,
    status: SnapshotRequestStatus,
) -> SnapshotRequest {
    SnapshotRequest {
        id: Uuid::parse_str(id).unwrap(),
        account_id: account_id.to_owned(),
        cloud_revision,
        expected_local_version: legacy_version(),
        file_sha256: FILE_SHA256.to_owned(),
        status,
    }
}

fn state(local_version: Option<String>, request: Option<SnapshotRequest>) -> SnapshotQueueState {
    SnapshotQueueState {
        version: 1,
        local_version,
        request,
    }
}

fn preparing_state() -> SnapshotQueueState {
    state(
        Some(legacy_version()),
        Some(request(
            PREPARING_ID,
            "fixture-账号-𝄞",
            7,
            SnapshotRequestStatus::Preparing,
        )),
    )
}

fn private_write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

/// 一个 Java 留下的队列目录：`state.bin`，以及请求在等激活时的 `<id>.ndjson`。
fn java_queue(fixture: &str, id: Option<&str>) -> (tempfile::TempDir, DictionarySnapshotQueue) {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("queue");
    fs::create_dir(&root).unwrap();
    private_write(&root.join(STATE_NAME), &hex::decode(fixture).unwrap());
    if let Some(id) = id {
        private_write(&root.join(format!("{id}.ndjson")), SNAPSHOT);
    }
    let queue = DictionarySnapshotQueue::new(root).unwrap();
    (parent, queue)
}

fn queue_root(parent: &tempfile::TempDir) -> std::path::PathBuf {
    parent.path().join("queue")
}

#[cfg(unix)]
fn inode(path: &Path) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path).unwrap();
    (metadata.dev(), metadata.ino())
}

fn entries(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn decodes_every_java_fixture() {
    let cases = [
        (PREPARING, preparing_state()),
        (
            LOCAL_ONLY,
            state(
                Some(format!(
                    "local-v1:0b6f9a4e-3c2d-4e1f-8a7b-6c5d4e3f2a1b:{}",
                    "b".repeat(64)
                )),
                None,
            ),
        ),
        (
            LOCAL_BACKUP,
            state(
                Some(legacy_version()),
                Some(request(
                    LOCAL_BACKUP_ID,
                    "local-backup",
                    0,
                    SnapshotRequestStatus::Queued,
                )),
            ),
        ),
        (
            CANCELLED,
            state(
                Some(legacy_version()),
                Some(request(
                    CANCELLED_ID,
                    "fixture",
                    3,
                    SnapshotRequestStatus::Cancelled,
                )),
            ),
        ),
    ];
    for (fixture, expected) in cases {
        assert_eq!(
            super::decode(&hex::decode(fixture).unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn converts_state_bin_in_place_without_copying_the_snapshot() {
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let root = queue_root(&parent);
    let snapshot = root.join(format!("{PREPARING_ID}.ndjson"));
    #[cfg(unix)]
    let before = inode(&snapshot);

    assert!(queue.import_android_v1().unwrap());

    assert_eq!(queue.read().unwrap(), preparing_state());
    assert!(!root.join(STATE_NAME).exists(), "转换完成后删掉 state.bin");
    let stored: SnapshotQueueState =
        serde_json::from_slice(&fs::read(root.join(JSON_STATE_NAME)).unwrap()).unwrap();
    assert_eq!(stored, preparing_state());
    assert_eq!(fs::read(&snapshot).unwrap(), SNAPSHOT);
    #[cfg(unix)]
    assert_eq!(inode(&snapshot), before, "快照沿用原来那个文件，不复制");
    assert_eq!(
        entries(&root),
        [
            format!("{PREPARING_ID}.ndjson"),
            "state.json".to_owned(),
            "state.lock".to_owned(),
        ]
    );
}

#[test]
fn converted_request_is_processed_by_the_rust_worker() {
    let (_parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    assert!(queue.import_android_v1().unwrap());
    let id = Uuid::parse_str(PREPARING_ID).unwrap();
    assert!(queue.file_path(id).unwrap().exists());

    let lease = queue.acquire_worker_lease().unwrap();
    let claimed = queue
        .claim(&lease, Some("fixture-账号-𝄞"))
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, id);
    let next = format!("local-v1:{id}:{}", "c".repeat(64));
    assert!(queue
        .complete(
            id,
            &lease,
            &legacy_version(),
            false,
            Some("fixture-账号-𝄞"),
            || { Ok(next.clone()) }
        )
        .unwrap());
    let state = queue.read().unwrap();
    assert_eq!(state.local_version.as_deref(), Some(next.as_str()));
    assert_eq!(
        state.request.unwrap().status,
        SnapshotRequestStatus::Applied
    );
    assert!(!queue.file_path(id).unwrap().exists());
}

#[test]
fn repeated_import_changes_nothing() {
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let root = queue_root(&parent);
    assert!(queue.import_android_v1().unwrap());
    let converted = fs::read(root.join(JSON_STATE_NAME)).unwrap();
    let listing = entries(&root);

    assert!(!queue.import_android_v1().unwrap());
    assert!(!queue.import_android_v1().unwrap());

    assert_eq!(fs::read(root.join(JSON_STATE_NAME)).unwrap(), converted);
    assert_eq!(entries(&root), listing);
    assert_eq!(
        fs::read(root.join(format!("{PREPARING_ID}.ndjson"))).unwrap(),
        SNAPSHOT
    );
}

#[test]
fn existing_state_json_wins_and_a_leftover_state_bin_is_removed() {
    // 上一次转换写好 state.json 后、删 state.bin 之前中断；之后 Rust 的队列又改过 state.json。
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let root = queue_root(&parent);
    let newer = state(
        Some(format!("local-v1:{PREPARING_ID}:{}", "d".repeat(64))),
        Some(request(
            PREPARING_ID,
            "fixture-账号-𝄞",
            7,
            SnapshotRequestStatus::Applied,
        )),
    );
    private_write(
        &root.join(JSON_STATE_NAME),
        &serde_json::to_vec(&newer).unwrap(),
    );

    assert!(!queue.import_android_v1().unwrap());

    assert_eq!(queue.read().unwrap(), newer);
    assert!(!root.join(STATE_NAME).exists());
    assert_eq!(
        fs::read(root.join(format!("{PREPARING_ID}.ndjson"))).unwrap(),
        SNAPSHOT
    );
}

#[test]
fn a_missing_directory_is_not_created() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("queue");
    let queue = DictionarySnapshotQueue::new(root.clone()).unwrap();
    assert!(!queue.import_android_v1().unwrap());
    assert!(!root.exists());
}

#[test]
fn a_queue_without_state_bin_is_left_alone() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("queue");
    fs::create_dir(&root).unwrap();
    let queue = DictionarySnapshotQueue::new(root.clone()).unwrap();
    assert!(!queue.import_android_v1().unwrap());
    assert!(!root.join(JSON_STATE_NAME).exists());
}

#[test]
fn reading_before_the_import_sees_the_java_state_without_writing() {
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let root = queue_root(&parent);
    assert_eq!(queue.read().unwrap(), preparing_state());
    assert!(root.join(STATE_NAME).exists());
    assert!(!root.join(JSON_STATE_NAME).exists());
}

#[test]
fn a_locked_operation_converts_before_overwriting_the_state() {
    // `:ime` 进程升级后先跑 `process`，它第一步就是 publish_local_version；没有先转换的话会写出一份没有请求的 state.json，旧队列从此读不到。
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let root = queue_root(&parent);
    queue.publish_local_version(&legacy_version()).unwrap();

    assert_eq!(queue.read().unwrap(), preparing_state());
    assert!(!root.join(STATE_NAME).exists());
    assert!(!queue.import_android_v1().unwrap());
    assert_eq!(queue.read().unwrap(), preparing_state());
}

#[test]
fn cancellation_converts_before_cancelling() {
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let root = queue_root(&parent);
    queue.cancel("fixture-账号-𝄞").unwrap();
    assert_eq!(
        queue.read().unwrap().request.unwrap().status,
        SnapshotRequestStatus::Cancelled
    );
    assert!(!root.join(STATE_NAME).exists());
    assert!(!root.join(format!("{PREPARING_ID}.ndjson")).exists());
}

fn assert_unreadable_state_bin_is_kept(bytes: &[u8]) {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("queue");
    fs::create_dir(&root).unwrap();
    private_write(&root.join(STATE_NAME), bytes);
    let snapshot = root.join(format!("{PREPARING_ID}.ndjson"));
    private_write(&snapshot, SNAPSHOT);
    let queue = DictionarySnapshotQueue::new(root.clone()).unwrap();

    assert!(matches!(
        queue.import_android_v1(),
        Err(SnapshotQueueError::Invalid)
    ));
    assert!(matches!(queue.read(), Err(SnapshotQueueError::Invalid)));
    assert!(matches!(
        queue.publish_local_version(&legacy_version()),
        Err(SnapshotQueueError::Invalid)
    ));
    assert_eq!(fs::read(root.join(STATE_NAME)).unwrap(), bytes);
    assert!(!root.join(JSON_STATE_NAME).exists());
    assert_eq!(fs::read(&snapshot).unwrap(), SNAPSHOT);
}

#[test]
fn a_truncated_state_bin_is_rejected_and_kept() {
    let complete = hex::decode(PREPARING).unwrap();
    for length in 0..complete.len() {
        assert_unreadable_state_bin_is_kept(&complete[..length]);
    }
}

#[test]
fn a_corrupt_state_bin_is_rejected_and_kept() {
    let complete = hex::decode(PREPARING).unwrap();
    let mut cases = Vec::new();
    let mut magic = complete.clone();
    magic[0] = b'X';
    cases.push(magic);
    let mut format = complete.clone();
    format[7] = 2;
    cases.push(format);
    let mut trailing = complete.clone();
    trailing.push(0);
    cases.push(trailing);
    let mut status = complete.clone();
    *status.last_mut().unwrap() = 6;
    cases.push(status);
    // 本机版本号里的一个字符改成不合法的 modified UTF-8 后续字节。
    let mut encoding = complete.clone();
    encoding[13] = 0xc3;
    cases.push(encoding);
    // UUID 全零：Rust 的状态不收 nil。
    let mut nil = complete.clone();
    let request_at = 4 + 4 + 1 + 2 + legacy_version().len() + 1;
    nil[request_at..request_at + 16].fill(0);
    cases.push(nil);
    // 修订号为负。
    let mut revision = complete.clone();
    let revision_at = complete.len() - 1 - 8;
    revision[revision_at] = 0x80;
    cases.push(revision);
    cases.push(vec![0; super::MAXIMUM_STATE_BYTES as usize + 1]);
    for bytes in cases {
        assert_unreadable_state_bin_is_kept(&bytes);
    }
}

#[test]
fn modified_utf8_follows_java_read_utf() {
    assert_eq!(super::modified_utf8(b"plain").unwrap(), "plain");
    assert_eq!(super::modified_utf8(&[0xc0, 0x80]).unwrap(), "\0");
    assert_eq!(super::modified_utf8(&[0xe8, 0xb4, 0xa6]).unwrap(), "账");
    assert_eq!(
        super::modified_utf8(&[0xed, 0xa0, 0xb4, 0xed, 0xb4, 0x9e]).unwrap(),
        "𝄞"
    );
    for invalid in [
        &[0xed, 0xa0, 0xb4][..],
        &[0xf0, 0x9d, 0x84, 0x9e],
        &[0x80],
        &[0xc3],
        &[0xc3, 0x41],
        &[0xe8, 0xb4],
    ] {
        assert!(matches!(
            super::modified_utf8(invalid),
            Err(SnapshotQueueError::Invalid)
        ));
    }
}

#[test]
fn import_waits_for_a_held_state_lock_instead_of_reporting_busy() {
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let held = queue.lock(super::super::STATE_LOCK_NAME).unwrap();
    let root = queue_root(&parent);
    let importer = std::thread::spawn(move || {
        DictionarySnapshotQueue::new(root)
            .unwrap()
            .import_android_v1()
    });
    // 宽限只有约 50 ms，报 Busy 的实现在这之前就会返回。
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!importer.is_finished(), "持锁期间转换要等着");
    drop(held);
    assert!(importer.join().unwrap().unwrap());
    assert_eq!(queue.read().unwrap(), preparing_state());
}

#[test]
fn import_after_the_conversion_does_not_wait_for_a_held_state_lock() {
    // Android 的 `DictionarySnapshotQueue.read` 每次都经 import_android_v1；`:ime` 激活时整个回调都持着 state.lock，转换完成以后读状态不能排在它后面。
    let (parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    assert!(queue.import_android_v1().unwrap());
    let held = queue.lock(super::super::STATE_LOCK_NAME).unwrap();
    let root = queue_root(&parent);
    let importer = std::thread::spawn(move || {
        DictionarySnapshotQueue::new(root)
            .unwrap()
            .import_android_v1()
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !importer.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let finished = importer.is_finished();
    drop(held);
    assert!(finished, "转换完成以后不该再等 state.lock");
    assert!(!importer.join().unwrap().unwrap());
    assert_eq!(queue.read().unwrap(), preparing_state());
}

#[test]
fn import_of_a_queue_without_state_bin_does_not_create_the_lock_file() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("queue");
    fs::create_dir(&root).unwrap();
    let queue = DictionarySnapshotQueue::new(root.clone()).unwrap();
    assert!(!queue.import_android_v1().unwrap());
    assert!(entries(&root).is_empty());
}

/// Java 排的本地备份恢复请求（`LOCAL_BACKUP`，账号是 `LOCAL_RESTORE_OWNER`）转换以后原样保留，Rust 的 worker 不知道当前账号是谁时也照样认领并激活，与 Java 的 `owned` 相同。`import_first` 为假时不先调 `import_android_v1`，由认领时的就地转换接上。
fn assert_local_backup_is_applied_without_an_account(import_first: bool) {
    let (_parent, queue) = java_queue(LOCAL_BACKUP, Some(LOCAL_BACKUP_ID));
    let id = Uuid::parse_str(LOCAL_BACKUP_ID).unwrap();
    let queued = state(
        Some(legacy_version()),
        Some(request(
            LOCAL_BACKUP_ID,
            LOCAL_RESTORE_OWNER,
            0,
            SnapshotRequestStatus::Queued,
        )),
    );
    if import_first {
        assert!(queue.import_android_v1().unwrap());
        assert_eq!(queue.read().unwrap(), queued);
    }

    let lease = queue.acquire_worker_lease().unwrap();
    let claimed = queue.claim(&lease, None).unwrap().unwrap();
    assert_eq!(claimed.id, id);
    assert_eq!(claimed.status, SnapshotRequestStatus::Preparing);
    let next = format!("local-v1:{id}:{}", "c".repeat(64));
    assert!(queue
        .complete(id, &lease, &legacy_version(), false, None, || Ok(
            next.clone()
        ))
        .unwrap());
    let applied = queue.read().unwrap();
    assert_eq!(applied.local_version.as_deref(), Some(next.as_str()));
    assert_eq!(
        applied.request.unwrap().status,
        SnapshotRequestStatus::Applied
    );
    assert!(!queue.file_path(id).unwrap().exists());
}

#[test]
fn a_converted_local_backup_request_is_applied_without_an_account() {
    assert_local_backup_is_applied_without_an_account(true);
}

#[test]
fn a_local_backup_request_left_by_java_is_applied_without_an_account() {
    assert_local_backup_is_applied_without_an_account(false);
}

#[test]
fn a_local_backup_request_is_never_cancelled_for_an_account_mismatch() {
    let (_parent, queue) = java_queue(LOCAL_BACKUP, Some(LOCAL_BACKUP_ID));
    let id = Uuid::parse_str(LOCAL_BACKUP_ID).unwrap();
    // 退出登录只取消该账号自己的请求，碰不到它。
    queue.cancel("fixture").unwrap();
    assert_eq!(
        queue.read().unwrap().request.unwrap().status,
        SnapshotRequestStatus::Queued
    );
    assert!(queue.file_path(id).unwrap().exists());

    // 登录了某个账号、未登录（空字符串）、认领和激活之间换了账号，都不当成别的账号的请求。
    let lease = queue.acquire_worker_lease().unwrap();
    assert_eq!(
        queue.claim(&lease, Some("someone")).unwrap().unwrap().id,
        id
    );
    assert_eq!(queue.claim(&lease, Some("")).unwrap().unwrap().id, id);
    let next = format!("local-v1:{id}:{}", "d".repeat(64));
    assert!(queue
        .complete(
            id,
            &lease,
            &legacy_version(),
            false,
            Some("someone-else"),
            || Ok(next.clone())
        )
        .unwrap());
    assert_eq!(
        queue.read().unwrap().request.unwrap().status,
        SnapshotRequestStatus::Applied
    );
}

#[test]
fn an_account_request_waits_while_the_account_is_unknown() {
    let (_parent, queue) = java_queue(PREPARING, Some(PREPARING_ID));
    let id = Uuid::parse_str(PREPARING_ID).unwrap();
    let lease = queue.acquire_worker_lease().unwrap();
    // 不知道当前账号是谁：不认领、不取消，快照留着。
    assert!(queue.claim(&lease, None).unwrap().is_none());
    assert_eq!(queue.read().unwrap(), preparing_state());
    assert!(queue.file_path(id).unwrap().exists());
    // 认领之后、激活之前又不知道了：不激活，请求和快照原样留着。
    assert!(queue
        .claim(&lease, Some("fixture-账号-𝄞"))
        .unwrap()
        .is_some());
    assert!(!queue
        .complete(id, &lease, &legacy_version(), false, None, || {
            panic!("an unknown account must not activate an account's request")
        })
        .unwrap());
    assert_eq!(queue.read().unwrap(), preparing_state());
    assert!(queue.file_path(id).unwrap().exists());
}
