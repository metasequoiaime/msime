use crate::tests::{call_android_data, host_options_with_main_dictionary};
use crate::{msime_client_dictionary_collections, msime_client_personal_dictionary_sync};
use msime_client_services::dictionary::personal::{
    PersonalDictionaryStore, PersonalWord, PersonalWordKind,
};
use serde_json::json;

fn word(index: u32) -> PersonalWord {
    PersonalWord {
        kind: PersonalWordKind::Pinyin,
        key: "ni'hao".into(),
        value: format!("{}蒿", char::from_u32(0x4e00 + index).unwrap()),
        weight: 10_000,
    }
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
    let mut rounds = 0;
    let mut longest = std::time::Duration::ZERO;
    loop {
        let sync_start = std::time::Instant::now();
        let synced = call_android_data(msime_client_personal_dictionary_sync, options.clone());
        longest = longest.max(sync_start.elapsed());
        assert_eq!(synced["ok"], true, "{synced}");
        assert_eq!(synced["value"]["failed_count"], 0, "{synced}");
        rounds += 1;
        let flushed = call_android_data(
            msime_client_dictionary_collections,
            json!({"options":options,"action":{"operation":"flush"}}),
        );
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
        "SYNTHETIC_IMPORT words={count} rounds={rounds} elapsed_ms={} longest_sync_ms={}",
        elapsed.as_millis(),
        longest.as_millis()
    );
}
