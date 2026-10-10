//! Unit tests for the per-key press counts (`dailyKeys`) of the parent module.

use super::*;

fn store() -> (tempfile::TempDir, TypingStatisticsStore) {
    let directory = tempfile::tempdir().expect("tempdir");
    let store = TypingStatisticsStore::new(directory.path());
    // These tests are about counting, not about the default. Statistics ship off.
    store.set_enabled(true).expect("enable");
    (directory, store)
}

fn keys(entries: &[(&str, u64)]) -> BTreeMap<String, u64> {
    entries
        .iter()
        .map(|(key, count)| ((*key).to_owned(), *count))
        .collect()
}

#[test]
fn the_whitelist_is_the_published_contract() {
    assert_eq!(KEY_IDS.len(), 141);
    let unique: HashSet<&str> = KEY_IDS.iter().copied().collect();
    assert_eq!(unique.len(), KEY_IDS.len());
    // Every id must also pass the document's own key-name rule shape: plain ASCII alphanumerics, which is what lets a settings page use them as lookup keys verbatim.
    assert!(KEY_IDS
        .iter()
        .all(|id| !id.is_empty() && id.bytes().all(|byte| byte.is_ascii_alphanumeric())));
    for id in [
        "KeyA",
        "KeyZ",
        "Digit0",
        "Semicolon",
        "IntlRo",
        "Space",
        "ShiftLeft",
        "Fn",
        "F12",
        "NumpadEnter",
        "NumLock",
        "Nine1",
        "SoftPunctuation",
        "SoftVoice",
        "FourteenQW",
        "FourteenL",
        "FourteenM",
    ] {
        assert!(is_known_key_id(id), "{id}");
    }
    for id in [
        "",
        "keya",
        "KeyAA",
        "F13",
        "Nine",
        "a",
        ";",
        "ShiftRight ",
        "Unidentified",
        "FourteenWQ",
        "FourteenQ",
    ] {
        assert!(!is_known_key_id(id), "{id:?}");
    }
}

#[test]
fn records_presses_per_key_per_day_and_adds_to_them() {
    let (directory, store) = store();
    assert_eq!(
        store
            .record_keys("2026-09-30", &keys(&[("KeyA", 3), ("Space", 2)]))
            .unwrap(),
        5
    );
    assert_eq!(
        store
            .record_keys("2026-09-30", &keys(&[("KeyA", 1), ("Backspace", 4)]))
            .unwrap(),
        5
    );
    let value = store.load().unwrap();
    assert_eq!(
        value.daily_keys["2026-09-30"],
        keys(&[("KeyA", 4), ("Space", 2), ("Backspace", 4)])
    );
    // Key presses are not characters: nothing on the character axes moves.
    assert_eq!(value.total, 0);
    assert!(value.days.is_empty());
    let persisted = fs::read_to_string(directory.path().join("typing-statistics.json")).unwrap();
    assert!(persisted.contains(r#""dailyKeys":{"2026-09-30":{"Backspace":4,"KeyA":4,"Space":2}}"#));
}

#[test]
fn nothing_is_written_while_statistics_are_off() {
    let directory = tempfile::tempdir().unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    assert_eq!(
        store
            .record_keys("2026-09-30", &keys(&[("KeyA", 3)]))
            .unwrap(),
        0
    );
    assert!(!directory.path().join("typing-statistics.json").exists());
    store.set_enabled(true).unwrap();
    store.set_enabled(false).unwrap();
    let before = fs::read(directory.path().join("typing-statistics.json")).unwrap();
    assert_eq!(
        store
            .record_keys("2026-09-30", &keys(&[("KeyA", 3)]))
            .unwrap(),
        0
    );
    assert_eq!(
        fs::read(directory.path().join("typing-statistics.json")).unwrap(),
        before
    );
    assert!(store.load().unwrap().daily_keys.is_empty());
}

#[test]
fn an_empty_batch_touches_nothing() {
    let (directory, store) = store();
    let before = fs::read(directory.path().join("typing-statistics.json")).unwrap();
    assert_eq!(
        store.record_keys("2026-09-30", &BTreeMap::new()).unwrap(),
        0
    );
    assert_eq!(
        fs::read(directory.path().join("typing-statistics.json")).unwrap(),
        before
    );
}

#[test]
fn an_unknown_id_or_a_zero_count_rejects_the_whole_batch() {
    let (_directory, store) = store();
    store
        .record_keys("2026-09-30", &keys(&[("KeyA", 1)]))
        .unwrap();
    for batch in [
        keys(&[("KeyA", 5), ("Unidentified", 1)]),
        keys(&[("KeyA", 5), ("a", 1)]),
        keys(&[("KeyA", 5), ("Space", 0)]),
    ] {
        assert!(matches!(
            store.record_keys("2026-09-30", &batch),
            Err(TypingStatisticsError::InvalidKey)
        ));
    }
    // The valid part of a rejected batch was not kept.
    assert_eq!(
        store.load().unwrap().daily_keys["2026-09-30"],
        keys(&[("KeyA", 1)])
    );
    assert!(matches!(
        store.record_keys("2026-9-30", &keys(&[("KeyA", 1)])),
        Err(TypingStatisticsError::InvalidDay)
    ));
}

#[test]
fn an_exhausted_count_rejects_the_whole_batch() {
    let (_directory, store) = store();
    store
        .record_keys("2026-09-30", &keys(&[("KeyA", MAX_COUNT)]))
        .unwrap();
    assert!(matches!(
        store.record_keys("2026-09-30", &keys(&[("KeyA", 1), ("KeyB", 1)])),
        Err(TypingStatisticsError::CountExhausted)
    ));
    assert_eq!(
        store.load().unwrap().daily_keys["2026-09-30"],
        keys(&[("KeyA", MAX_COUNT)])
    );
}

#[test]
fn a_day_with_keys_and_no_characters_is_valid_and_round_trips() {
    let (directory, store) = store();
    store
        .record_at("字", TypingSource::Quanpin, "2026-09-29", Some(9), 1_000)
        .unwrap();
    store
        .record_keys("2026-09-30", &keys(&[("ArrowDown", 7)]))
        .unwrap();
    let value = store.load().unwrap();
    assert!(!value.days.contains_key("2026-09-30"));
    assert_eq!(value.daily_keys["2026-09-30"]["ArrowDown"], 7);
    let path = directory.path().join("typing-statistics.json");
    let parsed: TypingStatistics = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(parsed, value);
    assert_eq!(
        serde_json::from_slice::<TypingStatistics>(&serde_json::to_vec(&value).unwrap()).unwrap(),
        value
    );
}

#[test]
fn a_document_written_before_key_counts_loads_without_them() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("typing-statistics.json"),
        r#"{"enabled":true,"total":3,"days":{"2026-09-07":3},"dailyHours":{"2026-09-07":[0,0,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,0,0,0,0,0,0,0]}}"#,
    )
    .unwrap();
    let store = TypingStatisticsStore::new(directory.path());
    let value = store.load().unwrap();
    assert!(value.daily_keys.is_empty());
    assert_eq!(value.total, 3);
    store
        .record_keys("2026-09-07", &keys(&[("KeyN", 1)]))
        .unwrap();
    let value = store.load().unwrap();
    assert_eq!(value.total, 3);
    assert_eq!(value.daily_keys["2026-09-07"]["KeyN"], 1);
}

#[test]
fn rejects_documents_with_key_counts_outside_the_contract() {
    for document in [
        r#"{"dailyKeys":{"2026-09-31":{"KeyA":1}}}"#,
        r#"{"dailyKeys":{"yesterday":{"KeyA":1}}}"#,
        r#"{"dailyKeys":{"2026-09-30":{"a":1}}}"#,
        r#"{"dailyKeys":{"2026-09-30":{"KeyA":9000000000000000001}}}"#,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("typing-statistics.json");
        fs::write(&path, document).unwrap();
        let store = TypingStatisticsStore::new(directory.path());
        assert!(store.load().is_err(), "{document}");
        // A rejected document is never overwritten by a write that could not read it.
        assert!(store
            .record_keys("2026-09-30", &keys(&[("KeyA", 1)]))
            .is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), document);
    }
}

#[test]
fn presses_before_and_after_midnight_stay_on_their_own_days() {
    let (_directory, store) = store();
    // A host that batched across midnight flushes the old day first, under the old day.
    store
        .record_keys("2026-09-30", &keys(&[("KeyA", 2), ("Enter", 1)]))
        .unwrap();
    store
        .record_keys("2026-10-01", &keys(&[("KeyA", 5)]))
        .unwrap();
    let value = store.load().unwrap();
    assert_eq!(
        value.daily_keys["2026-09-30"],
        keys(&[("KeyA", 2), ("Enter", 1)])
    );
    assert_eq!(value.daily_keys["2026-10-01"], keys(&[("KeyA", 5)]));
}

#[test]
fn retention_prunes_key_days_and_reset_clears_them() {
    let (_directory, store) = store();
    for day in ["2026-06-01", "2026-08-25", "2026-09-20"] {
        store.record_keys(day, &keys(&[("KeyQ", 1)])).unwrap();
    }
    let narrowed = store
        .set_retention(StatisticsRetention::Days30, "2026-09-21")
        .unwrap();
    assert_eq!(
        narrowed.daily_keys.keys().collect::<Vec<_>>(),
        ["2026-08-25", "2026-09-20"]
    );

    // The first key flush of a later day carries the window with it, as the first commit of a day does.
    store
        .record_keys("2026-09-25", &keys(&[("KeyQ", 1)]))
        .unwrap();
    let moved = store.load().unwrap();
    assert_eq!(
        moved.daily_keys.keys().collect::<Vec<_>>(),
        ["2026-09-20", "2026-09-25"]
    );
    assert_eq!(moved.last_pruned_day, "2026-09-25");

    let reset = store.reset().unwrap();
    assert!(reset.daily_keys.is_empty());
    assert!(store.load().unwrap().daily_keys.is_empty());
}
