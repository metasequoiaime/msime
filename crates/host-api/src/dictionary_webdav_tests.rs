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

/// 在偏好里配好一个连不上的网盘（本机回环上没人监听的端口）：真的联网就是 `dictionary_sync_unavailable`，测试据此分辨有没有联网。
fn configure_unreachable_drive(preferences_directory: &std::path::Path) {
    let store = PreferencesStore::new(preferences_directory);
    let snapshot = store.load().unwrap();
    let mut preferences = snapshot.preferences;
    preferences.dictionary_sync_webdav =
        msime_client_core::preferences::DictionarySyncWebDavPreferences {
            url: "http://127.0.0.1:9/dav/".into(),
            username: "synthetic@example.test".into(),
            password: "synthetic-app-password".into(),
        };
    store.save(snapshot.revision, preferences).unwrap();
}

fn runtime_options(root: &std::path::Path, options: &Value) -> std::path::PathBuf {
    let path = root.join("runtime-options.json");
    std::fs::write(&path, options.to_string()).unwrap();
    path
}

#[test]
fn the_input_method_never_creates_a_device_identity() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let directory = root.path().join("dictionary-sync");
    let mut host = ImeHost::new(runtime_options(root.path(), &options), directory.clone());
    // 没配置网盘：什么也不做，也不看词库。
    assert!(host.run(Trigger::Periodic).is_none());
    assert!(host.stamp().is_none() && host.fingerprint().is_none());
    // 配好了，但设置应用还没生成设备身份：不建目录、不联网（联网的话是 unavailable）。
    configure_unreachable_drive(root.path());
    for trigger in [Trigger::Periodic, Trigger::LocalChange] {
        let run = host.run(trigger).unwrap().result.unwrap();
        assert!(!run.network);
        assert_eq!(run.report.unwrap_err().code(), "clipboard_identity_missing");
    }
    assert!(!directory.exists());
    assert!(
        host.stamp().is_some(),
        "the journal is watched once configured"
    );

    // C 入口带 create_identity: false 的同样如此；设备管理在锁里检查，不写出密钥串。
    let call = |action: Value| {
        call(
            crate::msime_client_dictionary_sync,
            json!({"options": options, "directory": directory, "create_identity": false, "action": action}),
        )
    };
    let run = call(json!({"operation": "run", "trigger": "manual"}));
    assert_eq!(run["ok"], true, "{run}");
    assert_eq!(run["value"]["network"], false);
    assert_eq!(run["value"]["error"], "clipboard_identity_missing");
    let device = "0123456789abcdef0123456789abcdef";
    for action in [
        json!({"operation": "devices"}),
        json!({"operation": "approve", "device_id": device}),
        json!({"operation": "remove", "device_id": device}),
        json!({"operation": "reset"}),
        json!({"operation": "restart"}),
    ] {
        let answer = call(action.clone());
        assert_eq!(
            answer["error"], "clipboard_identity_missing",
            "{action}: {answer}"
        );
    }
    assert!(!directory.join("cloud-clipboard-relay-keystore").exists());
    let status = call(json!({"operation": "status"}));
    assert_eq!(status["value"]["device_id"], Value::Null, "{status}");
    // 不带这个字段的（设置应用）照常去登记，这里就联网了。
    let settings = self::call(
        crate::msime_client_dictionary_sync,
        json!({"options": options, "directory": directory, "action": {"operation": "run", "trigger": "manual"}}),
    );
    assert_eq!(
        settings["value"]["error"], "dictionary_sync_unavailable",
        "{settings}"
    );
    let wrong = call_with_identity(&options, &directory, json!("no"));
    assert_eq!(wrong["error"], INVALID);
}

fn call_with_identity(
    options: &Value,
    directory: &std::path::Path,
    create_identity: Value,
) -> Value {
    call(
        crate::msime_client_dictionary_sync,
        json!({"options": options, "directory": directory, "create_identity": create_identity, "action": {"operation": "status"}}),
    )
}

#[test]
fn the_journal_stamp_moves_on_every_write_but_the_fingerprint_only_when_words_change() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let mut host = ImeHost::new(
        runtime_options(root.path(), &options),
        root.path().join("dictionary-sync"),
    );
    host.engine = Some(engine(&options));
    let before = host.stamp().unwrap();
    seed(&options, &[("pinyin", "ni'hao", "你好")]);
    let (stamp, words) = (host.stamp().unwrap(), host.fingerprint().unwrap());
    assert_ne!(stamp, before, "the journal appeared");
    // 只改权重（打字学习也是这样改的）：文件变了，词没变。
    let edit = |previous: Value, replacement: Value, id: &str| {
        let edited = call(
            crate::msime_client_dictionary,
            json!({"options": options, "action": {"operation": "edit", "previous": previous, "replacement": replacement, "request_id": id}}),
        );
        assert_eq!(edited["ok"], true, "{edited}");
    };
    edit(
        json!({"kind": "pinyin", "key": "ni'hao", "value": "你好", "weight": 100}),
        json!({"kind": "pinyin", "key": "ni'hao", "value": "你好", "weight": 300}),
        "reweight",
    );
    let reweighted = host.stamp().unwrap();
    assert_ne!(reweighted, stamp);
    assert_eq!(host.fingerprint().unwrap(), words);
    // 加一个词：指纹变了。
    edit(
        Value::Null,
        json!({"kind": "pinyin", "key": "ni'men", "value": "你们", "weight": 100}),
        "add",
    );
    assert_ne!(host.stamp().unwrap(), reweighted);
    assert_ne!(host.fingerprint().unwrap(), words);
}

#[test]
fn the_background_sync_starts_and_stops_through_the_c_entries() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let options_path = runtime_options(root.path(), &options);
    let directory = root.path().join("dictionary-sync");
    let start = |body: Value| call(crate::msime_client_dictionary_sync_start, body);
    assert_eq!(
        start(json!({"options_path": "runtime-options.json", "directory": directory}))["error"],
        INVALID
    );
    assert_eq!(
        start(
            json!({"options_path": options_path, "directory": directory, "create_identity": true})
        )["error"],
        INVALID,
        "the background sync never creates an identity, so there is nothing to choose"
    );
    // SAFETY: 空指针按约定被拒绝。
    let response = unsafe { crate::msime_client_dictionary_sync_start(std::ptr::null(), 0) };
    let text = unsafe { std::ffi::CStr::from_ptr(response) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(response) };
    assert!(text.contains(INVALID));
    // 没配置网盘时起来、再起一次换掉、叫停都很快：后台线程只是在等下一次询问。
    let started = start(json!({"options_path": options_path, "directory": directory}));
    assert_eq!(started["ok"], true, "{started}");
    let again = start(json!({"options_path": options_path, "directory": directory}));
    assert_eq!(again["ok"], true, "{again}");
    let asked = std::time::Instant::now();
    assert!(crate::msime_client_dictionary_sync_stop(2_000));
    assert!(asked.elapsed() < std::time::Duration::from_secs(2));
    assert!(
        crate::msime_client_dictionary_sync_stop(0),
        "nothing is running"
    );
    assert!(!directory.exists());
}
