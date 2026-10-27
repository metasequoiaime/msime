use super::*;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const KEY1: &str = "11111111111111111111111111111111";
const KEY2: &str = "22222222222222222222222222222222";
const LINEAGE: &str = "0123456789abcdef0123456789abcdef";
const AT: i64 = 1_790_000_000;

fn key(id: &str) -> DataKey {
    DataKey::new(id, &[id.as_bytes()[0]; 32]).unwrap()
}

fn word(code: &str, text: &str) -> WordKey {
    WordKey::new(WordKind::Pinyin, code, text).unwrap()
}

fn op(code: &str, live: bool, millis: u64) -> Op {
    Op {
        key: word(code, &"字".repeat(code.split('\'').count())),
        live,
        hlc: Hlc(millis << 10),
    }
}

fn draft(ops: Vec<Op>) -> Draft {
    Draft {
        ops,
        ack: 0,
        retire: Vec::new(),
    }
}

fn genesis() -> Ledger {
    Ledger::genesis(LINEAGE, KEY1)
}

#[test]
fn an_event_written_by_one_device_replays_identically_on_another() {
    let mut writer = genesis();
    let mut reader = genesis();
    let bytes = writer.event(
        A,
        &draft(vec![op("shui'shan", true, 10), op("ma", false, 11)]),
        AT,
        &key(KEY1),
        [1; NONCE_BYTES],
    );
    let written = writer.apply(&bytes, Some(&key(KEY1))).unwrap();
    let read = reader.apply(&bytes, Some(&key(KEY1))).unwrap();
    assert_eq!(written, read);
    assert_eq!(read.device.as_deref(), Some(A));
    assert_eq!(read.ops.len(), 2);
    assert_eq!(writer, reader);
    assert_eq!(reader.seq, 1);
    assert_eq!(reader.head, sha256_hex(&bytes));
    assert!(reader.replica.is_live(&op("shui'shan", true, 10).key));
    assert_eq!(reader.replica.devices[A].seen, AT);
    // 网盘上只有密文：词和编码都看不出来。
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("shui") && !text.contains('字'), "{text}");
}

#[test]
fn an_event_that_does_not_follow_the_chain_is_refused_without_moving() {
    let mut ledger = genesis();
    let other = Ledger {
        head: "f".repeat(64),
        ..genesis()
    };
    let bytes = other.event(A, &draft(vec![]), AT, &key(KEY1), [1; NONCE_BYTES]);
    let before = ledger.clone();
    assert_eq!(ledger.apply(&bytes, Some(&key(KEY1))), Err(LogError::Chain));
    assert_eq!(ledger, before);
}

#[test]
fn unreadable_events_occupy_their_sequence_number_without_changing_state() {
    let mut ledger = genesis();
    let writer = genesis();
    let good = writer.event(
        A,
        &draft(vec![op("ma", true, 10)]),
        AT,
        &key(KEY1),
        [1; NONCE_BYTES],
    );
    // 改了头部的写入时间：aad 对不上，解不开。
    let mut tampered: serde_json::Value = serde_json::from_slice(&good).unwrap();
    tampered["at"] = serde_json::json!(AT + 1);
    let tampered = serde_json::to_vec(&tampered).unwrap();
    // 用另一把密钥写的、序号不对的、格式版本不认识的、根本不是 JSON 的。
    let mut wrong_key = writer.event(
        A,
        &draft(vec![op("ma", true, 10)]),
        AT,
        &key(KEY2),
        [1; NONCE_BYTES],
    );
    let mut value: serde_json::Value = serde_json::from_slice(&wrong_key).unwrap();
    value["key_id"] = serde_json::json!(KEY1);
    wrong_key = serde_json::to_vec(&value).unwrap();
    for bytes in [tampered, wrong_key, b"not json".to_vec()] {
        let mut probe = genesis();
        probe.apply(&bytes, Some(&key(KEY1))).unwrap();
        assert_eq!(probe.seq, 1);
        assert_eq!(probe.head, sha256_hex(&bytes));
        assert_eq!(probe.replica, Replica::default());
    }
    let mut future: serde_json::Value = serde_json::from_slice(&good).unwrap();
    future["version"] = serde_json::json!(2);
    let future = serde_json::to_vec(&future).unwrap();
    ledger.apply(&future, Some(&key(KEY1))).unwrap();
    assert_eq!(ledger.replica, Replica::default());
    assert_eq!(ledger.seq, 1);
}

#[test]
fn an_event_under_another_epoch_key_is_skipped_but_a_missing_key_stops_the_reader() {
    let writer = Ledger::genesis(LINEAGE, KEY2);
    let bytes = writer.event(
        A,
        &draft(vec![op("ma", true, 10)]),
        AT,
        &key(KEY2),
        [1; NONCE_BYTES],
    );
    let mut ledger = genesis();
    ledger.apply(&bytes, Some(&key(KEY1))).unwrap();
    assert_eq!(ledger.seq, 1);
    assert_eq!(ledger.replica, Replica::default());

    let bytes = genesis().event(
        A,
        &draft(vec![op("ma", true, 10)]),
        AT,
        &key(KEY1),
        [1; NONCE_BYTES],
    );
    let mut ledger = genesis();
    assert_eq!(ledger.apply(&bytes, None), Err(LogError::MissingKey));
    assert_eq!(ledger, genesis());
}

#[test]
fn invalid_operations_are_skipped_one_by_one_and_oversized_events_entirely() {
    let ledger = genesis();
    let payload = serde_json::json!({
        "device": A,
        "ack": 0,
        "ops": [
            ["pinyin", "ma", "字", 10u64 << 10, true],
            ["pinyin", "MA", "字", 10u64 << 10, true],
            ["bundled", "ma", "字", 10u64 << 10, true],
            ["pinyin", "ma", "字", 0, true],
            ["pinyin", "ma", "字", 10u64 << 10],
            "garbage"
        ]
    });
    let boxed = key(KEY1).seal(
        Purpose::Event,
        &serde_json::to_vec(&payload).unwrap(),
        &event_aad(1, GENESIS_HEAD, AT, KEY1),
        [1; NONCE_BYTES],
    );
    let bytes = serde_json::to_vec(&serde_json::json!({
        "format": FORMAT, "version": 1, "seq": 1, "prev": GENESIS_HEAD, "at": AT,
        "type": "ops", "key_id": KEY1, "box": boxed
    }))
    .unwrap();
    let mut reader = ledger.clone();
    let received = reader.apply(&bytes, Some(&key(KEY1))).unwrap();
    assert_eq!(received.ops.len(), 1);
    assert_eq!(reader.replica.entries.len(), 1);

    let many: Vec<Op> = (0..=MAX_OPS_PER_EVENT as u64)
        .map(|index| op("ma", true, 10 + index))
        .collect();
    let bytes = ledger.event(A, &draft(many), AT, &key(KEY1), [1; NONCE_BYTES]);
    let mut reader = ledger.clone();
    reader.apply(&bytes, Some(&key(KEY1))).unwrap();
    assert_eq!(reader.seq, 1);
    assert!(reader.replica.entries.is_empty());
}

#[test]
fn acknowledgements_and_retirements_travel_inside_events() {
    let mut ledger = genesis();
    let first = ledger.event(A, &draft(vec![]), AT, &key(KEY1), [1; NONCE_BYTES]);
    ledger.apply(&first, Some(&key(KEY1))).unwrap();
    let second = ledger.event(
        B,
        &Draft {
            ops: vec![],
            ack: 1,
            retire: vec![A.to_owned(), B.to_owned()],
        },
        AT + 5,
        &key(KEY1),
        [2; NONCE_BYTES],
    );
    ledger.apply(&second, Some(&key(KEY1))).unwrap();
    assert!(ledger.replica.devices[A].retired);
    // 一台设备不能宣布移除它自己。
    assert!(!ledger.replica.devices[B].retired);
    assert_eq!(ledger.replica.devices[B].ack, 1);
    // 确认位置不会超过写它的那个事件之前。
    let third = ledger.event(
        B,
        &Draft {
            ack: 99,
            ..Draft::default()
        },
        AT + 6,
        &key(KEY1),
        [3; NONCE_BYTES],
    );
    ledger.apply(&third, Some(&key(KEY1))).unwrap();
    assert_eq!(ledger.replica.devices[B].ack, 2);
}

#[test]
fn a_seal_closes_the_epoch_and_the_next_checkpoint_round_trips() {
    let mut ledger = genesis();
    let bytes = ledger.event(
        A,
        &draft(vec![op("ma", true, 10), op("shui'shan", false, 11)]),
        AT,
        &key(KEY1),
        [1; NONCE_BYTES],
    );
    ledger.apply(&bytes, Some(&key(KEY1))).unwrap();
    let seal = ledger.seal(AT + 1);
    ledger.apply(&seal, None).unwrap();
    assert!(ledger.sealed.is_some());
    assert_eq!(
        ledger.apply(&bytes, Some(&key(KEY1))),
        Err(LogError::Sealed)
    );
    let next = ledger.next_epoch(KEY1).unwrap();
    assert_eq!(next.epoch, 1);
    assert_eq!(next.generation, 0);
    // 唯一的设备已经确认到序号 0，删除在序号 1：墓碑还要留着。
    assert_eq!(next.replica.tombstone_count(), 1);
    let one = next.checkpoint(&key(KEY1), [5; NONCE_BYTES]);
    let two = next.checkpoint(&key(KEY1), [6; NONCE_BYTES]);
    assert_ne!(one, two);
    let read = Ledger::from_checkpoint(1, &one, Some(&key(KEY1))).unwrap();
    assert_eq!(read, next);
    assert_eq!(
        Ledger::from_checkpoint(1, &two, Some(&key(KEY1))).unwrap(),
        next
    );
    assert_eq!(
        Ledger::from_checkpoint(2, &one, Some(&key(KEY1))),
        Err(LogError::Malformed)
    );
    assert_eq!(
        Ledger::from_checkpoint(1, &one, None),
        Err(LogError::MissingKey)
    );
    assert_eq!(
        Ledger::from_checkpoint(1, &one, Some(&key(KEY2))),
        Err(LogError::MissingKey)
    );
    let mut tampered: serde_json::Value = serde_json::from_slice(&one).unwrap();
    tampered["generation"] = serde_json::json!(7);
    assert_eq!(
        Ledger::from_checkpoint(1, &serde_json::to_vec(&tampered).unwrap(), Some(&key(KEY1))),
        Err(LogError::Malformed)
    );
    let header = checkpoint_header(&one).unwrap();
    assert_eq!((header.epoch, header.seq), (1, 2));
    assert_eq!(header.head, sha256_hex(&seal));
}

#[test]
fn rekeying_bumps_the_generation_and_a_restart_changes_the_lineage() {
    let mut ledger = genesis();
    let seal = ledger.seal(AT);
    ledger.apply(&seal, None).unwrap();
    let rekeyed = ledger.next_epoch(KEY2).unwrap();
    assert_eq!((rekeyed.generation, rekeyed.key_id.as_str()), (1, KEY2));
    let bytes = rekeyed.checkpoint(&key(KEY2), [1; NONCE_BYTES]);
    assert_eq!(
        Ledger::from_checkpoint(1, &bytes, Some(&key(KEY2))).unwrap(),
        rekeyed
    );
    let restarted = ledger
        .restart("fedcba9876543210fedcba9876543210", KEY2, Replica::default())
        .unwrap();
    assert_eq!(restarted.generation, 1);
    assert_ne!(restarted.lineage, ledger.lineage);
    assert!(genesis().next_epoch(KEY1).is_none());
}

#[test]
fn an_upgrade_seal_is_recognised_and_readers_without_a_key_can_walk_to_a_seal() {
    let mut ledger = genesis();
    let event = ledger.event(
        A,
        &draft(vec![op("ma", true, 10)]),
        AT,
        &key(KEY1),
        [1; NONCE_BYTES],
    );
    let mut blind = genesis();
    skip_without_key(&mut blind, &event).unwrap();
    ledger.apply(&event, Some(&key(KEY1))).unwrap();
    assert_eq!(
        (blind.seq, blind.head.as_str()),
        (ledger.seq, ledger.head.as_str())
    );
    let mut seal: serde_json::Value = serde_json::from_slice(&ledger.seal(AT)).unwrap();
    seal["upgrade"] = serde_json::json!(2);
    let seal = serde_json::to_vec(&seal).unwrap();
    skip_without_key(&mut blind, &seal).unwrap();
    ledger.apply(&seal, Some(&key(KEY1))).unwrap();
    assert_eq!(ledger.sealed, Some(SealInfo { upgrade: Some(2) }));
    assert_eq!(blind.sealed, ledger.sealed);
}

#[test]
fn checkpoints_with_inconsistent_contents_are_refused() {
    let entry = (
        "pinyin".to_owned(),
        "ma".to_owned(),
        "字".to_owned(),
        10u64 << 10,
        0usize,
        1u64,
        true,
    );
    let cases = [
        // 序号超过检查点的位置。
        ReplicaFile {
            writers: vec![A.to_owned()],
            devices: vec![],
            entries: vec![(
                entry.0.clone(),
                entry.1.clone(),
                entry.2.clone(),
                entry.3,
                0,
                9,
                true,
            )],
        },
        // 写入者下标越界。
        ReplicaFile {
            writers: vec![A.to_owned()],
            devices: vec![],
            entries: vec![(
                entry.0.clone(),
                entry.1.clone(),
                entry.2.clone(),
                entry.3,
                1,
                1,
                true,
            )],
        },
        // 不规范的编码。
        ReplicaFile {
            writers: vec![A.to_owned()],
            devices: vec![],
            entries: vec![(
                entry.0.clone(),
                "MA".to_owned(),
                entry.2.clone(),
                entry.3,
                0,
                1,
                true,
            )],
        },
        // 重复的词条。
        ReplicaFile {
            writers: vec![A.to_owned()],
            devices: vec![],
            entries: vec![entry.clone(), entry.clone()],
        },
        // 设备编号不对、确认位置超过检查点。
        ReplicaFile {
            writers: vec![],
            devices: vec![("not-a-device".to_owned(), 0, AT, false)],
            entries: vec![],
        },
        ReplicaFile {
            writers: vec![],
            devices: vec![(A.to_owned(), 9, AT, false)],
            entries: vec![],
        },
    ];
    for case in cases {
        assert!(case.into_replica(2).is_none());
    }
    let good = ReplicaFile {
        writers: vec![A.to_owned()],
        devices: vec![(A.to_owned(), 1, AT, false)],
        entries: vec![entry],
    };
    let replica = good.clone().into_replica(2).unwrap();
    assert_eq!(ReplicaFile::from_replica(&replica), good);
}

#[test]
fn format_negotiation_accepts_only_this_version() {
    assert!(format_supported(&format_bytes()));
    assert!(!format_supported(
        br#"{"format":"msime-dictionary","version":2}"#
    ));
    assert!(!format_supported(
        br#"{"format":"msime-clipboard","version":1}"#
    ));
    assert!(!format_supported(b"garbage"));
}
