use super::*;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use msime_engine::{read_learning_sync, write_learning_sync};

fn state(records: impl IntoIterator<Item = LearningSyncRecord>) -> LearningSyncState {
    let mut state = LearningSyncState::default();
    for record in records {
        state.insert(record).unwrap();
    }
    state
}

fn weight(dictionary: &str, key: &str, value: &str, time: u64) -> LearningSyncRecord {
    LearningSyncRecord::Weight {
        dictionary: dictionary.into(),
        key: key.into(),
        value: value.into(),
        weight: 23_000_000_000,
        display: "合成\n\"内容\"".into(),
        updated_at: time,
    }
}

fn protected() -> Vec<LearningSyncRecord> {
    vec![
        LearningSyncRecord::Delete {
            dictionary: "pinyin".into(),
            key: "jia".into(),
            value: "甲".into(),
            updated_at: 1,
        },
        LearningSyncRecord::Pin {
            context: "synthetic".into(),
            value: "乙".into(),
            updated_at: 2,
        },
        LearningSyncRecord::Position {
            context: "synthetic".into(),
            key: "a".into(),
            value: "乙".into(),
            position: 1,
            updated_at: 3,
        },
        LearningSyncRecord::Position {
            context: "synthetic".into(),
            key: "b".into(),
            value: "丙".into(),
            position: 1,
            updated_at: 4,
        },
    ]
}

fn limits() -> Limits {
    Limits {
        records: MAX_LEARNING_SYNC_RECORDS,
        stream: MAX_STREAM_BYTES,
        envelope: MAX_ENVELOPE_BYTES,
    }
}

fn encode(state: &LearningSyncState) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_learning_sync(state, "synthetic", &mut bytes).unwrap();
    bytes
}

fn decoded(prepared: &PreparedUpload) -> LearningSyncState {
    read_learning_sync(prepared.stream()).unwrap().state
}

fn assert_error(
    result: Result<PreparedUpload, LearningSyncReadError>,
    error: LearningSyncReadError,
) {
    match result {
        Err(actual) => assert_eq!(actual, error),
        Ok(_) => panic!("expected preparation failure"),
    }
}

#[test]
fn an_unchanged_stream_keeps_exact_bytes_digest_and_source_state() {
    let original = state(
        protected()
            .into_iter()
            .chain([weight("english", "sample", "Sample", 8)]),
    );
    let before = original.clone();
    let expected = encode(&original);
    let prepared = prepare_upload(&original, "synthetic", 8).unwrap();
    assert_eq!(prepared.stream(), expected);
    assert_eq!(prepared.sha256, hex::encode(Sha256::digest(&expected)));
    assert_eq!(prepared.dropped_weights, 0);
    assert_eq!(prepared.records, 5);
    assert_eq!(prepared.observed_max, 8);
    assert_eq!(original, before);
}

#[test]
fn record_limits_drop_oldest_weights_with_all_tie_breakers_and_keep_hidden_positions() {
    let ordered = [
        weight("wubi", "z", "甲", 1),
        weight("english", "a", "A", 7),
        weight("english", "a", "B", 7),
        weight("english", "b", "A", 7),
        weight("pinyin", "a", "甲", 7),
    ];
    for omitted in 1..=ordered.len() {
        let source = state(protected().into_iter().chain(ordered.clone()));
        let expected = state(protected().into_iter().chain(ordered[omitted..].to_vec()));
        let prepared = prepare(
            &source,
            "synthetic",
            Limits {
                records: 9 - omitted,
                ..limits()
            },
        )
        .unwrap();
        assert_eq!(decoded(&prepared), expected);
        assert_eq!(prepared.dropped_weights, omitted);
        assert_eq!(prepared.records, 9 - omitted);
        assert_eq!(source.records().len(), 9);
    }
}

#[test]
fn plaintext_limit_keeps_a_complete_valid_stream_and_digest_of_retained_rows() {
    let older = weight("english", "a", "A", 1);
    let newer = weight("english", "b", "B", 2);
    let source = state([older, newer.clone()]);
    let expected = state([newer]);
    let bytes = encode(&expected);
    let prepared = prepare(
        &source,
        "synthetic",
        Limits {
            stream: bytes.len(),
            ..limits()
        },
    )
    .unwrap();
    assert_eq!(prepared.stream(), bytes);
    assert_eq!(decoded(&prepared), expected);
    assert_eq!(prepared.sha256, hex::encode(Sha256::digest(&bytes)));
    assert_ne!(
        prepared.sha256,
        hex::encode(Sha256::digest(encode(&source)))
    );
}

#[test]
fn actual_gzip_and_padding_limits_retain_a_deterministic_oldest_prefix() {
    let weights: Vec<_> = (0..24)
        .map(|index| {
            let value: String = (0..24)
                .map(|part| hex::encode(Sha256::digest(format!("synthetic-{index}-{part}"))))
                .collect();
            weight("english", &format!("sample-{index:02}"), &value, index + 10)
        })
        .collect();
    let source = state(protected().into_iter().chain(weights.clone()));
    let reference = state(protected().into_iter().chain(weights[12..].to_vec()));
    let envelope = blob_envelope_size(&encode(&reference)).unwrap();
    let first = prepare(
        &source,
        "synthetic",
        Limits {
            envelope,
            ..limits()
        },
    )
    .unwrap();
    let reversed = state(protected().into_iter().chain(weights.iter().rev().cloned()));
    let second = prepare(
        &reversed,
        "synthetic",
        Limits {
            envelope,
            ..limits()
        },
    )
    .unwrap();
    assert!(first.dropped_weights > 0 && first.dropped_weights < weights.len());
    assert!(blob_envelope_size(first.stream()).unwrap() <= envelope);
    assert_eq!(first.stream(), second.stream());
    assert_eq!(first.sha256, second.sha256);
    assert_eq!(first.observed_max, source.observed_max());
    assert_eq!(
        decoded(&first),
        state(
            protected()
                .into_iter()
                .chain(weights[first.dropped_weights..].to_vec())
        )
    );
}

#[test]
fn protected_records_are_never_dropped_to_satisfy_any_limit() {
    let source = state(protected());
    let bytes = encode(&source);
    for bound in [
        Limits {
            records: 3,
            ..limits()
        },
        Limits {
            stream: bytes.len() - 1,
            ..limits()
        },
        Limits {
            envelope: blob_envelope_size(&bytes).unwrap() - 1,
            ..limits()
        },
    ] {
        assert_error(
            prepare(&source, "synthetic", bound),
            LearningSyncReadError::TooLarge,
        );
        let with_weight = state(
            protected()
                .into_iter()
                .chain([weight("english", "a", "A", 10)]),
        );
        assert_error(
            prepare(&with_weight, "synthetic", bound),
            LearningSyncReadError::TooLarge,
        );
    }
    assert_eq!(source, state(protected()));
}

#[test]
fn empty_state_and_invalid_content_ids_follow_the_stream_contract() {
    let source = LearningSyncState::default();
    let prepared = prepare_upload(&source, "synthetic", source.observed_max()).unwrap();
    assert_eq!(decoded(&prepared), source);
    assert_eq!(
        (
            prepared.records,
            prepared.observed_max,
            prepared.dropped_weights
        ),
        (0, 0, 0)
    );
    for content in ["", "synthetic\0invalid"] {
        assert_error(
            prepare_upload(&source, content, source.observed_max()),
            LearningSyncReadError::Invalid,
        );
    }
}

#[test]
fn prepared_bytes_seal_and_authenticate_with_the_planned_generation() {
    // 与 learning_e2e 已知答案相同的合成身份和账号密钥，不接触系统密钥存储。
    let keys = Keystore::parse(&format!(
        "v1.{}.623094f752e30f5a8cc9ca3b612d1a42.{}.pOCSkrZRwni5dyxWn1-puxPZBrRqtoyd-dwrRAn4ogk.confirmed",
        URL_SAFE_NO_PAD.encode([1u8; 32]),
        URL_SAFE_NO_PAD.encode([2u8; 32]),
    )).unwrap();
    let source = state(protected());
    let prepared = prepare_upload(&source, "synthetic", source.observed_max()).unwrap();
    let sealed = prepared.seal(&keys, 17).unwrap();
    assert_eq!(sealed.len(), blob_envelope_size(prepared.stream()).unwrap());
    let opened = keys.open_blob(&keys.device_id(), &sealed).unwrap();
    assert_eq!(opened.generation, 17);
    assert_eq!(opened.stream, prepared.stream());
    assert_eq!(
        read_learning_sync(opened.stream.as_slice()).unwrap().state,
        source
    );
    assert_eq!(prepared.seal(&keys, 0), Err(SealError::InvalidGeneration));
    let unapproved = Keystore::generate().unwrap();
    assert_eq!(prepared.seal(&unapproved, 17), Err(SealError::NotReady));
}

#[test]
fn export_observation_includes_unknown_dictionaries_and_invalid_maxima_are_rejected() {
    let source = state([weight("english", "sample", "Sample", 10)]);
    let prepared = prepare_upload(&source, "synthetic", 999).unwrap();
    assert_eq!(prepared.observed_max, 999);
    assert_eq!(decoded(&prepared), source);
    for maximum in [9, MAX_LEARNING_SYNC_TIMESTAMP + 1] {
        assert_error(
            prepare_upload(&source, "synthetic", maximum),
            LearningSyncReadError::Invalid,
        );
    }
}
