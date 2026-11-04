//! 通过宿主编排验证连滑、隐式结束和候选身份的提交边界。

use super::*;

#[test]
fn bilingual_glide_selection_is_the_only_commit_boundary() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["resources", "dictionaries"] {
        std::fs::create_dir_all(dir.path().join(name)).unwrap();
        rusqlite::Connection::open(dir.path().join(name).join("msime-english.db"))
            .unwrap()
            .execute_batch("CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER); INSERT INTO english_words VALUES('hello','hello',1000),('world','world',900);")
            .unwrap();
    }
    let handle = test_host_with_pinyin_fixture(dir.path(), chinese_preferences());
    for (strokes, expected) in [
        (vec!["nihao", "hello"], "本地 hello "),
        (vec!["hello", "nihao"], "hello 本地"),
        (vec!["hello", "world"], "hello world "),
    ] {
        read(msime_client_focus(handle, true));
        let mut last = Value::Null;
        for spelling in strokes {
            let mut request: Value = serde_json::from_str(&glide_request(spelling)).unwrap();
            request["bilingual"] = json!(true);
            let encoded = request.to_string();
            last = read(unsafe { msime_client_glide(handle, encoded.as_ptr(), encoded.len()) });
            assert_eq!(last["ok"], true, "{last}");
            assert!(last["value"]["commit"].is_null(), "{last}");
            assert_eq!(
                last["value"]["view"]["requires_candidate_selection"], true,
                "{last}"
            );
        }
        // 原文提交、结束组字、失焦前结束及标点都不能替用户选择高亮候选。
        for command in [9, 2, 11, 15, 20] {
            let blocked = read(msime_client_command(handle, command));
            assert_eq!(blocked["ok"], true, "{blocked}");
            assert!(blocked["value"]["commit"].is_null(), "{blocked}");
            assert_eq!(
                blocked["value"]["view"]["requires_candidate_selection"],
                true
            );
            assert_eq!(
                blocked["value"]["view"]["editing_text"],
                last["value"]["view"]["editing_text"]
            );
            last = blocked;
        }
        let blocked = read(msime_client_punctuation(handle, b','));
        assert_eq!(blocked["ok"], true, "{blocked}");
        assert!(blocked["value"]["commit"].is_null(), "{blocked}");
        let view = &blocked["value"]["view"];
        let index = view["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .position(|row| row["text"] == expected.trim_end())
            .unwrap();
        let picked = read(msime_client_select(
            handle,
            view["generation"].as_u64().unwrap(),
            index,
        ));
        assert_eq!(picked["value"]["commit"], expected, "{picked}");
        assert_eq!(picked["value"]["view"]["editing_text"], "");
        assert!(picked["value"]["view"]
            .get("requires_candidate_selection")
            .is_none());
    }
    read(msime_client_destroy(handle));
}

#[test]
fn bilingual_glide_space_selects_the_highlighted_whole_candidate() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["resources", "dictionaries"] {
        std::fs::create_dir_all(dir.path().join(name)).unwrap();
        rusqlite::Connection::open(dir.path().join(name).join("msime-english.db")).unwrap()
            .execute_batch("CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER); INSERT INTO english_words VALUES('hello','hello',1000),('hello','HELLO',900),('world','world',900);").unwrap();
    }
    let handle = test_host_with_pinyin_fixture(dir.path(), chinese_preferences());
    read(msime_client_focus(handle, true));
    for spelling in ["hello", "world"] {
        let mut request: Value = serde_json::from_str(&glide_request(spelling)).unwrap();
        request["bilingual"] = json!(true);
        let encoded = request.to_string();
        let result = read(unsafe { msime_client_glide(handle, encoded.as_ptr(), encoded.len()) });
        assert!(result["value"]["commit"].is_null(), "{result}");
    }
    let moved = read(msime_client_command(handle, 102));
    let rows = moved["value"]["view"]["candidates"].as_array().unwrap();
    assert!(rows.len() > 1, "{moved}");
    let expected = rows[1]["text"].as_str().unwrap().to_owned() + " ";
    let selected = read(msime_client_command(handle, 1));
    assert_eq!(selected["value"]["commit"], expected, "{selected}");
    assert_eq!(selected["value"]["view"]["editing_text"], "");
    assert!(selected["value"]["view"]
        .get("requires_candidate_selection")
        .is_none());
    read(msime_client_destroy(handle));
}

#[test]
fn bilingual_glide_confirmation_state_clears_on_cancel_and_is_absent_for_typed_input() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(dir.path(), chinese_preferences());
    read(msime_client_focus(handle, true));
    let mut request: Value = serde_json::from_str(&glide_request("nihao")).unwrap();
    request["bilingual"] = json!(true);
    let encoded = request.to_string();
    let pending = read(unsafe { msime_client_glide(handle, encoded.as_ptr(), encoded.len()) });
    assert_eq!(
        pending["value"]["view"]["requires_candidate_selection"],
        true
    );
    let cancelled = read(msime_client_command(handle, 3));
    assert!(cancelled["value"]["view"]
        .get("requires_candidate_selection")
        .is_none());
    for letter in b"nihao" {
        let typed = read(msime_client_character(handle, *letter, false));
        assert!(typed["value"]["view"]
            .get("requires_candidate_selection")
            .is_none());
    }
    read(msime_client_destroy(handle));
}
