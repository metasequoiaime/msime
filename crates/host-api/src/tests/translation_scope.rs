//! 候选词翻译范围（`candidate_translation_scope`）：查询只为高亮的候选请求候选词翻译，视图只把它画在高亮行，离线释义开关的释义照旧每一行都有。

use super::*;
use msime_client_core::preferences::{CandidateTranslationScope, TranslationTargetLanguage};

fn scoped_preferences(scope: CandidateTranslationScope) -> Preferences {
    let mut preferences = Preferences {
        candidate_translations: true,
        candidate_translation_scope: scope,
        translation_target_language: TranslationTargetLanguage::En,
        ..chinese_preferences()
    };
    preferences.tencent_tmt.enabled = false;
    preferences.custom_translation.enabled = true;
    preferences.custom_translation.endpoint = "https://translation.example.invalid".into();
    preferences
}

/// 打出 `nihao`，返回视图；夹具词库里它有「本地」「拟好」两个候选。
fn type_nihao(handle: u64) -> Value {
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    let mut view = Value::Null;
    for byte in b"nihao" {
        view = read(msime_client_character(handle, *byte, false))["value"]["view"].clone();
    }
    let texts = candidate_texts(&view);
    assert!(
        texts.starts_with(&["本地".to_owned(), "拟好".to_owned()]),
        "{view}"
    );
    view
}

fn candidate_texts(view: &Value) -> Vec<String> {
    view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["text"].as_str().unwrap().to_owned())
        .collect()
}

fn query_texts(query: &Value) -> Vec<String> {
    query["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["text"].as_str().unwrap().to_owned())
        .collect()
}

fn apply(handle: u64, generation: u64, answers: Value) -> Value {
    let payload = serde_json::to_vec(&answers).unwrap();
    read(unsafe {
        msime_client_apply_translations(handle, generation, payload.as_ptr(), payload.len())
    })
}

fn translation(view: &Value, row: usize) -> Option<&str> {
    view["candidates"][row]["translation"].as_str()
}

fn reserved(view: &Value) -> Vec<bool> {
    view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["translation_reserved"].as_bool().unwrap_or(false))
        .collect()
}

fn view(handle: u64) -> Value {
    read(msime_client_view(handle))["value"].clone()
}

#[test]
fn highlighted_scope_requests_and_draws_only_the_highlighted_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(
        dir.path(),
        scoped_preferences(CandidateTranslationScope::Highlighted),
    );
    let typed = type_nihao(handle);
    let rows = typed["candidates"].as_array().unwrap().len();

    // 没移动过高亮时只问第一个候选；离线释义开关关着，没有 gloss_query。
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), ["本地"], "{query}");
    assert!(query.get("gloss_query").is_none(), "{query}");
    assert_eq!(
        query["custom_translation"]["endpoint"],
        "https://translation.example.invalid"
    );
    let first = query["generation"].as_u64().unwrap();

    // 高亮行一直留出译文那一行，回答还没到也留；其他行不留。
    let mut expected = vec![false; rows];
    expected[0] = true;
    assert_eq!(reserved(&typed), expected, "{typed}");

    // 没升级的宿主照旧把整页答回来，也只有高亮行画出译文。
    let applied = apply(
        handle,
        first,
        json!([
            {"text": "本地", "translation": "local"},
            {"text": "拟好", "translation": "draft"}
        ]),
    );
    assert_eq!(applied["value"]["applied"], true, "{applied}");
    let shown = &applied["value"]["view"];
    assert_eq!(translation(shown, 0), Some("local"), "{shown}");
    assert_eq!(translation(shown, 1), None, "{shown}");

    // 高亮移到第二个：查询跟着换成它，第一行的译文收起，留行跟着走；第二个的译文刚才已经随整页答回来了，直接画出。
    let moved = read(msime_client_command(handle, 102))["value"]["view"].clone();
    assert_eq!(translation(&moved, 0), None, "{moved}");
    assert_eq!(translation(&moved, 1), Some("draft"), "{moved}");
    let mut expected = vec![false; rows];
    expected[1] = true;
    assert_eq!(reserved(&moved), expected, "{moved}");
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), ["拟好"], "{query}");
    let second = query["generation"].as_u64().unwrap();
    assert!(second > first);

    // 高亮已经移走，旧代次的回答不能落下来。
    let stale = apply(
        handle,
        first,
        json!([{"text": "本地", "translation": "stale"}]),
    );
    assert_eq!(stale["value"]["applied"], false, "{stale}");

    let applied = apply(
        handle,
        second,
        json!([{"text": "拟好", "translation": "draft, v."}]),
    );
    let shown = &applied["value"]["view"];
    assert_eq!(translation(shown, 0), None, "{shown}");
    assert_eq!(translation(shown, 1), Some("draft, v."), "{shown}");

    // 移回第一个：同一次组字里已经收到的译文直接画出来，不用等新的回答。
    let back = read(msime_client_command(handle, 103))["value"]["view"].clone();
    assert_eq!(translation(&back, 0), Some("local"), "{back}");
    assert_eq!(translation(&back, 1), None, "{back}");
    read(msime_client_destroy(handle));
}

#[test]
fn all_scope_keeps_translating_every_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(
        dir.path(),
        scoped_preferences(CandidateTranslationScope::All),
    );
    let typed = type_nihao(handle);
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), candidate_texts(&typed), "{query}");
    assert!(query.get("gloss_query").is_none());
    assert!(reserved(&typed).iter().all(|reserved| !reserved));
    let applied = apply(
        handle,
        query["generation"].as_u64().unwrap(),
        json!([
            {"text": "本地", "translation": "local"},
            {"text": "拟好", "translation": "draft"}
        ]),
    );
    let shown = &applied["value"]["view"];
    assert_eq!(translation(shown, 0), Some("local"));
    assert_eq!(translation(shown, 1), Some("draft"));
    let moved = read(msime_client_command(handle, 102))["value"]["view"].clone();
    assert_eq!(translation(&moved, 0), Some("local"));
    assert_eq!(translation(&moved, 1), Some("draft"));
    read(msime_client_destroy(handle));
}

#[test]
fn highlighted_scope_leaves_the_offline_gloss_on_every_row() {
    let dir = tempfile::tempdir().unwrap();
    let preferences = Preferences {
        candidate_english_gloss: true,
        ..scoped_preferences(CandidateTranslationScope::Highlighted)
    };
    let handle = test_host_with_pinyin_fixture(dir.path(), preferences);
    let typed = type_nihao(handle);
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), ["本地"], "{query}");
    assert_eq!(query["english_gloss"], true);
    // 离线释义另成一份不带任何翻译服务的查询，覆盖整页。
    let glosses = &query["gloss_query"];
    assert_eq!(query_texts(glosses), candidate_texts(&typed), "{glosses}");
    assert_eq!(glosses["generation"], query["generation"]);
    assert_eq!(glosses["english_gloss"], true);
    assert_eq!(glosses["provider"], "none");
    assert_eq!(glosses["translation_account"], false);
    for service in [
        "custom_translation",
        "tencent_tmt",
        "niutrans",
        "ai_assistant",
    ] {
        assert!(glosses[service].is_null(), "{service}: {glosses}");
    }
    assert!(glosses["resources"].is_string());
    assert!(glosses.get("gloss_query").is_none());
    assert!(glosses["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .all(|candidate| candidate["online_gloss"] == false));

    let generation = query["generation"].as_u64().unwrap();
    let applied = apply(
        handle,
        generation,
        json!([
            {"text": "本地", "translation": "local"},
            {"text": "本地", "translation": "gloss one", "gloss": true},
            {"text": "拟好", "translation": "gloss two", "gloss": true}
        ]),
    );
    let shown = &applied["value"]["view"];
    assert_eq!(translation(shown, 0), Some("local"), "{shown}");
    assert_eq!(translation(shown, 1), Some("gloss two"), "{shown}");

    // 高亮移开后，第一行退回它的离线释义；第二行在候选词翻译到达之前先画离线释义。
    let moved = read(msime_client_command(handle, 102))["value"]["view"].clone();
    assert_eq!(translation(&moved, 0), Some("gloss one"), "{moved}");
    assert_eq!(translation(&moved, 1), Some("gloss two"), "{moved}");
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), ["拟好"]);
    let applied = apply(
        handle,
        query["generation"].as_u64().unwrap(),
        json!([{"text": "拟好", "translation": "draft"}]),
    );
    let shown = &applied["value"]["view"];
    assert_eq!(translation(shown, 0), Some("gloss one"), "{shown}");
    assert_eq!(translation(shown, 1), Some("draft"), "{shown}");

    // 候选词翻译关掉、只剩离线释义时没有范围可言：查询是整页，也不留行。
    let mut offline = Preferences {
        candidate_english_gloss: true,
        ..scoped_preferences(CandidateTranslationScope::Highlighted)
    };
    offline.candidate_translations = false;
    update(handle, 1, &offline);
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), candidate_texts(&typed), "{query}");
    assert!(query.get("gloss_query").is_none());
    assert!(reserved(&view(handle)).iter().all(|reserved| !reserved));
    read(msime_client_destroy(handle));
}

#[test]
fn changing_the_scope_drops_answers_for_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(
        dir.path(),
        scoped_preferences(CandidateTranslationScope::All),
    );
    type_nihao(handle);
    let query = read(msime_client_translation_query(handle))["value"].clone();
    let old = query["generation"].as_u64().unwrap();
    apply(
        handle,
        old,
        json!([
            {"text": "本地", "translation": "local"},
            {"text": "拟好", "translation": "draft"}
        ]),
    );
    let changed = update(
        handle,
        1,
        &scoped_preferences(CandidateTranslationScope::Highlighted),
    );
    let shown = &changed["value"]["view"];
    assert_eq!(translation(shown, 0), None, "{shown}");
    assert_eq!(translation(shown, 1), None, "{shown}");
    assert!(reserved(shown)[0], "{shown}");
    // 旧范围下发出的查询的回答不能再落下来。
    let stale = apply(
        handle,
        old,
        json!([{"text": "本地", "translation": "local"}]),
    );
    assert_eq!(stale["value"]["applied"], false, "{stale}");
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), ["本地"]);
    read(msime_client_destroy(handle));
}

#[test]
fn a_highlight_off_the_displayed_page_leaves_only_the_offline_gloss_query() {
    let dir = tempfile::tempdir().unwrap();
    let preferences = Preferences {
        candidate_english_gloss: true,
        ..scoped_preferences(CandidateTranslationScope::Highlighted)
    };
    let handle = test_host_with_pinyin_fixture(dir.path(), preferences);
    let typed = type_nihao(handle);
    let generation = typed["generation"].as_u64().unwrap();
    // 宿主按自己的排版只显示了第二个候选，高亮的第一个不在这一组里。
    let seats = [1usize];
    let shown =
        read(unsafe { msime_client_set_display_page(handle, generation, seats.as_ptr(), 1) });
    assert_eq!(shown["value"]["applied"], true, "{shown}");
    let query = read(msime_client_translation_query(handle))["value"].clone();
    assert_eq!(query_texts(&query), ["拟好"], "{query}");
    assert_eq!(query["provider"], "none");
    assert!(query["custom_translation"].is_null());
    assert!(query.get("gloss_query").is_none());
    // 宿主照普通查询处理这份文档、不加标记：回答是离线释义，照样画出来。
    let applied = apply(
        handle,
        generation,
        json!([{"text": "拟好", "translation": "gloss two"}]),
    );
    assert_eq!(translation(&applied["value"]["view"], 1), Some("gloss two"));
    read(msime_client_destroy(handle));
}
