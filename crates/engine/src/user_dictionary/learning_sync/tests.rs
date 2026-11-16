use std::io::{self, BufReader, Cursor, Read};

use super::*;

fn weight(timestamp: u64, weight: u64, display: &str) -> LearningSyncRecord {
    LearningSyncRecord::Weight {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        weight,
        display: display.into(),
        updated_at: timestamp,
    }
}

fn deleted(timestamp: u64) -> LearningSyncRecord {
    LearningSyncRecord::Delete {
        dictionary: "pinyin".into(),
        key: "jia".into(),
        value: "甲".into(),
        updated_at: timestamp,
    }
}

fn position(value: &str, slot: u8, timestamp: u64) -> LearningSyncRecord {
    LearningSyncRecord::Position {
        context: "synthetic".into(),
        key: value.into(),
        value: value.into(),
        position: slot,
        updated_at: timestamp,
    }
}

fn state(records: impl IntoIterator<Item = LearningSyncRecord>) -> LearningSyncState {
    let mut state = LearningSyncState::default();
    for record in records {
        state.insert(record).unwrap();
    }
    state
}

fn encoded(state: &LearningSyncState) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_learning_sync(state, "synthetic-content", &mut bytes).unwrap();
    bytes
}

fn stream(records: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = b"{\"type\":\"header\",\"format\":\"msime-learning-sync\",\"version\":1,\"content_id\":\"synthetic-content\"}\n".to_vec();
    for record in records {
        serde_json::to_writer(&mut bytes, record).unwrap();
        bytes.push(b'\n');
    }
    serde_json::to_writer(
        &mut bytes,
        &serde_json::json!({"type":"footer", "records": records.len()}),
    )
    .unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn operations_compare_time_then_delete_then_weight_and_display() {
    for (left, right, expected) in [
        (deleted(10), weight(11, 1, ""), weight(11, 1, "")),
        (weight(10, 900, "z"), deleted(10), deleted(10)),
        (weight(10, 5, "z"), weight(10, 6, "a"), weight(10, 6, "a")),
        (weight(10, 5, "a"), weight(10, 5, "z"), weight(10, 5, "z")),
    ] {
        for records in [[left.clone(), right.clone()], [right, left]] {
            assert_eq!(
                state(records).records().collect::<Vec<_>>(),
                vec![&expected]
            );
        }
    }
}

#[test]
fn mixed_records_form_an_idempotent_commutative_and_associative_merge() {
    let pin = |value: &str, updated_at| LearningSyncRecord::Pin {
        context: "synthetic".into(),
        value: value.into(),
        updated_at,
    };
    let devices = [
        state([weight(2, 7, "a"), pin("a", 3), position("a", 1, 2)]),
        state([deleted(2), pin("b", 3), position("a", 2, 3)]),
        state([weight(3, 1, "b"), pin("z", 2), position("b", 1, 4)]),
    ];
    let expected = state([
        weight(3, 1, "b"),
        pin("b", 3),
        position("a", 2, 3),
        position("b", 1, 4),
    ]);
    for [a, b, c] in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut left = devices[a].clone();
        left.combine(&devices[b]);
        left.combine(&devices[c]);
        let mut pair = devices[b].clone();
        pair.combine(&devices[c]);
        let mut right = devices[a].clone();
        right.combine(&pair);
        assert_eq!(left, expected);
        assert_eq!(right, expected);
        left.combine(&left.clone());
        assert_eq!(left, expected);
    }
}

#[test]
fn pins_compare_time_then_value_and_positions_prefer_smaller_slots() {
    let pin = |value: &str, updated_at| LearningSyncRecord::Pin {
        context: "synthetic".into(),
        value: value.into(),
        updated_at,
    };
    for (a, b, winner) in [
        (pin("b", 1), pin("a", 2), pin("a", 2)),
        (pin("a", 1), pin("b", 1), pin("b", 1)),
        (
            position("a", 1, 1),
            position("a", 2, 2),
            position("a", 2, 2),
        ),
        (
            position("a", 2, 1),
            position("a", 1, 1),
            position("a", 1, 1),
        ),
    ] {
        let combined = state([a.clone(), b.clone()]);
        assert_eq!(combined, state([b, a]));
        assert_eq!(combined, state([winner.clone()]));
        assert_eq!(state([winner.clone()]).records().next(), Some(&winner));
        let mut combined = state([winner.clone()]);
        combined.combine(&combined.clone());
        assert_eq!(combined.records().collect::<Vec<_>>(), vec![&winner]);
    }
}

#[test]
fn combine_retains_hidden_positions_for_all_permutations_and_parenthesizations() {
    // 协议里的反例：过早投影会使 (A+B)+C 与 A+(B+C) 得到不同的候选状态。
    let devices = [
        state([position("b", 2, 1)]),
        state([position("a", 2, 1)]),
        state([position("a", 1, 1)]),
    ];
    let expected = state([position("a", 1, 1), position("b", 2, 1)]);
    for [a, b, c] in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut left = devices[a].clone();
        left.combine(&devices[b]);
        left.combine(&devices[c]);
        let mut right = devices[b].clone();
        right.combine(&devices[c]);
        right.combine(&devices[a]);
        assert_eq!(left, expected);
        assert_eq!(right, expected);
        assert_eq!(encoded(&left), encoded(&right));
        let restored = read_learning_sync(Cursor::new(encoded(&left)))
            .unwrap()
            .state;
        assert_eq!(restored, expected);
    }
    let hidden = state([position("b", 2, 1), position("a", 2, 1)]);
    assert_eq!(hidden.position_projection(), vec![&position("a", 2, 1)]);
    assert_eq!(hidden.records().len(), 2);
    let mut restored = read_learning_sync(Cursor::new(encoded(&hidden)))
        .unwrap()
        .state;
    restored.combine(&devices[2]);
    assert_eq!(
        restored.position_projection().len(),
        2,
        "保存再读入仍能恢复槽位冲突的输家"
    );
}

#[test]
fn projection_uses_time_before_identity_and_never_changes_state() {
    let complete = state([position("a", 2, 1), position("z", 2, 2)]);
    let before = encoded(&complete);
    assert_eq!(complete.position_projection(), vec![&position("z", 2, 2)]);
    assert_eq!(encoded(&complete), before);
}

#[test]
fn reset_floor_filters_even_hidden_positions_and_same_second_logical_times() {
    let complete = state([
        weight(103, 10, ""),
        position("a", 1, 103),
        position("b", 1, 104),
        deleted(104),
    ]);
    assert_eq!(complete.observed_max(), 104);
    assert_eq!(
        complete.after_reset_floor(103),
        state([position("b", 1, 104), deleted(104)])
    );
    assert_eq!(
        complete.after_reset_floor(104),
        LearningSyncState::default()
    );
    assert_eq!(complete.after_reset_floor(0), complete);
}

#[test]
fn stream_matches_fixed_ndjson_bytes_and_sorts_operation_kinds_together() {
    let input = state([
        position("b", 2, 4),
        deleted(2),
        LearningSyncRecord::Pin {
            context: "synthetic".into(),
            value: "甲".into(),
            updated_at: 3,
        },
        LearningSyncRecord::Weight {
            dictionary: "english".into(),
            key: "sample".into(),
            value: "Sample".into(),
            weight: 9,
            display: "".into(),
            updated_at: 1,
        },
    ]);
    let expected = concat!(
        "{\"type\":\"header\",\"format\":\"msime-learning-sync\",\"version\":1,\"content_id\":\"synthetic-content\"}\n",
        "{\"type\":\"weight\",\"dictionary\":\"english\",\"key\":\"sample\",\"value\":\"Sample\",\"weight\":9,\"display\":\"\",\"updated_at\":1}\n",
        "{\"type\":\"delete\",\"dictionary\":\"pinyin\",\"key\":\"jia\",\"value\":\"甲\",\"updated_at\":2}\n",
        "{\"type\":\"pin\",\"context\":\"synthetic\",\"value\":\"甲\",\"updated_at\":3}\n",
        "{\"type\":\"position\",\"context\":\"synthetic\",\"key\":\"b\",\"value\":\"b\",\"position\":2,\"updated_at\":4}\n",
        "{\"type\":\"footer\",\"records\":4}\n",
    );
    assert_eq!(encoded(&input), expected.as_bytes());
    let read = read_learning_sync(Cursor::new(expected)).unwrap();
    assert_eq!(read.state, input);
    assert_eq!(read.observed_max, 4);
    assert_eq!(read.skipped, 0);
}

#[test]
fn unknown_records_are_counted_and_known_fields_still_validate() {
    let mut unknown_dictionary = serde_json::to_value(weight(19, 1, "")).unwrap();
    unknown_dictionary["dictionary"] = "future-kind".into();
    let read = read_learning_sync(Cursor::new(stream(&[
        serde_json::json!({"type":"future-record", "payload":"synthetic"}),
        unknown_dictionary.clone(),
        serde_json::to_value(deleted(10)).unwrap(),
    ])))
    .unwrap();
    assert_eq!(read.skipped, 2);
    assert_eq!(read.observed_max, 19);
    assert_eq!(read.state, state([deleted(10)]));
    unknown_dictionary["updated_at"] = 0.into();
    assert_eq!(
        read_learning_sync(Cursor::new(stream(&[unknown_dictionary]))),
        Err(LearningSyncReadError::Invalid)
    );
}

#[test]
fn malformed_or_out_of_range_records_never_return_partial_state() {
    let valid = serde_json::to_value(weight(10, 5, "")).unwrap();
    for (field, bad) in [
        ("weight", serde_json::json!(0)),
        ("weight", serde_json::json!(100_000_001)),
        ("updated_at", serde_json::json!(0)),
        ("updated_at", serde_json::json!((1u64 << 40) + 1)),
        ("updated_at", serde_json::json!(1.5)),
        ("updated_at", serde_json::json!(-1)),
        ("updated_at", serde_json::json!("10")),
        ("key", serde_json::json!("")),
        ("key", serde_json::json!("a".repeat(513))),
        ("value", serde_json::json!("甲".repeat(683))),
        ("display", serde_json::json!("a".repeat(2049))),
        ("value", serde_json::json!("a\0b")),
    ] {
        let mut record = valid.clone();
        record[field] = bad;
        assert_eq!(
            read_learning_sync(Cursor::new(stream(&[valid.clone(), record]))),
            Err(LearningSyncReadError::Invalid),
            "{field}"
        );
    }
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("display");
    assert_eq!(
        read_learning_sync(Cursor::new(stream(&[missing]))),
        Err(LearningSyncReadError::Invalid)
    );
    for slot in [0, 6] {
        assert!(state([]).insert(position("a", slot, 1)).is_err());
    }
    assert!(state([])
        .insert(LearningSyncRecord::Pin {
            context: "a".repeat(1025),
            value: "甲".into(),
            updated_at: 1
        })
        .is_err());
    assert!(state([])
        .insert(weight(MAX_LEARNING_SYNC_TIMESTAMP, 100_000_000, ""))
        .is_ok());
}

#[test]
fn stream_rejects_truncation_extra_content_and_false_counts() {
    let bytes = encoded(&state([weight(10, 5, "")]));
    let text = String::from_utf8(bytes.clone()).unwrap();
    for bad in [
        text.lines().take(2).collect::<Vec<_>>().join("\n"),
        text.replace("\"records\":1", "\"records\":2"),
        format!("{text}\n"),
        format!("{text}{{\"type\":\"footer\",\"records\":1}}\n"),
        text.replace("\"version\":1", "\"version\":0"),
        text.replace("msime-learning-sync", "another-format"),
        text.replace("\"weight\":5", "\"weight\":5,\"weight\":6"),
        text.replace("\"records\":1", "\"records\":1,\"records\":1"),
    ] {
        assert_eq!(
            read_learning_sync(Cursor::new(bad)),
            Err(LearningSyncReadError::Invalid)
        );
    }
    assert_eq!(
        read_learning_sync(Cursor::new(text.replace("\"version\":1", "\"version\":2"))),
        Err(LearningSyncReadError::UnsupportedVersion)
    );
    assert!(read_learning_sync(Cursor::new(text.trim_end_matches('\n'))).is_ok());
    assert_eq!(
        read_learning_sync(Cursor::new(b"")),
        Err(LearningSyncReadError::Invalid)
    );
}

#[test]
fn record_limit_counts_duplicates_and_unknown_records() {
    let header = stream(&[]);
    let header = &header[..header.iter().position(|&byte| byte == b'\n').unwrap() + 1];
    let records = b"{\"type\":\"future\"}\n".repeat(500_001);
    let input = Cursor::new(header).chain(Cursor::new(records));
    assert_eq!(
        read_learning_sync(BufReader::new(input)),
        Err(LearningSyncReadError::TooLarge)
    );
}

#[test]
fn plaintext_limit_bounds_a_single_unterminated_line() {
    let input = io::repeat(b' ').take(128 * 1024 * 1024 + 1);
    assert_eq!(
        read_learning_sync(BufReader::new(input)),
        Err(LearningSyncReadError::TooLarge)
    );
}

#[test]
fn io_errors_are_sanitized_and_do_not_publish_partial_results() {
    struct Failed;
    impl Read for Failed {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("synthetic-private-data"))
        }
    }
    let error = read_learning_sync(BufReader::new(Failed)).unwrap_err();
    assert_eq!(error, LearningSyncReadError::Io);
    assert!(!error.to_string().contains("synthetic-private-data"));
}
