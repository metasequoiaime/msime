//! `record` 攒批：不逐次写文件、满 30 秒或 flush 才写、写时与别的进程改过的文件合并。

use super::*;

fn enabled_store() -> (tempfile::TempDir, TypingStatisticsStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    store.set_enabled(true).unwrap();
    (directory, store)
}

/// 文件里此刻写着的统计，不经 store：`load` 会先把攒着的上屏写出去，这里要看的正是还没写的时候。
fn on_disk(directory: &tempfile::TempDir) -> TypingStatistics {
    match fs::read(directory.path().join(STATISTICS_FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => TypingStatistics::default(),
        Err(error) => panic!("{error}"),
    }
}

/// 代替另一个进程（设置页）在文件锁内改统计文件：它看不到本进程攒着的上屏和选词，也不会替本进程写出它们。不经 `read_locked` 和 `write_locked`，那两处会把读到、写下的文件状态记成本进程所知的，而本进程并不知道别的进程改了什么。
fn other_process(store: &TypingStatisticsStore, change: impl FnOnce(&mut TypingStatistics)) {
    let lock = store.lock_file().unwrap();
    let mut value = match fs::read(store.path()) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => TypingStatistics::default(),
        Err(error) => panic!("{error}"),
    };
    change(&mut value);
    crate::file_lock::write_private_file_at(
        &lock.directory,
        std::ffi::OsStr::new(STATISTICS_FILE),
        &serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

const DAY: &str = "2026-10-27";

#[test]
fn records_are_held_in_memory_instead_of_rewriting_the_file_each_time() {
    let (directory, store) = enabled_store();
    let before = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    for (offset, text) in ["你好", "世界", "abc"].into_iter().enumerate() {
        let now = 1_000 + offset as u64 * 2_000;
        let count = store
            .record_at(text, TypingSource::Quanpin, DAY, Some(9), now)
            .unwrap();
        assert_eq!(count, text.chars().count() as u64);
    }
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        before,
        "a commit within the interval must not rewrite the file"
    );
    assert_eq!(on_disk(&directory).total, 0);

    store.flush().unwrap();
    let written = on_disk(&directory);
    assert_eq!(written.total, 7);
    assert_eq!(written.days[DAY], 7);
    assert_eq!(written.detail.characters["han"], 4);
    assert_eq!(written.detail.characters["latin"], 3);
    assert_eq!(written.daily_hours[DAY][9], 7);
    // Two gaps of 2 s each, measured commit by commit as before batching.
    assert_eq!(written.active_ms(DAY), Some(4_000));
    assert_eq!(written.last_commit_ms, 5_000);
}

#[test]
fn the_next_record_after_thirty_seconds_writes_the_batch() {
    let (directory, store) = enabled_store();
    store
        .record_at("一", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    store
        .record_at(
            "二",
            TypingSource::Quanpin,
            DAY,
            Some(9),
            1_000 + COMMIT_FLUSH_INTERVAL_MS - 1,
        )
        .unwrap();
    assert_eq!(on_disk(&directory).total, 0, "29.999 s is not yet due");

    store
        .record_at(
            "三",
            TypingSource::Quanpin,
            DAY,
            Some(9),
            1_000 + COMMIT_FLUSH_INTERVAL_MS,
        )
        .unwrap();
    assert_eq!(
        on_disk(&directory).total,
        3,
        "the commit that reaches 30 s writes itself and everything before it"
    );

    // The interval starts again from that write.
    store
        .record_at(
            "四",
            TypingSource::Quanpin,
            DAY,
            Some(9),
            1_000 + COMMIT_FLUSH_INTERVAL_MS + 5_000,
        )
        .unwrap();
    assert_eq!(on_disk(&directory).total, 3);
    assert_eq!(store.load().unwrap().total, 4);
}

#[test]
fn a_clock_set_backwards_writes_rather_than_waiting_for_it_to_catch_up() {
    let (directory, store) = enabled_store();
    store
        .record_at("一", TypingSource::Quanpin, DAY, Some(9), 100_000)
        .unwrap();
    store
        .record_at(
            "二",
            TypingSource::Quanpin,
            DAY,
            Some(9),
            100_000 - COMMIT_FLUSH_INTERVAL_MS,
        )
        .unwrap();
    assert_eq!(on_disk(&directory).total, 2);
}

#[test]
fn batched_commits_produce_the_same_document_as_writing_each_one() {
    let commits: [(&str, &str, Option<u8>, u64); 6] = [
        ("你好", "2026-10-26", Some(23), 1_000),
        ("世界", "2026-10-26", Some(23), 3_000),
        // A break: no active time, and a new run.
        (
            "再见",
            "2026-10-26",
            Some(23),
            3_000 + ACTIVE_GAP_LIMIT_MS + 1,
        ),
        // Across midnight: the gap goes to the new day, and retention runs on its first write.
        (
            "早",
            "2026-10-27",
            Some(0),
            3_000 + ACTIVE_GAP_LIMIT_MS + 2_000,
        ),
        (
            "上好",
            "2026-10-27",
            None,
            3_000 + ACTIVE_GAP_LIMIT_MS + 4_000,
        ),
        // A clock set backwards adds nothing and never moves the mark back.
        ("！", "2026-10-27", Some(0), 500),
    ];
    let (one_by_one_directory, one_by_one) = enabled_store();
    let (batched_directory, batched) = enabled_store();
    for store in [&one_by_one, &batched] {
        store
            .set_retention(StatisticsRetention::Days30, "2026-10-25")
            .unwrap();
    }
    for (text, day, hour, now) in commits {
        one_by_one
            .record_at(text, TypingSource::Quanpin, day, hour, now)
            .unwrap();
        one_by_one.flush().unwrap();
        batched
            .record_at(text, TypingSource::Quanpin, day, hour, now)
            .unwrap();
    }
    assert_eq!(on_disk(&batched_directory).total, 0);
    batched.flush().unwrap();
    let expected = on_disk(&one_by_one_directory);
    assert_eq!(expected.total, 10);
    assert_eq!(on_disk(&batched_directory), expected);
}

/// 代替另一个进程做 `reset`：清空的字段与 [`TypingStatisticsStore::reset`] 相同，也换清空代号，但不先写出本进程攒着的上屏。
fn other_process_reset(store: &TypingStatisticsStore) {
    other_process(store, |value| {
        *value = TypingStatistics {
            enabled: value.enabled,
            retention: value.retention,
            last_pruned_day: value.last_pruned_day.clone(),
            generation: next_generation(value.generation),
            ..TypingStatistics::default()
        };
    });
}

#[test]
fn pending_commits_typed_before_another_process_reset_are_not_written_back() {
    let (directory, store) = enabled_store();
    store
        .record_at("昨天", TypingSource::Quanpin, "2026-10-26", Some(9), 1_000)
        .unwrap();
    store.flush().unwrap();
    store
        .record_at("今天", TypingSource::Quanpin, DAY, Some(9), 90_000)
        .unwrap();
    // The settings app clears the statistics while this process still holds today's commit. Before batching that commit was already in the file and the clear removed it; it must not come back.
    other_process_reset(&store);
    let cleared = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    store.flush().unwrap();
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        cleared
    );
    let after = on_disk(&directory);
    assert_eq!(after.total, 0);
    assert!(after.days.is_empty());
    assert!(after.daily_hours.is_empty());
    assert_eq!(after.last_commit_ms, 0);

    // The batch is dropped only once: typing after the clear is counted against the cleared file.
    store
        .record_at("之后", TypingSource::Quanpin, DAY, Some(10), 200_000)
        .unwrap();
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).total, 2);
}

#[test]
fn a_due_record_after_another_process_reset_drops_the_held_batch() {
    let (directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    other_process_reset(&store);
    let (_, total) = store
        .record_with_total_at(
            "世界",
            TypingSource::Quanpin,
            DAY,
            Some(9),
            1_000 + COMMIT_FLUSH_INTERVAL_MS,
        )
        .unwrap();
    assert_eq!(total, 0, "the running total follows the cleared file");
    assert_eq!(on_disk(&directory).total, 0);
    store
        .record_at("再见", TypingSource::Quanpin, DAY, Some(9), 40_000)
        .unwrap();
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).total, 2);
}

#[test]
fn a_retention_change_elsewhere_keeps_the_held_batch() {
    let (directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    other_process(&store, |value| {
        value.retention = StatisticsRetention::Days30;
    });
    store.flush().unwrap();
    let written = on_disk(&directory);
    assert_eq!(written.total, 2);
    assert_eq!(written.retention, StatisticsRetention::Days30);
}

#[test]
fn reset_changes_the_generation() {
    let (directory, store) = enabled_store();
    let before = on_disk(&directory).generation;
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    let cleared = store.reset().unwrap();
    assert_eq!(cleared.total, 0);
    assert_ne!(cleared.generation, before);
    assert_ne!(cleared.generation, 0);
    assert_eq!(on_disk(&directory), cleared);
}

#[test]
fn pending_commits_are_dropped_when_another_process_turned_statistics_off() {
    let (directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    other_process(&store, |value| value.enabled = false);
    let before = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    store.flush().unwrap();
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        before
    );
    assert_eq!(on_disk(&directory).total, 0);
}

#[test]
fn the_record_that_finds_statistics_turned_off_counts_nothing() {
    let (directory, store) = enabled_store();
    assert_eq!(
        store
            .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
            .unwrap(),
        2
    );
    other_process(&store, |value| value.enabled = false);
    assert_eq!(
        store
            .record_at(
                "世界",
                TypingSource::Quanpin,
                DAY,
                Some(9),
                1_000 + COMMIT_FLUSH_INTERVAL_MS,
            )
            .unwrap(),
        0
    );
    assert_eq!(on_disk(&directory).total, 0);
    // Off is now cached: nothing more is held until the switch is read again.
    assert_eq!(
        store
            .record_at(
                "再见",
                TypingSource::Quanpin,
                DAY,
                Some(9),
                2_000 + COMMIT_FLUSH_INTERVAL_MS,
            )
            .unwrap(),
        0
    );
}

#[test]
fn a_switch_turned_on_elsewhere_is_read_again_after_the_interval() {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    assert_eq!(
        store
            .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
            .unwrap(),
        0
    );
    other_process(&store, |value| value.enabled = true);
    assert_eq!(
        store
            .record_at(
                "世界",
                TypingSource::Quanpin,
                DAY,
                Some(9),
                1_000 + COMMIT_FLUSH_INTERVAL_MS - 1,
            )
            .unwrap(),
        0,
        "within the interval the cached switch still answers"
    );
    assert_eq!(
        store
            .record_at(
                "再见",
                TypingSource::Quanpin,
                DAY,
                Some(9),
                1_000 + COMMIT_FLUSH_INTERVAL_MS,
            )
            .unwrap(),
        2
    );
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).total, 2);
}

#[test]
fn every_other_operation_in_the_process_sees_and_orders_after_pending_commits() {
    let (directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    // A separate instance, as each FFI call builds its own, shares the process's batch.
    let other = TypingStatisticsStore::new(directory.path());
    assert_eq!(other.load().unwrap().total, 2);
    assert_eq!(on_disk(&directory).total, 2);

    store
        .record_at("世界", TypingSource::Quanpin, DAY, Some(9), 2_000)
        .unwrap();
    other
        .record_keys(DAY, &BTreeMap::from([("KeyA".to_owned(), 1)]))
        .unwrap();
    assert_eq!(on_disk(&directory).total, 4);

    // Reset and turning statistics off come after what was typed before them, never before it.
    store
        .record_at("再见", TypingSource::Quanpin, DAY, Some(9), 3_000)
        .unwrap();
    assert_eq!(other.reset().unwrap().total, 0);
    store
        .record_at("又见", TypingSource::Quanpin, DAY, Some(9), 4_000)
        .unwrap();
    assert!(!other.set_enabled(false).unwrap().enabled);
    assert_eq!(on_disk(&directory).total, 2);
    assert_eq!(
        store
            .record_at("关了", TypingSource::Quanpin, DAY, Some(9), 5_000)
            .unwrap(),
        0
    );
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).total, 2);
}

#[test]
fn flush_without_pending_commits_touches_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("never-created");
    TypingStatisticsStore::new(&missing).flush().unwrap();
    assert!(!missing.exists());

    let (directory, store) = enabled_store();
    // Whitespace counts nothing and holds nothing.
    assert_eq!(
        store
            .record_at(" \n", TypingSource::English, DAY, Some(9), 1_000)
            .unwrap(),
        0
    );
    let before = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    store.flush().unwrap();
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        before
    );
}

#[test]
fn the_running_total_includes_what_the_process_still_holds() {
    let (directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    store.flush().unwrap();
    assert_eq!(
        store
            .record_with_total_at("世界", TypingSource::Quanpin, DAY, Some(9), 2_000)
            .unwrap(),
        (2, 4)
    );
    assert_eq!(
        store
            .record_with_total_at("再见了", TypingSource::Quanpin, DAY, Some(9), 3_000)
            .unwrap(),
        (3, 7)
    );
    assert_eq!(on_disk(&directory).total, 2);
}

#[test]
fn the_background_flush_writes_what_the_process_holds() {
    let (directory, store) = enabled_store();
    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    assert_eq!(on_disk(&directory).total, 0);
    flush_pending_commits_in_background();
    // The caller does not wait for the write; poll the file until the background thread has replaced it.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while on_disk(&directory).total != 2 {
        assert!(
            std::time::Instant::now() < deadline,
            "the background flush never wrote the held commit"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// 输入线程交过来的选词只进本进程的批次，不取文件锁、不碰磁盘：目录根本建不出来时照样攒下。
#[test]
fn queueing_selections_never_touches_the_statistics_file() {
    let directory = tempfile::tempdir().unwrap();
    let blocker = directory.path().join("not-a-directory");
    fs::write(&blocker, b"").unwrap();
    let unreachable = TypingStatisticsStore::new(blocker.join("statistics"));
    assert_eq!(unreachable.queue_selections(&[(1, 2)]).unwrap(), None);
    assert_eq!(unreachable.known_enabled(), None);
    // 同一个目录，在当前线程上写的那条路就失败了。
    assert!(unreachable.record_selections(&[(1, 1)]).is_err());

    let (directory, store) = enabled_store();
    let before = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    // `set_enabled` 刚写过文件，本进程知道统计开着。
    assert_eq!(
        store.queue_selections(&[(1, 2), (3, 1), (12, 1)]).unwrap(),
        Some(true)
    );
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        before
    );
    // 整批攒下或都不攒。
    assert!(matches!(
        store.queue_selections(&[(2, 1), (0, 1)]),
        Err(TypingStatisticsError::InvalidPosition)
    ));

    store.flush().unwrap();
    let written = on_disk(&directory).selections;
    assert_eq!(written.ranks[0], 2);
    assert_eq!(written.ranks[1], 0);
    assert_eq!(written.ranks[2], 1);
    assert_eq!(written.beyond, 1);
    assert_eq!(written.total(), 4);
    // 写出一次就不再攒着：再 flush 不碰文件，也不会算两遍。
    let after = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    store.flush().unwrap();
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        after
    );
}

/// 读统计的地方（统计页、上传、另一个 store 实例）先把攒着的选词写进去，所以总看得到它们；满间隔的 `record` 也连同上屏一起写。
#[test]
fn readers_and_the_due_record_write_queued_selections() {
    let (directory, store) = enabled_store();
    store.queue_selections(&[(1, 1)]).unwrap();
    let other = TypingStatisticsStore::new(directory.path());
    assert_eq!(other.load().unwrap().selections.total(), 1);
    assert_eq!(on_disk(&directory).selections.total(), 1);

    store
        .record_at("你好", TypingSource::Quanpin, DAY, Some(9), 1_000)
        .unwrap();
    store.queue_selections(&[(2, 3)]).unwrap();
    assert_eq!(on_disk(&directory).selections.total(), 1);
    store
        .record_at(
            "世界",
            TypingSource::Quanpin,
            DAY,
            Some(9),
            1_000 + COMMIT_FLUSH_INTERVAL_MS,
        )
        .unwrap();
    let written = on_disk(&directory);
    assert_eq!(written.total, 4);
    assert_eq!(written.selections.ranks[1], 3);
    assert_eq!(written.selections.total(), 4);
}

/// 和攒着的上屏一样：写出时文件里的统计已被关掉，或已被清空过，攒着的选词整批丢掉；清空之后再攒的照常写。
#[test]
fn queued_selections_follow_the_switch_and_a_reset_elsewhere() {
    let (directory, store) = enabled_store();
    store.queue_selections(&[(1, 5)]).unwrap();
    other_process(&store, |value| value.enabled = false);
    let before = fs::read(directory.path().join(STATISTICS_FILE)).unwrap();
    store.flush().unwrap();
    assert_eq!(
        fs::read(directory.path().join(STATISTICS_FILE)).unwrap(),
        before
    );
    // 刚读到关着的开关：之后交来的选词当场丢掉，打开以后不会补记关着时的选词。
    assert_eq!(store.queue_selections(&[(1, 1)]).unwrap(), Some(false));
    other_process(&store, |value| value.enabled = true);
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).selections.total(), 0);

    // 本进程重新读到开着的开关以后照常攒。
    assert!(store.load().unwrap().enabled);
    assert_eq!(store.queue_selections(&[(1, 2)]).unwrap(), Some(true));
    other_process_reset(&store);
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).selections.total(), 0);
    store.queue_selections(&[(4, 1)]).unwrap();
    store.flush().unwrap();
    let written = on_disk(&directory).selections;
    assert_eq!(written.ranks[3], 1);
    assert_eq!(written.total(), 1);
}

/// 本进程清空统计时，清空之前交来、还没写出的选词随清空一起删掉，不在清空之后写回来。
#[test]
fn a_reset_in_this_process_clears_queued_selections() {
    let (directory, store) = enabled_store();
    store.queue_selections(&[(1, 2)]).unwrap();
    assert_eq!(store.reset().unwrap().selections.total(), 0);
    store.flush().unwrap();
    assert_eq!(on_disk(&directory).selections.total(), 0);
}

/// 后台 flush 也写只攒着选词、没有上屏的目录。
#[test]
fn the_background_flush_writes_queued_selections() {
    let (directory, store) = enabled_store();
    store.queue_selections(&[(1, 1)]).unwrap();
    flush_pending_commits_in_background();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while on_disk(&directory).selections.total() != 1 {
        assert!(
            std::time::Instant::now() < deadline,
            "the background flush never wrote the queued selection"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
