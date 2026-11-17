//! 用正常编译的共享库验证语言分组、旧历史与整批写入，测试数据均为合成计数。

use msime_client_services::typing_statistics::{
    KeyCounts, KeyLanguage, KeySourceCounts, StatisticsRetention, TypingSource,
    TypingStatisticsStore,
};
use serde_json::json;
use std::fs;

const DAY: &str = "2026-11-17";

fn counts(entries: &[(&str, u64)]) -> KeyCounts {
    entries
        .iter()
        .map(|(key, count)| ((*key).into(), *count))
        .collect()
}

fn store() -> (tempfile::TempDir, TypingStatisticsStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    store.set_enabled(true).unwrap();
    (directory, store)
}

#[test]
fn mixed_sources_share_one_batch_and_function_keys_share_one_language() {
    let (_directory, store) = store();
    let sources = KeySourceCounts::from([
        (
            TypingSource::Quanpin,
            counts(&[("KeyA", 2), ("ShiftLeft", 1), ("SoftLayer", 1)]),
        ),
        (TypingSource::Wubi, counts(&[("KeyA", 1)])),
        (
            TypingSource::English,
            counts(&[("KeyA", 3), ("ShiftLeft", 2)]),
        ),
        (TypingSource::Japanese, counts(&[("KeyA", 4)])),
    ]);
    let keys = counts(&[("KeyA", 10), ("ShiftLeft", 3), ("SoftLayer", 1)]);
    assert_eq!(
        store
            .record_keys_by_source(DAY, &keys, Some(&sources))
            .unwrap(),
        14
    );
    let value = store.load().unwrap();
    assert_eq!(value.daily_keys[DAY], keys);
    let languages = &value.daily_key_languages[DAY];
    assert_eq!(languages[&KeyLanguage::Chinese], counts(&[("KeyA", 3)]));
    assert_eq!(languages[&KeyLanguage::English], counts(&[("KeyA", 3)]));
    assert_eq!(languages[&KeyLanguage::Japanese], counts(&[("KeyA", 4)]));
    assert_eq!(
        languages[&KeyLanguage::Common],
        counts(&[("ShiftLeft", 3), ("SoftLayer", 1)])
    );
    assert!(!languages.contains_key(&KeyLanguage::Unknown));
    let serialized = serde_json::to_value(&value).unwrap();
    assert_eq!(serialized["dailyKeyLanguages"][DAY]["chinese"]["KeyA"], 3);
    assert!(serialized["dailyKeyLanguages"][DAY]
        .get("quanpin")
        .is_none());
}

#[test]
fn legacy_history_and_legacy_hosts_never_invent_a_language() {
    let (directory, store) = store();
    fs::write(
        directory.path().join("typing-statistics.json"),
        serde_json::to_vec(&json!({
            "enabled": true, "total": 0, "dailyKeys": {DAY: {"KeyA": 5, "Tab": 2}},
            "dailyKeyLanguages": {DAY: {"english": {"KeyA": 1}}}
        }))
        .unwrap(),
    )
    .unwrap();
    let value = store.load().unwrap();
    assert_eq!(
        value.daily_key_languages[DAY][&KeyLanguage::Unknown]["KeyA"],
        4
    );
    assert_eq!(
        value.daily_key_languages[DAY][&KeyLanguage::Common]["Tab"],
        2
    );
    let keys = counts(&[("KeyA", 1), ("Tab", 1)]);
    store.record_keys(DAY, &keys).unwrap();
    let reopened = TypingStatisticsStore::new(directory.path()).load().unwrap();
    let languages = &reopened.daily_key_languages[DAY];
    assert_eq!(languages[&KeyLanguage::Unknown]["KeyA"], 5);
    assert_eq!(languages[&KeyLanguage::English]["KeyA"], 1);
    assert_eq!(languages[&KeyLanguage::Common]["Tab"], 3);
    assert_eq!(reopened.daily_keys[DAY]["KeyA"], 6);
}

#[test]
fn malformed_or_overflowing_batches_leave_the_file_unchanged() {
    let (directory, store) = store();
    let path = directory.path().join("typing-statistics.json");
    let before = fs::read(&path).unwrap();
    let keys = counts(&[("KeyA", 2)]);
    for sources in [
        KeySourceCounts::new(),
        KeySourceCounts::from([(TypingSource::English, counts(&[("KeyA", 1)]))]),
        KeySourceCounts::from([(TypingSource::English, counts(&[("KeyA", 3)]))]),
        KeySourceCounts::from([(TypingSource::English, counts(&[("KeyB", 2)]))]),
        KeySourceCounts::from([(TypingSource::English, counts(&[("KeyA", 2), ("KeyB", 0)]))]),
        KeySourceCounts::from([(TypingSource::English, counts(&[("private-text", 2)]))]),
        KeySourceCounts::from([(TypingSource::English, counts(&[("KeyA", u64::MAX)]))]),
    ] {
        assert!(store
            .record_keys_by_source(DAY, &keys, Some(&sources))
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
    assert!(store
        .record_keys_by_source("2026-02-30", &keys, None)
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    let max = counts(&[("KeyA", 9_000_000_000_000_000)]);
    let sources = KeySourceCounts::from([(TypingSource::English, max.clone())]);
    store
        .record_keys_by_source(DAY, &max, Some(&sources))
        .unwrap();
    let before = fs::read(&path).unwrap();
    let sources = KeySourceCounts::from([(TypingSource::Japanese, keys.clone())]);
    assert!(store
        .record_keys_by_source(DAY, &keys, Some(&sources))
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn disabling_retention_and_reset_cover_both_totals_and_languages() {
    let (directory, store) = store();
    let keys = counts(&[("KeyA", 2), ("Space", 1)]);
    let sources = KeySourceCounts::from([(TypingSource::Korean, keys.clone())]);
    store
        .record_keys_by_source("2026-10-01", &keys, Some(&sources))
        .unwrap();
    store
        .record_keys_by_source(DAY, &keys, Some(&sources))
        .unwrap();
    let value = store
        .set_retention(StatisticsRetention::Days30, DAY)
        .unwrap();
    assert_eq!(value.daily_keys.len(), 1);
    assert_eq!(value.daily_key_languages.len(), 1);
    assert!(value.daily_key_languages.contains_key(DAY));
    store.set_enabled(false).unwrap();
    let before = fs::read(directory.path().join("typing-statistics.json")).unwrap();
    assert_eq!(
        store
            .record_keys_by_source(DAY, &keys, Some(&sources))
            .unwrap(),
        0
    );
    assert_eq!(
        fs::read(directory.path().join("typing-statistics.json")).unwrap(),
        before
    );
    let value = store.reset().unwrap();
    assert!(!value.enabled);
    assert!(value.daily_keys.is_empty() && value.daily_key_languages.is_empty());
}

#[test]
fn corrupt_group_counts_are_rejected_before_legacy_normalization() {
    let (directory, store) = store();
    for groups in [
        json!({"english": {"KeyA": 3}}),
        json!({"english": {"KeyA": 2}, "japanese": {"KeyA": 1}}),
        json!({"common": {"KeyA": 1}}),
        json!({"english": {"ShiftLeft": 1}}),
        json!({"invented": {"KeyA": 1}}),
        json!({"english": {"NotAKey": 1}}),
    ] {
        fs::write(
            directory.path().join("typing-statistics.json"),
            serde_json::to_vec(&json!({
                "enabled": true, "dailyKeys": {DAY: {"KeyA": 2, "ShiftLeft": 1}},
                "dailyKeyLanguages": {DAY: groups}
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(store.load().is_err());
    }
}
