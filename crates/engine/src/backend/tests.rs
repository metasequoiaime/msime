use std::path::Path;

use rusqlite::Connection;
use serde_json::{json, Value};

use super::{execute, execute_raw, MAXIMUM_REQUEST_BYTES};
use crate::assets;

/// A resource directory with a small pinyin and English dictionary in the shipped layout.
fn resources() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let pinyin = Connection::open(directory.path().join(assets::MAIN_DICTIONARY)).unwrap();
    for table in ["tbl_1_z", "tbl_2_z", "tbl_2_c", "tbl_3_z", "tbl_others_c"] {
        pinyin
            .execute_batch(&format!(
                "CREATE TABLE {table}(key TEXT, jp TEXT, value TEXT, weight INTEGER DEFAULT 0);"
            ))
            .unwrap();
    }
    pinyin
        .execute_batch(
            "INSERT INTO tbl_1_z VALUES('zhong','z','中',9),('zhou','z','周',5),('zhe','z','这',1);
             INSERT INTO tbl_2_z VALUES('zhou''shen','zs','周深',2),('zhong''guo','zg','中国',40);
             INSERT INTO tbl_2_c VALUES('chen''yao','cy','陈瑶',2),('ce''shi','cs','测试',30),('ce''si','cs','厕所',7),('ce''shi','cs','侧视',6),('chang''cheng','cc','长城',11);
             INSERT INTO tbl_3_z VALUES('zhong''hua''min','zhm','中华民',3);
             CREATE TABLE quick_parases(key TEXT, value TEXT, weight INTEGER);
             INSERT INTO quick_parases VALUES('email','me@example.com',5),('emoji','😀',9),('addr','地址',1);
             INSERT INTO tbl_others_c VALUES('ce''ce''ce''ce''ce''ce''ce''ce','cccccccc','测测测测测测测测',4);",
        )
        .unwrap();
    let english = Connection::open(directory.path().join(assets::ENGLISH_DICTIONARY)).unwrap();
    english
        .execute_batch(
            "CREATE TABLE english_words(word TEXT, display TEXT, weight INTEGER, PRIMARY KEY(word, display)) WITHOUT ROWID;
             INSERT INTO english_words VALUES('hello','hello',100),('hello','Hello',50),('help','help',80),('helmet','helmet',90);
             CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY, chinese_gloss TEXT NOT NULL) WITHOUT ROWID;
             CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY, english_gloss TEXT NOT NULL) WITHOUT ROWID;
             INSERT INTO en_zh_glosses VALUES('hello','喂');
             INSERT INTO zh_en_glosses VALUES('苹果','apple');",
        )
        .unwrap();
    let others = Connection::open(directory.path().join(assets::OTHER_DICTIONARY)).unwrap();
    others
        .execute_batch(
            "CREATE TABLE emoji_pinyin(key TEXT, emoji TEXT, sort_order INTEGER);
             INSERT INTO emoji_pinyin VALUES('kaixin','😄',1),('kaixin','😊',2),('kaixinguo','🌰',3);
             CREATE TABLE kaomoji(pinyin TEXT, jianpin TEXT, kaomoji TEXT, sort_order INTEGER);
             INSERT INTO kaomoji VALUES('kaixin','kx','╰(*´︶`*)╯',1);",
        )
        .unwrap();
    directory
}

fn run(request: Value, resources: &Path) -> Value {
    execute(
        &request,
        resources,
        Path::new("/nonexistent-scratch"),
        str::to_owned,
    )
}

#[test]
fn listed_pinyin_batch_matches_exact_code_and_word() {
    let resources = resources();
    let response = run(
        json!({"operation": "listed_pinyin_batch", "entries": [
            {"code": "zhou'shen", "word": "周深"},
            {"code": "chen'yao", "word": "陈瑶"},
            {"code": "ce'si", "word": "测试"},
            {"code": "ce'shi", "word": "测试"},
            {"code": "ce'ce'ce'ce'ce'ce'ce'ce", "word": "测测测测测测测测"},
            {"code": "xu'an", "word": "宣璐"},
        ]}),
        resources.path(),
    );
    assert_eq!(
        response,
        json!({"listed": [true, true, false, true, true, false]})
    );
}

#[test]
fn listed_pinyin_batch_rejects_bad_batches() {
    let resources = resources();
    for entries in [
        json!([]),
        json!([{"code": "ce'shi", "word": ""}]),
        json!([{"code": "", "word": "测试"}]),
        json!([{"code": "ce'shi"}]),
        Value::Array(vec![json!({"code": "ce'shi", "word": "测试"}); 51]),
    ] {
        let response = run(
            json!({"operation": "listed_pinyin_batch", "entries": entries}),
            resources.path(),
        );
        assert_eq!(response, json!({"error": "invalid_request"}), "{entries}");
    }
}

#[test]
fn listed_english_batch_is_case_sensitive_on_display() {
    let resources = resources();
    let response = run(
        json!({"operation": "listed_english_batch", "entries": [
            {"word": "hello", "display": "hello"},
            {"word": "hello", "display": "HELLO"},
            {"word": "zzzz", "display": "zzzz"},
        ]}),
        resources.path(),
    );
    assert_eq!(response, json!({"listed": [true, false, false]}));
}

#[test]
fn checks_report_missing_dictionaries() {
    let empty = tempfile::tempdir().unwrap();
    for request in [
        json!({"operation": "listed_pinyin_batch", "entries": [{"code": "ce'shi", "word": "测试"}]}),
        json!({"operation": "listed_english_batch", "entries": [{"word": "a", "display": "a"}]}),
        json!({"operation": "pinyin_weight_medians"}),
    ] {
        assert_eq!(
            run(request, empty.path()),
            json!({"error": "resources_unavailable"})
        );
    }
}

#[test]
fn weight_medians_take_the_lower_middle_per_syllable_count() {
    let resources = resources();
    let response = run(
        json!({"operation": "pinyin_weight_medians"}),
        resources.path(),
    );
    // One syllable: 1, 5, 9 → 5. Two: 2, 2, 7, 11, 30, 40 → lower middle 7. Three: 3. Eight or more: 4.
    assert_eq!(
        response,
        json!({"medians": {"1": 5, "2": 7, "3": 3, "8": 4}})
    );
}

#[test]
fn legacy_file_names_are_still_read() {
    let resources = resources();
    std::fs::rename(
        resources.path().join(assets::MAIN_DICTIONARY),
        resources.path().join("msime.db"),
    )
    .unwrap();
    let response = run(
        json!({"operation": "listed_pinyin_batch", "entries": [{"code": "zhou'shen", "word": "周深"}]}),
        resources.path(),
    );
    assert_eq!(response, json!({"listed": [true]}));
}

#[test]
fn validate_dictionary_normalises_and_reports_invalid_entries() {
    let resources = resources();
    let response = run(
        json!({"operation": "validate_dictionary", "kind": "pinyin", "code": "ni hao", "text": "你好"}),
        resources.path(),
    );
    assert_eq!(
        response,
        json!({"kind": "pinyin", "code": "ni'hao", "word": "你好", "weight": 100000})
    );
    let response = run(
        json!({"operation": "validate_dictionary", "kind": "pinyin", "code": "xyzzy", "text": "你好"}),
        resources.path(),
    );
    assert_eq!(response, json!({"error": "invalid_dictionary_entry"}));
    // 服务端也存 98 版五笔词条，按 `wubi98` 原名返回；不认识的种类仍是无效请求。
    let response = run(
        json!({"operation": "validate_dictionary", "kind": "wubi98", "code": "wq", "text": "你"}),
        resources.path(),
    );
    assert_eq!(
        response,
        json!({"kind": "wubi98", "code": "wq", "word": "你", "weight": 100000})
    );
    let response = run(
        json!({"operation": "validate_dictionary", "kind": "wubi98", "code": "abcde", "text": "你"}),
        resources.path(),
    );
    assert_eq!(response, json!({"error": "invalid_dictionary_entry"}));
    let response = run(
        json!({"operation": "validate_dictionary", "kind": "wubi86", "code": "wq", "text": "你"}),
        resources.path(),
    );
    assert_eq!(response, json!({"error": "invalid_request"}));
}

#[test]
fn validate_dictionary_batch_stops_at_the_first_failure() {
    let resources = resources();
    let response = run(
        json!({"operation": "validate_dictionary_batch", "entries": [
            {"kind": "pinyin", "code": "ce'shi", "text": "测试", "weight": 10},
            {"kind": "english", "code": "hello", "text": "hello"},
        ]}),
        resources.path(),
    );
    assert_eq!(
        response,
        json!({"entries": [
            {"kind": "pinyin", "code": "ce'shi", "word": "测试", "weight": 10},
            {"kind": "english", "code": "hello", "word": "hello", "weight": 100000},
        ]})
    );
    let response = run(
        json!({"operation": "validate_dictionary_batch", "entries": [
            {"kind": "pinyin", "code": "ce'shi", "text": "测试"},
            {"kind": "pinyin", "code": "", "text": "测试"},
        ]}),
        resources.path(),
    );
    assert_eq!(response, json!({"error": "invalid_dictionary_entry"}));
}

#[test]
fn request_framing_rejects_oversized_or_malformed_input() {
    let resources = resources();
    let scratch = Path::new("/nonexistent-scratch");
    assert_eq!(
        execute_raw(
            &vec![b' '; MAXIMUM_REQUEST_BYTES + 1],
            resources.path(),
            scratch,
            str::to_owned
        ),
        json!({"error": "invalid_request"})
    );
    for raw in [&b"not json"[..], b"[]", b"{}", br#"{"operation": 3}"#] {
        assert_eq!(
            execute_raw(raw, resources.path(), scratch, str::to_owned),
            json!({"error": "invalid_request"})
        );
    }
    for request in [
        json!({"operation": "pinyin_weight_medians", "limit": 0}),
        json!({"operation": "pinyin_weight_medians", "limit": 201}),
        json!({"operation": "pinyin_weight_medians", "limit": "5"}),
        json!({"operation": "pinyin_weight_medians", "text": "x".repeat(8193)}),
    ] {
        assert_eq!(
            run(request, resources.path()),
            json!({"error": "invalid_request"})
        );
    }
    assert_eq!(
        run(json!({"operation": "no_such_operation"}), resources.path()),
        json!({"error": "unknown_operation"})
    );
}

#[test]
fn unicode_and_romaji_need_no_resources() {
    let empty = Path::new("");
    assert_eq!(
        run(json!({"operation": "unicode", "text": "4E2D"}), empty)["candidates"][0]["word"],
        json!("中")
    );
    assert_eq!(
        run(json!({"operation": "unicode", "text": "D800"}), empty),
        json!({"candidates": []})
    );
    assert_eq!(
        run(json!({"operation": "romaji", "text": "konnnichiha"}), empty),
        json!({"text": "こんにちは", "pending": "", "complete": true})
    );
    assert_eq!(
        run(json!({"operation": "romaji", "text": "ky"}), empty),
        json!({"text": "", "pending": "ky", "complete": false})
    );
    assert_eq!(
        run(
            json!({"operation": "romaji", "text": "ひらがな", "direction": "hiragana-katakana"}),
            empty
        ),
        json!({"text": "ヒラガナ"})
    );
    assert_eq!(
        run(
            json!({"operation": "romaji", "text": "かな", "direction": "kana-romaji"}),
            empty
        ),
        json!({"text": "kana"})
    );
    assert_eq!(
        run(
            json!({"operation": "romaji", "text": "a", "direction": "sideways"}),
            empty
        ),
        json!({"error": "invalid_request"})
    );
}

#[test]
fn date_time_validates_the_supplied_date() {
    let date = json!({"year": 2026, "month": 9, "day": 9, "weekday": 3, "hour": 8, "minute": 5, "second": 0});
    let response = run(
        json!({"operation": "datetime", "text": "rq", "date": date, "limit": 3}),
        Path::new(""),
    );
    let words: Vec<_> = response["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["word"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(words, ["2026年9月9日", "2026-09-09", "2026/09/09"]);
    // 农历关键词也在服务端白名单里。
    let summer = json!({"year": 2026, "month": 8, "day": 9, "weekday": 0, "hour": 8, "minute": 5, "second": 0});
    for text in ["nl", "nongli", "yinli"] {
        let response = run(
            json!({"operation": "datetime", "text": text, "date": summer, "limit": 2}),
            Path::new(""),
        );
        let words: Vec<_> = response["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["word"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            words,
            ["丙午年六月二十七日", "丙午年六月二十七日 星期日"],
            "{text}"
        );
    }
    for (text, date) in [
        ("hello", date.clone()),
        (
            "rq",
            json!({"year": 2026, "month": 13, "day": 9, "weekday": 3, "hour": 8, "minute": 5, "second": 0}),
        ),
        (
            "rq",
            json!({"year": 2026, "month": 9, "day": 9, "weekday": 7, "hour": 8, "minute": 5, "second": 0}),
        ),
        (
            "rq",
            json!({"year": 2026, "month": 2, "day": 29, "weekday": 0, "hour": 8, "minute": 5, "second": 0}),
        ),
        (
            "rq",
            json!({"year": 2026, "month": 4, "day": 31, "weekday": 5, "hour": 8, "minute": 5, "second": 0}),
        ),
        ("rq", json!({"year": 2026, "month": 9})),
    ] {
        assert_eq!(
            run(
                json!({"operation": "datetime", "text": text, "date": date}),
                Path::new("")
            ),
            json!({"error": "invalid_request"})
        );
    }
}

fn words(response: &Value) -> Vec<String> {
    response["candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("no candidates in {response}"))
        .iter()
        .map(|item| item["word"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn local_modes_read_the_shipped_catalogs() {
    let resources = resources();
    let path = resources.path();
    // A prefix query: kaixinguo starts with kaixin.
    assert_eq!(
        words(&run(json!({"operation": "emoji", "text": "kaixin"}), path)),
        ["😄", "😊", "🌰"]
    );
    assert_eq!(
        words(&run(
            json!({"operation": "kaomoji", "text": "kaixin"}),
            path
        )),
        ["╰(*´︶`*)╯"]
    );
    assert_eq!(
        words(&run(json!({"operation": "jianpin", "text": "zg"}), path)),
        ["中国"]
    );
    assert_eq!(
        words(&run(
            json!({"operation": "quick", "text": "em", "limit": 1}),
            path
        )),
        ["😀"]
    );
    assert_eq!(
        run(
            json!({"operation": "emoji", "text": "kaixin", "scheme": "cangjie"}),
            path
        ),
        json!({"error": "invalid_request"})
    );
    let empty = tempfile::tempdir().unwrap();
    for operation in ["emoji", "kaomoji", "jianpin", "quick", "english", "gloss"] {
        assert_eq!(
            run(json!({"operation": operation, "text": "a"}), empty.path()),
            json!({"error": "resources_unavailable"}),
            "{operation}"
        );
    }
}

#[test]
fn empty_catalog_page_does_not_reserve_result_page() {
    let resources = resources();
    let connection = Connection::open(resources.path().join(assets::OTHER_DICTIONARY)).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE emoji(emoji TEXT,category TEXT,keywords TEXT,sort_order INTEGER);
             INSERT INTO emoji VALUES('😀','faces','smile',1);",
        )
        .unwrap();
    let empty_request = json!({
        "operation": "catalog",
        "kind": "emoji",
        "text": "missing",
        "limit": 200,
    });
    let matching_request = json!({
        "operation": "catalog",
        "kind": "emoji",
        "text": "smile",
        "limit": 200,
    });
    let (empty_response, empty_allocations) =
        crate::ime::personal_rerank::allocations::count(|| {
            run(empty_request.clone(), resources.path())
        });
    let (matching_response, matching_allocations) =
        crate::ime::personal_rerank::allocations::count(|| run(matching_request, resources.path()));
    assert_eq!(empty_response["items"], json!([]));
    assert!(!matching_response["items"].as_array().unwrap().is_empty());
    assert!(
        empty_allocations < matching_allocations,
        "空 catalog 页不应申请结果缓冲：空页 {empty_allocations}，非空页 {matching_allocations}"
    );
}

#[test]
fn empty_dictionary_page_does_not_reserve_result_page() {
    let resources = resources();
    let empty_request = json!({
        "operation": "dictionary",
        "kind": "quick",
        "text": "zzz",
        "limit": 200,
    });
    let matching_request = json!({
        "operation": "dictionary",
        "kind": "quick",
        "text": "em",
        "limit": 200,
    });
    let (empty_response, empty_allocations) =
        crate::ime::personal_rerank::allocations::count(|| {
            run(empty_request.clone(), resources.path())
        });
    let (matching_response, matching_allocations) =
        crate::ime::personal_rerank::allocations::count(|| run(matching_request, resources.path()));
    assert_eq!(empty_response["entries"], json!([]));
    assert!(!matching_response["entries"].as_array().unwrap().is_empty());
    assert!(
        empty_allocations < matching_allocations,
        "空 dictionary 页不应申请结果缓冲：空页 {empty_allocations}，非空页 {matching_allocations}"
    );
}

#[test]
fn english_completes_prefixes_and_glosses_both_ways() {
    let resources = resources();
    let path = resources.path();
    assert_eq!(
        words(&run(json!({"operation": "english", "text": "hel"}), path)),
        ["hello", "helmet", "help", "Hello"]
    );
    assert_eq!(
        words(&run(
            json!({"operation": "english", "text": "hello", "limit": 2}),
            path
        )),
        ["hello", "Hello"]
    );
    assert_eq!(
        run(json!({"operation": "gloss", "text": "hello"}), path),
        json!({"text": "喂"})
    );
    assert_eq!(
        run(
            json!({"operation": "gloss", "text": "苹果", "direction": "zh-en"}),
            path
        ),
        json!({"text": "apple"})
    );
    assert_eq!(
        run(json!({"operation": "gloss", "text": "missing"}), path),
        json!({"text": ""})
    );
    assert_eq!(
        run(
            json!({"operation": "gloss", "text": "hello", "direction": "en-fr"}),
            path
        ),
        json!({"error": "invalid_request"})
    );
}

#[test]
fn english_completion_keeps_a_prefix_ending_at_the_maximum_scalar() {
    let resources = resources();
    let connection = Connection::open(resources.path().join(assets::ENGLISH_DICTIONARY)).unwrap();
    let maximum = "\u{10ffff}";
    let suffix = format!("{maximum}suffix");
    connection
        .execute("INSERT INTO english_words VALUES(?1, ?1, 1)", [maximum])
        .unwrap();
    connection
        .execute("INSERT INTO english_words VALUES(?1, ?1, 2)", [&suffix])
        .unwrap();

    assert_eq!(
        run(
            json!({"operation": "dictionary", "kind": "english", "text": maximum}),
            resources.path()
        )["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![maximum, suffix.as_str()]
    );
}

/// Answer a personal request with `snapshot` as the server's snapshot file.
fn personal(request: Value, snapshot: &str, resources: &Path) -> Value {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(scratch.path().join("snapshot.jsonl"), snapshot).unwrap();
    execute(&request, resources, scratch.path(), str::to_owned)
}

fn ranked(response: &Value) -> Vec<(String, i64, i64)> {
    response["candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("no candidates in {response}"))
        .iter()
        .map(|item| {
            (
                item["word"].as_str().unwrap().to_owned(),
                item["weight"].as_i64().unwrap(),
                item["fixed_position"].as_i64().unwrap(),
            )
        })
        .collect()
}

const CESHI: &str =
    r#"{"operation":"personal_query","query":{"operation":"candidates","text":"ceshi","limit":3}}"#;

#[test]
fn personal_query_replays_the_overlay_and_reports_the_revision() {
    let resources = resources();
    let request: Value = serde_json::from_str(CESHI).unwrap();
    let plain = personal(
        request.clone(),
        "{\"snapshot_revision\":7}\n",
        resources.path(),
    );
    assert_eq!(plain["revision"], json!(7));
    assert_eq!(plain["context"], json!("ce'shi"));
    assert_eq!(ranked(&plain)[0].0, "测试");
    let added = personal(
        request.clone(),
        concat!(
            "{\"snapshot_revision\":8}\n",
            "{\"previous\":null,\"replacement\":{\"kind\":\"pinyin\",\"code\":\"ce'shi\",\"word\":\"侧室\",\"weight\":999}}\n",
            "{\"previous\":{\"kind\":\"pinyin\",\"code\":\"ce'shi\",\"word\":\"测试\",\"weight\":30},\"replacement\":null}\n",
        ),
        resources.path(),
    );
    let words: Vec<_> = ranked(&added).into_iter().map(|row| row.0).collect();
    assert!(words.contains(&"侧室".to_owned()), "{words:?}");
    assert!(!words.contains(&"测试".to_owned()), "{words:?}");
    assert_eq!(added["revision"], json!(8));
}

#[test]
fn personal_query_applies_fixed_positions() {
    let resources = resources();
    let response = personal(
        serde_json::from_str(CESHI).unwrap(),
        "{\"fixed\":{\"context\":\"ce'shi\",\"code\":\"ce'shi\",\"word\":\"侧视\",\"position\":1}}\n",
        resources.path(),
    );
    assert_eq!(ranked(&response)[0], ("侧视".to_owned(), 6, 1));
}

#[test]
fn personal_rank_reports_changed_weights_and_the_counter() {
    let resources = resources();
    let pick = |mode: &str, trigger: i64, snapshot: &str| {
        personal(
            json!({"operation": "personal_rank",
                   "query": {"operation": "candidates", "text": "ceshi"},
                   "action": {"code": "ce'shi", "word": "侧视", "mode": mode, "trigger_count": trigger}}),
            snapshot,
            resources.path(),
        )
    };
    let pinned = pick("pin", 1, "{\"snapshot_revision\":3}\n");
    assert_eq!(pinned["changed"], json!(true), "{pinned}");
    assert_eq!(pinned["revision"], json!(3));
    let update = &pinned["updates"][0];
    assert_eq!(update["word"], json!("侧视"));
    assert!(update["weight"].as_i64().unwrap() > 30, "{update}");
    assert_eq!(update["user_inserted"], json!(false));
    // Below the trigger count only the counter moves.
    let counted = pick(
        "promote",
        3,
        "{\"selection\":{\"context\":\"ce'shi\",\"code\":\"ce'shi\",\"word\":\"侧视\",\"count\":1}}\n",
    );
    assert_eq!(counted["changed"], json!(false));
    assert_eq!(counted["updates"], json!([]));
    assert_eq!(counted["selection"]["count"], json!(2));
    let unknown = pick("sideways", 1, "{\"snapshot_revision\":1}\n");
    assert_eq!(unknown, json!({"error": "invalid_request"}));
}

#[test]
fn personal_delete_refuses_single_characters_and_reports_ownership() {
    let resources = resources();
    let delete = |text: &str, code: &str, word: &str, snapshot: &str| {
        personal(
            json!({"operation": "personal_delete",
                   "query": {"operation": "candidates", "text": text},
                   "action": {"code": code, "word": word}}),
            snapshot,
            resources.path(),
        )
    };
    assert_eq!(
        delete("ceshi", "ce'shi", "侧视", "{\"snapshot_revision\":2}\n"),
        json!({"deleted": {"kind": "pinyin", "code": "ce'shi", "word": "侧视", "weight": 6, "user_inserted": false},
               "changed": true, "revision": 2})
    );
    let owned = delete(
        "ceshi",
        "ce'shi",
        "侧室",
        "{\"previous\":null,\"replacement\":{\"kind\":\"pinyin\",\"code\":\"ce'shi\",\"word\":\"侧室\",\"weight\":5}}\n",
    );
    assert_eq!(owned["deleted"]["user_inserted"], json!(true), "{owned}");
    assert_eq!(
        delete("zhong", "zhong", "中", "{\"snapshot_revision\":1}\n"),
        json!({"error": "invalid_request"})
    );
}

#[test]
fn personal_english_keeps_weights_above_the_import_ceiling() {
    let resources = resources();
    // English frequency learning never had a ceiling, so stored overlays can exceed the engine's import bound.
    let response = personal(
        json!({"operation": "personal_query", "query": {"operation": "english", "text": "hel", "limit": 2}}),
        "{\"previous\":null,\"replacement\":{\"kind\":\"english\",\"code\":\"helium\",\"word\":\"helium\",\"weight\":23000000000}}\n",
        resources.path(),
    );
    assert_eq!(
        ranked(&response)[0],
        ("helium".to_owned(), 23_000_000_000, 0)
    );
    assert_eq!(response["context"], json!("english:hel"));
}

#[test]
fn personal_requests_need_a_scratch_directory_and_a_readable_snapshot() {
    let resources = resources();
    let request: Value = serde_json::from_str(CESHI).unwrap();
    assert_eq!(
        execute(&request, resources.path(), Path::new(""), str::to_owned),
        json!({"error": "resources_unavailable"})
    );
    let scratch = tempfile::tempdir().unwrap();
    assert_eq!(
        execute(&request, resources.path(), scratch.path(), str::to_owned),
        json!({"error": "engine_failure"})
    );
    assert_eq!(
        personal(request.clone(), "not json\n", resources.path()),
        json!({"error": "invalid_request"})
    );
    assert_eq!(
        personal(
            json!({"operation": "personal_query", "query": {"operation": "emoji", "text": "a"}}),
            "{\"snapshot_revision\":1}\n",
            resources.path()
        ),
        json!({"error": "invalid_request"})
    );
}

#[test]
fn validate_snapshot_checks_exported_entries() {
    let resources = resources();
    let request = json!({"operation": "validate_snapshot"});
    let valid = concat!(
        "{\"type\":\"header\",\"data\":{}}\n",
        "{\"type\":\"entry\",\"data\":{\"kind\":\"pinyin\",\"code\":\"ni'hao\",\"word\":\"你好\",\"weight\":5}}\n",
        "{\"type\":\"overlay\",\"deleted\":true,\"data\":{\"kind\":\"english\",\"code\":\"hello\",\"word\":\"hello\",\"weight\":0}}\n",
    );
    assert_eq!(
        personal(request.clone(), valid, resources.path()),
        json!({"valid": true})
    );
    // A code the engine would normalise was not stored normalised.
    let unnormalised = "{\"type\":\"entry\",\"data\":{\"kind\":\"pinyin\",\"code\":\"ni hao\",\"word\":\"你好\",\"weight\":5}}\n";
    assert_eq!(
        personal(request, unnormalised, resources.path()),
        json!({"error": "invalid_request"})
    );
}

#[cfg(unix)]
#[test]
fn personal_snapshot_does_not_follow_a_symlinked_leaf() {
    use std::os::unix::fs::symlink;

    let resources = resources();
    let scratch = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().join("snapshot.jsonl");
    std::fs::write(&target, b"{\"type\":\"header\",\"data\":{}}\n").unwrap();
    symlink(&target, scratch.path().join("snapshot.jsonl")).unwrap();

    assert_eq!(
        execute(
            &json!({"operation": "validate_snapshot"}),
            resources.path(),
            scratch.path(),
            str::to_owned,
        ),
        json!({"error": "engine_failure"})
    );
}

/// 在 `resources()` 的主词库里加上 86 和 98 两版五笔码表：同一个编码 `kg` 在两版里是不同的字，读错表就能看出来。
fn wubi_resources() -> tempfile::TempDir {
    let directory = resources();
    Connection::open(directory.path().join(assets::MAIN_DICTIONARY))
        .unwrap()
        .execute_batch(
            "CREATE TABLE wubi86(key TEXT, value TEXT, weight INTEGER, UNIQUE(key, value));
             CREATE TABLE wubi98(key TEXT, value TEXT, weight INTEGER, UNIQUE(key, value));
             INSERT INTO wubi86 VALUES('kg','甲',300),('kg','甲乙',100);
             INSERT INTO wubi98 VALUES('kg','乙',300),('kg','丙丁',100);",
        )
        .unwrap();
    directory
}

#[test]
fn wubi98_personal_query_reads_the_wubi98_table_and_its_own_entries() {
    let resources = wubi_resources();
    let query = |scheme: &str, snapshot: &str| {
        personal(
            json!({"operation": "personal_query", "query": {"operation": "candidates", "scheme": scheme, "text": "kg", "limit": 5}}),
            snapshot,
            resources.path(),
        )
    };
    let words = |response: &Value| -> Vec<String> {
        ranked(response).into_iter().map(|row| row.0).collect()
    };
    let plain = "{\"snapshot_revision\":1}\n";
    assert_eq!(words(&query("wubi98", plain)), ["乙", "丙丁"]);
    assert_eq!(words(&query("wubi", plain)), ["甲", "甲乙"]);
    // 98 版的个人词条回放进 `wubi98`，不出现在 86 版的候选里。
    let snapshot = concat!(
        "{\"snapshot_revision\":2}\n",
        "{\"previous\":null,\"replacement\":{\"kind\":\"wubi98\",\"code\":\"kg\",\"word\":\"戊己\",\"weight\":999}}\n",
    );
    let response = query("wubi98", snapshot);
    assert_eq!(words(&response)[0], "戊己", "{response}");
    assert_eq!(response["revision"], json!(2));
    assert!(!words(&query("wubi", snapshot)).contains(&"戊己".to_owned()));
}

#[test]
fn wubi98_personal_delete_reports_the_wubi98_kind() {
    let resources = wubi_resources();
    let response = personal(
        json!({"operation": "personal_delete",
               "query": {"operation": "candidates", "scheme": "wubi98", "text": "kg"},
               "action": {"code": "kg", "word": "丙丁"}}),
        "{\"snapshot_revision\":4}\n",
        resources.path(),
    );
    assert_eq!(
        response,
        json!({"deleted": {"kind": "wubi98", "code": "kg", "word": "丙丁", "weight": 100, "user_inserted": false},
               "changed": true, "revision": 4})
    );
}

#[test]
fn wubi98_dictionary_catalog_reads_the_wubi98_table() {
    let resources = wubi_resources();
    let words = |kind: &str| -> Vec<String> {
        let response = run(
            json!({"operation": "dictionary", "kind": kind, "text": "kg"}),
            resources.path(),
        );
        response["entries"]
            .as_array()
            .unwrap_or_else(|| panic!("no entries in {response}"))
            .iter()
            .map(|entry| entry["word"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(words("wubi98"), ["乙", "丙丁"]);
    assert_eq!(words("wubi"), ["甲", "甲乙"]);
}

#[test]
fn validate_snapshot_accepts_wubi98_entries() {
    let resources = resources();
    let snapshot = concat!(
        "{\"type\":\"header\",\"data\":{}}\n",
        "{\"type\":\"entry\",\"data\":{\"kind\":\"wubi98\",\"code\":\"wq\",\"word\":\"你\",\"weight\":5}}\n",
        "{\"type\":\"overlay\",\"deleted\":false,\"data\":{\"kind\":\"wubi98\",\"code\":\"wq\",\"word\":\"你\",\"weight\":5}}\n",
    );
    assert_eq!(
        personal(
            json!({"operation": "validate_snapshot"}),
            snapshot,
            resources.path()
        ),
        json!({"valid": true})
    );
}
