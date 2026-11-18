//! 使用正常编译的共享库，覆盖宿主调用时的连续输入计数，避免 `cfg(test)` 掩盖平台差异。

use msime_client_services::typing_statistics::{
    SummaryInputs, TypingSource, TypingStatisticsStore,
};

#[test]
fn native_library_tracks_runs_across_flush_and_reopening() {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    store.set_enabled(true).unwrap();
    store
        .record_at("甲乙", TypingSource::Quanpin, "2026-11-16", Some(23), 1_000)
        .unwrap();
    store.flush().unwrap();
    let reopened = TypingStatisticsStore::new(directory.path());
    reopened
        .record_at(
            "丙丁戊",
            TypingSource::Quanpin,
            "2026-11-17",
            Some(0),
            11_000,
        )
        .unwrap();
    let summary = reopened
        .summary("2026-11-17", &SummaryInputs::default())
        .unwrap();
    let run = summary.keys.longest_run.expect("已启用统计应记录连续输入");
    assert_eq!(run.characters, 5);
    assert_eq!(run.day, "2026-11-16");
    reopened
        .record_at("己", TypingSource::Quanpin, "2026-11-17", Some(0), 21_001)
        .unwrap();
    let value = reopened.load().unwrap();
    assert_eq!(value.current_run.characters, 1);
    assert_eq!(value.longest_run.characters, 5);
    assert_eq!(value.longest_run.day, "2026-11-16");
}

#[test]
fn disabled_statistics_does_not_start_or_extend_a_run() {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    assert!(!store.load().unwrap().enabled);
    assert_eq!(
        store
            .record_at("甲", TypingSource::Quanpin, "2026-11-17", None, 1_000)
            .unwrap(),
        0
    );
    assert!(store
        .summary("2026-11-17", &SummaryInputs::default())
        .unwrap()
        .keys
        .longest_run
        .is_none());
    store.set_enabled(true).unwrap();
    store
        .record_at("乙丙", TypingSource::Quanpin, "2026-11-17", None, 2_000)
        .unwrap();
    store.set_enabled(false).unwrap();
    store
        .record_at("丁戊己", TypingSource::Quanpin, "2026-11-17", None, 3_000)
        .unwrap();
    let value = store.load().unwrap();
    assert_eq!(value.total, 2);
    assert_eq!(value.longest_run.characters, 2);
}
