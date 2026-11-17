use super::*;

fn protocol(request: Value) -> Value {
    let bytes = request.to_string();
    // SAFETY: 合成请求在调用期间有效，返回字符串由 read 释放。
    read(unsafe { msime_client_ai_candidate_protocol(bytes.as_ptr(), bytes.len()) })
}

fn ai_host(root: &Path) -> u64 {
    let mut preferences = Preferences {
        scheme: InputScheme::Quanpin,
        ..chinese_preferences()
    };
    preferences.ai_assistant.enabled = true;
    preferences.ai_assistant.model = "synthetic-model".into();
    preferences.ai_assistant.token = "synthetic-token".into();
    preferences.ai_assistant.endpoint = "https://api.deepseek.com/chat/completions".into();
    let handle = test_host_preferences(root, preferences);
    read(msime_client_focus(handle, true));
    handle
}

fn type_input(handle: u64, input: &str) {
    for byte in input.bytes() {
        read(msime_client_character(handle, byte, false));
    }
}

fn candidates(query: &Value, text: &str, segmentation: &[&str]) -> Value {
    let body = json!({"choices":[{"message":{"content":json!({"candidates":[
        {"text":text,"segmentation":segmentation}
    ]}).to_string()}}]});
    let parsed = protocol(json!({"op":"parse","query":query,"limit":3,"body":body.to_string()}));
    assert_eq!(parsed["ok"], true, "{parsed}");
    parsed["value"].clone()
}

fn apply(handle: u64, query: &Value, texts: &Value) -> Value {
    let query = query.to_string();
    let texts = texts.to_string();
    // SAFETY: 合成查询与候选缓冲区在调用期间保持有效。
    read(unsafe {
        msime_client_apply_online_candidates(
            handle,
            query.as_ptr(),
            query.len(),
            texts.as_ptr(),
            texts.len(),
            1,
        )
    })
}

#[test]
fn ai_pinyin_alignment_raw_query_reaches_descriptor_and_alternative_commits_whole_input() {
    for (input, text, segmentation) in [
        (
            "woxiwangaikeyi",
            "我希望 AI 可以",
            vec!["wo", "xi", "wang", "ai", "ke", "yi"],
        ),
        ("danouer", "但偶尔", vec!["dan", "ou", "er"]),
        ("woaigithub", "我爱 GitHub", vec!["wo", "ai", "git", "hub"]),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let handle = ai_host(dir.path());
        type_input(handle, input);
        let before = read(msime_client_view(handle))["value"].clone();
        let query = read(msime_client_online_query(handle))["value"].clone();
        assert_eq!(query["ai_eligible"], true, "{query}");
        let bytes = query.to_string();
        // SAFETY: 查询缓冲区在调用期间有效。
        let descriptor =
            read(unsafe { msime_client_ai_request_for_query(handle, bytes.as_ptr(), bytes.len()) });
        assert_eq!(descriptor["ok"], true, "{descriptor}");
        let message: Value = serde_json::from_str(
            descriptor["value"]["body"]["messages"][1]["content"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(message["input"], input);
        assert_eq!(message["explicit_boundaries"], json!([]));
        let texts = candidates(&query, text, &segmentation);
        assert_eq!(texts, json!([text]));
        let result = apply(handle, &query, &texts);
        assert_eq!(result["value"]["applied"], true, "{result}");
        let view = &result["value"]["view"];
        let rows = view["candidates"].as_array().unwrap();
        let index = rows.iter().position(|row| row["text"] == text).unwrap();
        // AI 到达前已显示的前几个座位保持原样。
        for (old, new) in before["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .zip(rows)
            .take(index)
        {
            assert_eq!(old["text"], new["text"]);
            assert_eq!(old["id"], new["id"]);
        }
        let selected = read(msime_client_select(
            handle,
            view["generation"].as_u64().unwrap(),
            index,
        ));
        assert_eq!(selected["value"]["commit"], text, "{selected}");
        assert_eq!(selected["value"]["view"]["editing_text"], "");
        assert_eq!(apply(handle, &query, &texts)["value"]["applied"], false);
        read(msime_client_destroy(handle));
    }
}

#[test]
fn ai_pinyin_alignment_manual_boundary_and_late_answers_remain_guarded() {
    let dir = tempfile::tempdir().unwrap();
    let handle = ai_host(dir.path());
    type_input(handle, "wang'ai");
    let query = read(msime_client_online_query(handle))["value"].clone();
    let message = protocol(json!({"op":"message","query":query,"limit":3}));
    assert_eq!(message["value"]["message"]["input"], "wang'ai");
    assert_eq!(
        message["value"]["message"]["explicit_boundaries"],
        json!([4])
    );
    assert_eq!(candidates(&query, "万该", &["wan", "gai"]), json!([]));
    let texts = candidates(&query, "王 AI", &["wang", "ai"]);
    assert_eq!(texts, json!(["王 AI"]));
    type_input(handle, "a");
    assert_eq!(apply(handle, &query, &texts)["value"]["applied"], false);
    read(msime_client_command(handle, 0));
    assert_eq!(apply(handle, &query, &texts)["value"]["applied"], false);
    read(msime_client_destroy(handle));
}

#[test]
fn ai_pinyin_alignment_protocol_rejects_invalid_buffers_and_inputs() {
    // SAFETY: 空指针和超长声明必须在读取内存之前拒绝。
    assert_eq!(
        read(unsafe { msime_client_ai_candidate_protocol(std::ptr::null(), 0) })["ok"],
        false
    );
    assert_eq!(
        read(unsafe { msime_client_ai_candidate_protocol(b"x".as_ptr(), 8 * 1024 * 1024 + 1) })
            ["ok"],
        false
    );
    assert_eq!(protocol(json!({"op":"invalid"}))["ok"], false);
}
