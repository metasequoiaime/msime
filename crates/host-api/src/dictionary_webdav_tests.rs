use super::*;
use crate::tests::host_options_with_main_dictionary;

fn call(function: unsafe extern "C" fn(*const u8, usize) -> *mut c_char, body: Value) -> Value {
    let bytes = body.to_string();
    // SAFETY: 指针和长度来自同一个活着的字符串；返回的响应随即释放。
    let response = unsafe { function(bytes.as_ptr(), bytes.len()) };
    let text = unsafe { std::ffi::CStr::from_ptr(response) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(response) };
    serde_json::from_str(&text).unwrap()
}
use msime_client_core::cloud::dictionary_sync::LocalChange;

fn seed(options: &Value, entries: &[(&str, &str, &str)]) {
    for (index, (kind, key, value)) in entries.iter().enumerate() {
        let edited = call(
            crate::msime_client_dictionary,
            json!({"options": options, "action": {"operation": "edit", "previous": null, "replacement": {"kind": kind, "key": key, "value": value, "weight": 100}, "request_id": format!("seed-{index}")}}),
        );
        assert_eq!(edited["ok"], true, "{edited}");
    }
}

fn engine(options: &Value) -> EngineOptions {
    HostOptions::from_document(options.clone())
        .unwrap()
        .into_engine_options()
}

fn change(key: WordKey, add: bool, id: &str) -> LocalChange {
    LocalChange {
        key,
        add,
        request_id: format!("dsync-{id:0>32}"),
    }
}

#[test]
fn the_engine_dictionary_reads_my_words_and_writes_bounded_batches() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    seed(
        &options,
        &[
            ("pinyin", "ni'hao", "你好"),
            ("quick_phrase", "addr", "第一行\n第二行"),
            ("english", "dont", "don't"),
        ],
    );
    let engine = engine(&options);
    let mut local = EngineDictionary::new(&engine, false);
    let words = local.words().unwrap();
    let nihao = WordKey::new(WordKind::Pinyin, "ni'hao", "你好").unwrap();
    assert!(words.contains(&nihao));
    assert!(words.contains(&WordKey::new(WordKind::Quick, "addr", "第一行\n第二行").unwrap()));
    assert!(words.contains(&WordKey::new(WordKind::English, "dont", "don't").unwrap()));
    assert_eq!(words.len(), 3);
    // 加一个、删一个、删一个本来就没有的。
    let added = WordKey::new(WordKind::Pinyin, "ni'men", "你们").unwrap();
    let absent = WordKey::new(WordKind::Pinyin, "mei'you", "没有").unwrap();
    let outcomes = local
        .apply(&[
            change(added.clone(), true, "1"),
            change(nihao.clone(), false, "2"),
            change(absent, false, "3"),
        ])
        .unwrap();
    assert_eq!(outcomes, vec![LocalOutcome::Applied; 3]);
    local.release();
    let words = local.words().unwrap();
    assert!(words.contains(&added) && !words.contains(&nihao));
    // 同一个请求编号重试不改变结果。
    let outcomes = local.apply(&[change(added.clone(), true, "1")]).unwrap();
    assert_eq!(outcomes, vec![LocalOutcome::Applied]);
    assert_eq!(local.words().unwrap().len(), 3);
}

/// 叫醒输入法的次数：`announce` 是 `fn()`，带不了闭包里的计数。
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
static ANNOUNCED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[test]
fn an_interactive_write_wakes_the_input_method_once_when_it_asks_for_the_dictionary() {
    use std::sync::atomic::Ordering;
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let engine = engine(&options);
    let session = DictionaryAccess::try_session(
        std::path::Path::new(&engine.user_data),
        std::path::Path::new(&engine.dictionaries),
    )
    .unwrap()
    .unwrap();
    let mut waiting = EngineDictionary::announcing(&engine, true, || {
        ANNOUNCED.fetch_add(1, Ordering::SeqCst);
    });
    let word = WordKey::new(WordKind::Pinyin, "ni'men", "你们").unwrap();
    // 输入会话一直不放，让出请求挂上、叫醒一次，预算用完后答「忙」；同一轮里再写一批不再叫醒。
    assert_eq!(
        waiting.apply(&[change(word.clone(), true, "1")]),
        Err(LocalError::Busy)
    );
    assert_eq!(ANNOUNCED.load(Ordering::SeqCst), 1);
    assert_eq!(
        waiting.apply(&[change(word.clone(), true, "1")]),
        Err(LocalError::Busy)
    );
    assert_eq!(ANNOUNCED.load(Ordering::SeqCst), 1);
    waiting.release();
    drop(session);
    // 输入会话放开之后照常写进去，没有再叫醒。
    let mut local = EngineDictionary::announcing(&engine, true, || {
        ANNOUNCED.fetch_add(1, Ordering::SeqCst);
    });
    assert_eq!(
        local.apply(&[change(word, true, "1")]).unwrap(),
        vec![LocalOutcome::Applied]
    );
    assert_eq!(ANNOUNCED.load(Ordering::SeqCst), 1);
}

#[test]
fn an_active_input_session_makes_automatic_writes_wait() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let engine = engine(&options);
    let session = DictionaryAccess::try_session(
        std::path::Path::new(&engine.user_data),
        std::path::Path::new(&engine.dictionaries),
    )
    .unwrap()
    .unwrap();
    let mut local = EngineDictionary::new(&engine, false);
    let word = WordKey::new(WordKind::Pinyin, "ni'men", "你们").unwrap();
    assert_eq!(
        local.apply(&[change(word.clone(), true, "1")]),
        Err(LocalError::Busy)
    );
    // 读不需要维护权。
    assert!(local.words().unwrap().is_empty());
    drop(session);
    assert_eq!(
        local.apply(&[change(word, true, "1")]).unwrap(),
        vec![LocalOutcome::Applied]
    );
}

#[test]
fn a_word_the_engine_refuses_is_reported_and_others_in_the_batch_still_land() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let engine = engine(&options);
    let mut local = EngineDictionary::new(&engine, false);
    // 音节表里没有的拼音：同步的规则收，Engine 不收。夹具的主词库只有两字、三字词的 n、a 开头那两张表。
    let odd = WordKey::new(WordKind::Pinyin, "xq", "怪").unwrap();
    let good = WordKey::new(WordKind::Pinyin, "nan'ren", "男人").unwrap();
    let outcomes = local
        .apply(&[change(odd, true, "1"), change(good.clone(), true, "2")])
        .unwrap();
    assert_eq!(
        outcomes,
        vec![LocalOutcome::Rejected, LocalOutcome::Applied]
    );
    assert_eq!(local.words().unwrap(), BTreeSet::from([good]));
}

#[test]
fn requests_are_validated_before_anything_touches_the_network() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let directory = root.path().join("dictionary-sync");
    let call = |body: Value| call(crate::msime_client_dictionary_sync, body);
    assert_eq!(
        call(
            json!({"options": options, "directory": "relative", "action": {"operation": "status"}})
        )["error"],
        INVALID
    );
    assert_eq!(
        call(json!({"options": options, "directory": directory, "action": {"operation": "dance"}}))
            ["error"],
        INVALID
    );
    assert_eq!(
        call(
            json!({"options": options, "directory": directory, "action": {"operation": "run", "trigger": "sometimes"}})
        )["error"],
        INVALID
    );
    // 偏好里没有配置词库同步。
    assert_eq!(
        call(
            json!({"options": options, "directory": directory, "action": {"operation": "status"}})
        )["error"],
        "dictionary_sync_not_configured"
    );
    // SAFETY: 空指针按约定被拒绝。
    let response = unsafe { crate::msime_client_dictionary_sync(std::ptr::null(), 0) };
    let text = unsafe { std::ffi::CStr::from_ptr(response) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(response) };
    assert!(text.contains(INVALID));
}
