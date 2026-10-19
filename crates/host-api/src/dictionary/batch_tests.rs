use crate::tests::{call_android_data, host_options_with_main_dictionary};
use crate::{msime_client_dictionary_collections, msime_client_personal_dictionary_sync};
use msime_client_services::dictionary::personal::{
    PersonalDictionaryStore, PersonalWord, PersonalWordKind,
};
use serde_json::json;

fn word(index: u32) -> PersonalWord {
    // 五万词夹具会跨过 UTF-16 代理区，合成值只使用有效 Unicode 标量。
    let scalar = 0x4e00 + index;
    let scalar = if scalar >= 0xd800 {
        scalar + 0x800
    } else {
        scalar
    };
    PersonalWord {
        kind: PersonalWordKind::Pinyin,
        key: "ni'hao".into(),
        value: format!("{}蒿", char::from_u32(scalar).unwrap()),
        weight: 10_000,
    }
}

fn import_collection(options: &serde_json::Value, count: u32) {
    let text: String = (0..count)
        .map(|index| format!("{}\tni hao\t10000\n", word(index).value))
        .collect();
    let response = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"import","name":"合成作用域词库","kind":"pinyin","format":"rime","text":text}}),
    );
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["import"]["imported"], count);
}

fn synchronize_collection(options: &serde_json::Value) -> serde_json::Value {
    call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"synchronize"}}),
    )
}

#[test]
fn scoped_collection_sync_applies_and_reloads_more_than_one_queue() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    import_collection(&options, 513);
    for attempt in 0..513 {
        let response = synchronize_collection(&options);
        assert_eq!(response["ok"], true, "{response}");
        assert_eq!(response["value"]["failed_count"], 0, "{response}");
        if response["value"]["pending_count"] == 0 {
            break;
        }
        assert!(attempt < 512, "{response}");
    }
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        513
    );
    let reloaded = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"load"}}),
    );
    assert_eq!(reloaded["value"]["outbox_pending"], 0, "{reloaded}");
    assert_eq!(
        synchronize_collection(&options)["value"]["pending_count"],
        0
    );
}

#[test]
fn scoped_collection_sync_busy_preserves_both_durable_queues() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    import_collection(&options, 260);
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    let before = store.read().unwrap().requests;
    let outbox = root.path().join("DictionaryCollections/outbox.json");
    let before_outbox = std::fs::read(&outbox).unwrap();
    let lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &root.path().join("user"),
        &root.path().join("dictionaries"),
    )
    .unwrap()
    .unwrap();
    let response = synchronize_collection(&options);
    assert_eq!(
        response["error"], "dictionary maintenance busy",
        "{response}"
    );
    assert_eq!(store.read().unwrap().requests, before);
    assert_eq!(std::fs::read(outbox).unwrap(), before_outbox);
    drop(lease);
    assert_eq!(synchronize_collection(&options)["ok"], true);
}

#[test]
fn scoped_collection_sync_rejects_corruption_before_applying_personal_words() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    import_collection(&options, 260);
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    let before = store.read().unwrap().requests;
    std::fs::write(
        root.path().join("DictionaryCollections/outbox.json"),
        b"{broken",
    )
    .unwrap();
    let response = synchronize_collection(&options);
    assert_eq!(response["error"], "collections_corrupt", "{response}");
    assert_eq!(store.read().unwrap().requests, before);
}

#[test]
fn scoped_collection_sync_rejects_unknown_action_fields() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let response = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"synchronize","unexpected":true}}),
    );
    assert_eq!(response["error"], "collections_invalid", "{response}");
}

#[test]
fn scoped_collection_sync_rejects_invalid_budgets_before_creating_storage() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    for budget in [
        json!(0),
        json!(2001),
        json!(65536),
        json!("100"),
        json!(null),
    ] {
        let response = call_android_data(
            msime_client_dictionary_collections,
            json!({"options":options,"action":{"operation":"synchronize","budget_millis":budget}}),
        );
        assert_eq!(response["error"], "collections_invalid", "{response}");
        assert!(!root.path().join("DictionaryCollections").exists());
    }
}

#[test]
fn scoped_collection_sync_yields_at_budget_and_counts_unqueued_words() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    import_collection(&options, 260);
    let result = crate::dictionary::synchronize_collections_with_budget(
        &crate::HostOptions::from_document(options.clone()).unwrap(),
        std::time::Duration::ZERO,
    )
    .unwrap();
    assert_eq!(result["pending_count"], 132, "{result}");
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        128
    );
    // 作用域结束后维护租约必须可被新输入会话取得；后续调用重新读盘并完成余下条目。
    let lease = msime_client_services::dictionary::access::DictionaryAccess::try_session(
        &root.path().join("user"),
        &root.path().join("dictionaries"),
    )
    .unwrap()
    .unwrap();
    drop(lease);
    assert_eq!(
        synchronize_collection(&options)["value"]["pending_count"],
        0
    );
}

#[test]
fn scoped_collection_sync_reports_a_rejected_word_and_continues_later_batches() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    prepare_journal(&options);
    let journal = rusqlite::Connection::open(root.path().join("user/msime_user.db")).unwrap();
    journal.execute_batch(&format!("CREATE TRIGGER synthetic_reject BEFORE INSERT ON user_dictionary_operations WHEN NEW.value='{}' BEGIN SELECT RAISE(ABORT,'synthetic rejection'); END;", word(1).value)).unwrap();
    import_collection(&options, 260);
    let response = synchronize_collection(&options);
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["failed_count"], 1, "{response}");
    assert_eq!(response["value"]["pending_count"], 0, "{response}");
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        259
    );
}

#[test]
fn a_full_import_queue_is_applied_in_one_bounded_sync() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import((0..128).map(word).collect(), "collections-batch".into())
        .unwrap();
    let response = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["pending_count"], 0, "{response}");
    assert_eq!(response["value"]["failed_count"], 0, "{response}");
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        128
    );
}

fn engine_options(options: &serde_json::Value) -> msime_engine::host::EngineOptions {
    crate::HostOptions::from_document(options.clone())
        .unwrap()
        .into_engine_options()
}

fn engine_word(word: &PersonalWord) -> msime_engine::host::DictionaryEntry {
    msime_engine::host::DictionaryEntry {
        kind: msime_engine::host::DictionaryKind::Pinyin,
        key: word.key.clone(),
        value: word.value.clone(),
        weight: word.weight,
    }
}

fn prepare_journal(options: &serde_json::Value) {
    msime_engine::host::dictionary_edit(
        &engine_options(options),
        None,
        Some(&engine_word(&word(200))),
        "seed",
    )
    .unwrap();
}

#[test]
fn collection_retry_after_database_commit_keeps_collection_ownership() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import(vec![word(0)], "collections-retry".into())
        .unwrap();
    let request = store.read().unwrap().requests[0].clone();
    // 模拟数据库已提交、进程尚未来得及确认队列时退出。
    msime_engine::host::dictionary_edit(
        &engine_options(&options),
        None,
        Some(&engine_word(&word(0))),
        &request.id,
    )
    .unwrap();
    let response = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(response["ok"], true, "{response}");
    let state = store.read().unwrap();
    assert_eq!(state.pending_count(), 0);
    assert!(
        state.already_present.is_empty(),
        "本次请求自己的回执不能被误认成用户原有词"
    );
}

#[test]
fn a_collection_addition_of_a_shipped_word_keeps_the_shipped_weight() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let dictionary =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    dictionary
        .execute(
            "INSERT INTO tbl_2_n(key,jp,value,weight) VALUES('ni''hao','nh','你好',4321)",
            [],
        )
        .unwrap();
    let shipped = PersonalWord {
        kind: PersonalWordKind::Pinyin,
        key: "ni'hao".into(),
        value: "你好".into(),
        weight: 10_000,
    };
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import(vec![shipped.clone()], "collections-shipped".into())
        .unwrap();

    let response = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["pending_count"], 0, "{response}");
    assert_eq!(response["value"]["failed_count"], 0, "{response}");
    assert_eq!(
        store.read().unwrap().already_present,
        vec![shipped.identity()]
    );
    assert_eq!(
        dictionary
            .query_row(
                "SELECT weight FROM tbl_2_n WHERE key='ni''hao' AND value='你好'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        4321
    );
    let journal = rusqlite::Connection::open(root.path().join("user/msime_user.db")).unwrap();
    assert_eq!(
        journal
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='ni''hao' AND value='你好' AND user_inserted=1",
                [],
                |row| row.get::<_, u32>(0),
            )
            .unwrap(),
        0
    );
}

fn candidate_words(options: &serde_json::Value, key: &str) -> Vec<String> {
    use crate::tests::{read, type_letters};
    use crate::{msime_client_create, msime_client_destroy, msime_client_focus};
    let created = call_android_data(msime_client_create, options.clone());
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    let view = type_letters(handle, key.as_bytes());
    let words = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["text"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    words
}

/// 显式验收只读发布资源的副本，普通单测不依赖本机缓存。
#[test]
#[ignore]
fn shipped_rime_import_retains_reported_common_character_positions() {
    use sha2::{Digest, Sha256};
    let source = std::path::PathBuf::from(
        std::env::var_os("MSIME_RIME_SHIPPED_MAIN").expect("需要已校验的只读拼音资源路径"),
    );
    let lock: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../resources/desktop-dictionary.lock.json"
    ))
    .unwrap();
    let artifact = lock["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["name"] == "msime-pinyin.db")
        .unwrap();
    assert_eq!(
        hex::encode(Sha256::digest(std::fs::read(&source).unwrap())),
        artifact["sha256"].as_str().unwrap()
    );
    let root = tempfile::tempdir().unwrap();
    let mut options = host_options_with_main_dictionary(root.path());
    options["preferences"] = serde_json::to_value(crate::tests::chinese_preferences()).unwrap();
    for name in ["resources", "dictionaries"] {
        std::fs::copy(&source, root.path().join(name).join("msime-pinyin.db")).unwrap();
    }
    let groups = [("a", "啊"), ("mei", "没"), ("ke", "可")];
    let before: Vec<_> = groups
        .iter()
        .map(|(key, value)| {
            let candidates = candidate_words(&options, key);
            candidates
                .iter()
                .position(|candidate| candidate == value)
                .expect("合成查询的常用字应在首屏")
        })
        .collect();
    let text: String = groups
        .iter()
        .enumerate()
        .map(|(index, (key, value))| {
            format!(
                "{value}\t{key}\t0\n{}\t{key}\t7622\n",
                char::from_u32(0x20000 + index as u32).unwrap()
            )
        })
        .collect();
    let imported = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"import","name":"合成发布库排序","kind":"pinyin","format":"rime","text":text}}),
    );
    assert_eq!(imported["ok"], true, "{imported}");
    let id = imported["value"]["collections"][0]["id"].clone();
    let response = synchronize_collection(&options);
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["value"]["pending_count"], 0, "{response}");
    for ((key, value), position) in groups.iter().zip(&before) {
        assert_eq!(
            candidate_words(&options, key)
                .iter()
                .position(|candidate| candidate == value),
            Some(*position)
        );
    }
    let disabled = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"set_enabled","id":id,"enabled":false}}),
    );
    assert_eq!(disabled["ok"], true, "{disabled}");
    assert_eq!(
        synchronize_collection(&options)["value"]["pending_count"],
        0
    );
    for ((key, value), position) in groups.iter().zip(before) {
        assert_eq!(
            candidate_words(&options, key)
                .iter()
                .position(|candidate| candidate == value),
            Some(position)
        );
    }
}

/// 从 Rime 集合导入到实际候选排序，再停用集合，已有常用字不能改权重或被集合取得所有权。
#[test]
fn rime_collection_keeps_shipped_single_character_order_when_disabled() {
    use crate::tests::chinese_preferences;

    let root = tempfile::tempdir().unwrap();
    let mut options = host_options_with_main_dictionary(root.path());
    options["preferences"] = serde_json::to_value(chinese_preferences()).unwrap();
    let groups = [
        ("a", "啊", "阿", "锕"),
        ("mei", "没", "美", "镁"),
        ("ke", "可", "科", "氪"),
    ];
    for directory in ["resources", "dictionaries"] {
        let connection =
            rusqlite::Connection::open(root.path().join(directory).join("msime-pinyin.db"))
                .unwrap();
        for (key, first, second, _) in groups {
            let table = format!("tbl_1_{}", &key[..1]);
            connection.execute_batch(&format!("CREATE TABLE {table}(key TEXT,jp TEXT,value TEXT,weight INTEGER); CREATE INDEX fixture_{table}_key ON {table}(key);")).unwrap();
            for (value, weight) in [(first, 1000), (second, 500)] {
                connection
                    .execute(
                        &format!("INSERT INTO {table} VALUES(?1,?2,?3,?4)"),
                        rusqlite::params![key, &key[..1], value, weight],
                    )
                    .unwrap();
            }
        }
    }
    let candidates = |key: &str| candidate_words(&options, key);
    for (key, first, second, _) in groups {
        assert_eq!(candidates(key), [first, second]);
    }
    // 故意把导入权重排成相反顺序，未覆盖已有字时仍保持合成主库的原顺序。
    let text: String = groups
        .iter()
        .map(|(key, first, second, added)| {
            format!("{first}\t{key}\t0\n{second}\t{key}\t7622\n{added}\t{key}\t400\n")
        })
        .collect();
    let imported = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"import","name":"合成排序回归","kind":"pinyin","format":"rime","text":text}}),
    );
    assert_eq!(imported["ok"], true, "{imported}");
    assert_eq!(imported["value"]["import"]["imported"], 9);
    let id = imported["value"]["collections"][0]["id"].clone();
    let synchronize = || {
        let synced = synchronize_collection(&options);
        assert_eq!(synced["ok"], true, "{synced}");
        assert_eq!(synced["value"]["pending_count"], 0, "{synced}");
        assert_eq!(synced["value"]["failed_count"], 0, "{synced}");
    };
    synchronize();
    for (key, first, second, added) in groups {
        assert_eq!(candidates(key), [first, second, added]);
    }
    let disabled = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"set_enabled","id":id,"enabled":false}}),
    );
    assert_eq!(disabled["ok"], true, "{disabled}");
    synchronize();
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    for (key, first, second, _) in groups {
        assert_eq!(candidates(key), [first, second]);
        for (value, expected) in [(first, 1000), (second, 500)] {
            let weight: i64 = connection
                .query_row(
                    &format!(
                        "SELECT weight FROM tbl_1_{} WHERE key=?1 AND value=?2",
                        &key[..1]
                    ),
                    rusqlite::params![key, value],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(weight, expected);
        }
    }
}

#[test]
fn a_sql_failure_rolls_back_only_its_entry() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    prepare_journal(&options);
    let journal = rusqlite::Connection::open(root.path().join("user/msime_user.db")).unwrap();
    journal.execute_batch(&format!("CREATE TRIGGER synthetic_failure BEFORE INSERT ON user_dictionary_operations WHEN NEW.value='{}' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;", word(1).value)).unwrap();
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import((0..3).map(word).collect(), "collections-partial".into())
        .unwrap();
    let response = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(response["value"]["pending_count"], 0, "{response}");
    assert_eq!(response["value"]["failed_count"], 1, "{response}");
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM tbl_2_n WHERE value=?1",
                [&word(1).value],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        journal
            .query_row(
                "SELECT count(*) FROM personal_dictionary_receipts",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        3
    );
}

#[test]
fn an_outer_sql_rollback_keeps_the_whole_batch_pending() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    prepare_journal(&options);
    let journal = rusqlite::Connection::open(root.path().join("user/msime_user.db")).unwrap();
    journal.execute_batch(&format!("CREATE TRIGGER synthetic_rollback BEFORE INSERT ON user_dictionary_operations WHEN NEW.value='{}' BEGIN SELECT RAISE(ROLLBACK,'synthetic rollback'); END;", word(1).value)).unwrap();
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import((0..3).map(word).collect(), "collections-rollback".into())
        .unwrap();
    let before = store.read().unwrap();
    let response = call_android_data(msime_client_personal_dictionary_sync, options.clone());
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(store.read().unwrap().requests, before.requests);
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        journal
            .query_row(
                "SELECT count(*) FROM personal_dictionary_receipts",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    journal
        .execute_batch("DROP TRIGGER synthetic_rollback")
        .unwrap();
    let retried = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(retried["value"]["pending_count"], 0, "{retried}");
    assert_eq!(retried["value"]["failed_count"], 0, "{retried}");
}

#[test]
fn a_commit_blocked_by_a_reader_keeps_the_whole_batch_pending() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    prepare_journal(&options);
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import((0..3).map(word).collect(), "collections-commit".into())
        .unwrap();
    let before = store.read().unwrap();
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    connection
        .execute_batch("BEGIN; SELECT * FROM tbl_2_n")
        .unwrap();
    let response = call_android_data(msime_client_personal_dictionary_sync, options.clone());
    assert_eq!(response["ok"], false, "{response}");
    assert_eq!(store.read().unwrap().requests, before.requests);
    connection.execute_batch("ROLLBACK").unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        1
    );
    let journal = rusqlite::Connection::open(root.path().join("user/msime_user.db")).unwrap();
    assert_eq!(
        journal
            .query_row(
                "SELECT count(*) FROM personal_dictionary_receipts",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    let retried = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(retried["value"]["pending_count"], 0, "{retried}");
    assert_eq!(retried["value"]["failed_count"], 0, "{retried}");
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        4
    );
    assert_eq!(
        journal
            .query_row(
                "SELECT count(*) FROM personal_dictionary_receipts",
                [],
                |row| row.get::<_, u32>(0)
            )
            .unwrap(),
        4
    );
}

#[test]
fn a_collection_retry_rejects_a_reused_receipt_with_another_payload() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let store = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
    store
        .enqueue_import(vec![word(0)], "collections-reuse".into())
        .unwrap();
    let request = store.read().unwrap().requests[0].clone();
    let mut previous_payload = engine_word(&word(0));
    previous_payload.weight += 1;
    msime_engine::host::dictionary_edit(
        &engine_options(&options),
        None,
        Some(&previous_payload),
        &request.id,
    )
    .unwrap();
    let response = call_android_data(msime_client_personal_dictionary_sync, options);
    assert_eq!(response["value"]["failed_count"], 1, "{response}");
    assert!(store.read().unwrap().already_present.is_empty());
}

#[test]
fn oversized_edit_batches_are_rejected_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let entry = engine_word(&word(0));
    let edits = vec![
        msime_engine::host::DictionaryEdit {
            previous: None,
            replacement: Some(&entry),
            request_id: "oversized",
            preserve_existing: false
        };
        129
    ];
    assert!(msime_engine::host::dictionary_edit_batch(&engine_options(&options), &edits).is_err());
    assert!(!root.path().join("user/msime_user.db").exists());
}

/// 显式运行的合成吞吐测量，普通回归不以本机时间设门槛。
#[test]
#[ignore]
fn synthetic_dictionary_layers_throughput() {
    let count = std::env::var("MSIME_BATCH_BENCH_WORDS")
        .ok()
        .map(|value| value.parse::<u32>().unwrap())
        .unwrap_or(512);
    assert!((1..=50_000).contains(&count));
    for layer in ["engine", "personal"] {
        let root = tempfile::tempdir().unwrap();
        let options = host_options_with_main_dictionary(root.path());
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db"))
            .unwrap()
            .execute_batch("CREATE INDEX fixture_pinyin ON tbl_2_n(key,value)")
            .unwrap();
        let words: Vec<_> = (0..count).map(word).collect();
        let engine_words: Vec<_> = words.iter().map(engine_word).collect();
        let request_ids: Vec<_> = (0..count)
            .map(|index| format!("collections-layer-{index}"))
            .collect();
        let engine = engine_options(&options);
        let personal = PersonalDictionaryStore::new(root.path().join("PersonalDictionary"));
        let started = std::time::Instant::now();
        let mut longest = std::time::Duration::ZERO;
        let mut rounds = 0;
        for offset in (0..words.len()).step_by(128) {
            let end = (offset + 128).min(words.len());
            let batch_started = std::time::Instant::now();
            if layer == "engine" {
                let edits: Vec<_> = (offset..end)
                    .map(|index| msime_engine::host::DictionaryEdit {
                        previous: None,
                        replacement: Some(&engine_words[index]),
                        request_id: &request_ids[index],
                        preserve_existing: true,
                    })
                    .collect();
                let applied = msime_engine::host::dictionary_edit_batch(&engine, &edits).unwrap();
                assert_eq!(applied.len(), end - offset);
                assert!(applied.iter().all(Result::is_ok));
            } else {
                personal
                    .enqueue_import(
                        words[offset..end].to_vec(),
                        format!("collections-layer-batch-{offset}"),
                    )
                    .unwrap();
                let applied =
                    call_android_data(msime_client_personal_dictionary_sync, options.clone());
                assert_eq!(applied["ok"], true, "{applied}");
                assert_eq!(applied["value"]["pending_count"], 0, "{applied}");
                assert_eq!(applied["value"]["failed_count"], 0, "{applied}");
            }
            rounds += 1;
            longest = longest.max(batch_started.elapsed());
        }
        let elapsed = started.elapsed();
        let connection =
            rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            count
        );
        eprintln!(
            "SYNTHETIC_LAYER layer={layer} words={count} rounds={rounds} elapsed_ms={} longest_batch_ms={}",
            elapsed.as_millis(),
            longest.as_millis()
        );
    }
}

/// 显式运行的完整导入测量，包含集合持久化与有界同步。
#[test]
#[ignore]
fn synthetic_rime_import_throughput() {
    let count = std::env::var("MSIME_BATCH_BENCH_WORDS")
        .ok()
        .map(|value| value.parse::<u32>().unwrap())
        .unwrap_or(512);
    assert!(
        (1..=msime_client_services::dictionary::collections::MAX_COLLECTION_ENTRIES as u32)
            .contains(&count),
        "合成测量规模必须落在单个集合的容量内"
    );
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db"))
        .unwrap()
        .execute_batch("CREATE INDEX fixture_pinyin ON tbl_2_n(key,value)")
        .unwrap();
    let text: String = (0..count)
        .map(|index| format!("{}\tni hao\t10000\n", word(index).value))
        .collect();
    let start = std::time::Instant::now();
    let imported = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,
        "action":{"operation":"import","name":"合成词库","kind":"pinyin","format":"rime","text":text}}),
    );
    assert_eq!(imported["ok"], true, "{imported}");
    assert_eq!(imported["value"]["import"]["imported"], count, "{imported}");
    let import_elapsed = start.elapsed();
    let mut rounds = 0;
    let mut longest = std::time::Duration::ZERO;
    let mut sync_elapsed = std::time::Duration::ZERO;
    let mut flush_elapsed = std::time::Duration::ZERO;
    let scoped = std::env::var("MSIME_BATCH_BENCH_SCOPED").as_deref() == Ok("1");
    let idle_budget: u16 = std::env::var("MSIME_BATCH_BENCH_IDLE_MILLIS")
        .ok()
        .map(|value| value.parse().unwrap())
        .unwrap_or(2000);
    assert!((1..=2000).contains(&idle_budget));
    loop {
        let sync_start = std::time::Instant::now();
        let synced = if scoped {
            call_android_data(
                msime_client_dictionary_collections,
                json!({"options":options,"action":{"operation":"synchronize",
                    "budget_millis": if rounds == 0 { 2000 } else { idle_budget }}}),
            )
        } else {
            call_android_data(msime_client_personal_dictionary_sync, options.clone())
        };
        let sync_duration = sync_start.elapsed();
        longest = longest.max(sync_duration);
        sync_elapsed += sync_duration;
        assert_eq!(synced["ok"], true, "{synced}");
        assert_eq!(synced["value"]["failed_count"], 0, "{synced}");
        rounds += 1;
        if scoped {
            if synced["value"]["pending_count"] == 0 {
                break;
            }
            assert!(rounds <= count);
            continue;
        }
        let flush_start = std::time::Instant::now();
        let flushed = call_android_data(
            msime_client_dictionary_collections,
            json!({"options":options,"action":{"operation":"flush"}}),
        );
        flush_elapsed += flush_start.elapsed();
        assert_eq!(flushed["ok"], true, "{flushed}");
        if synced["value"]["pending_count"] == 0 && flushed["value"]["sent"] == 0 {
            break;
        }
        assert!(rounds <= count);
    }
    let elapsed = start.elapsed();
    let connection =
        rusqlite::Connection::open(root.path().join("dictionaries/msime-pinyin.db")).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM tbl_2_n", [], |row| row
                .get::<_, u32>(0))
            .unwrap(),
        count
    );
    eprintln!(
        "SYNTHETIC_IMPORT scoped={scoped} idle_budget_ms={idle_budget} words={count} rounds={rounds} elapsed_ms={} import_ms={} sync_ms={} flush_ms={} longest_sync_ms={}",
        elapsed.as_millis(),
        import_elapsed.as_millis(),
        sync_elapsed.as_millis(),
        flush_elapsed.as_millis(),
        longest.as_millis()
    );
}
