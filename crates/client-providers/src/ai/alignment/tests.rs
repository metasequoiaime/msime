use super::*;

fn request(raw: &str, local: &[&str]) -> AiSuggestionRequest {
    AiSuggestionRequest {
        mode: AiInputMode::Pinyin,
        input: raw.into(),
        segmented_pinyin: local.iter().map(|part| (*part).into()).collect(),
        context: String::new(),
        candidate_limit: 3,
        sentence_punctuation: false,
    }
}

fn parse(request: &AiSuggestionRequest, entries: Value) -> Vec<String> {
    let body =
        json!({"choices":[{"message":{"content":json!({"candidates":entries}).to_string()}}]});
    parse_candidate_response(&serde_json::to_vec(&body).unwrap(), request)
        .unwrap()
        .candidates
        .into_iter()
        .map(|row| row.text)
        .collect()
}

#[test]
fn raw_input_and_manual_boundaries_are_distinct_from_local_segmentation() {
    let request = request(
        "wo'xi'wang'ai'ke'yi",
        &["wo", "xi", "wan", "gai", "ke", "yi"],
    );
    let message = user_message(&request).unwrap();
    assert_eq!(message["input"], "wo'xi'wang'ai'ke'yi");
    assert_eq!(
        message["segmented_pinyin"],
        json!(["wo", "xi", "wan", "gai", "ke", "yi"])
    );
    assert_eq!(message["explicit_boundaries"], json!([2, 4, 8, 10, 12]));
}

#[test]
fn alternative_segmentation_can_align_chinese_and_an_english_acronym() {
    let request = request("woxiwangaikeyi", &["wo", "xi", "wan", "gai", "ke", "yi"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"我希望 AI 可以", "segmentation":["wo","xi","wang","ai","ke","yi"], "confidence":0.96}
            ])
        ),
        vec!["我希望 AI 可以"]
    );
}

#[test]
fn optional_punctuation_does_not_consume_pinyin_or_split_english() {
    let mut request = request(
        "woxiwangaikeyi",
        &["wo", "xi", "wang", "a", "i", "ke", "yi"],
    );
    let entries = json!([
        {"text":"，我希望 AI 可以"},
        {"text":"我希望，，AI 可以。"},
        {"text":"我希望 A，I 可以。"},
        {"text":"我很好，AI 可以。"},
        {"text":"我希望，AI。"},
        {"text":"我希望，AI 可以。"},
        {"text":"我希望 AI 可以"}
    ]);
    assert_eq!(parse(&request, entries.clone()), ["我希望 AI 可以"]);
    assert!(user_message(&request)
        .unwrap()
        .get("sentence_punctuation")
        .is_none());

    request.sentence_punctuation = true;
    assert_eq!(
        parse(&request, entries),
        ["我希望，AI 可以。", "我希望 AI 可以"]
    );
    let message = user_message(&request).unwrap();
    assert_eq!(message["sentence_punctuation"], true);
    assert!(message["punctuation_instruction"]
        .as_str()
        .unwrap()
        .contains("标点不放进 segmentation"));
}

#[test]
fn danouer_uses_a_complete_alternative_path() {
    let request = request("danouer", &["da", "nou", "er"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"但偶尔", "segmentation":["dan","ou","er"]}
            ])
        ),
        vec!["但偶尔"]
    );
}

#[test]
fn manual_boundaries_cannot_be_crossed_by_model_segmentation() {
    let request = request("wang'ai", &["wang", "ai"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"万该", "segmentation":["wan","gai"]},
                {"text":"王 AI", "segmentation":["wang","ai"]}
            ])
        ),
        vec!["王 AI"]
    );
}

#[test]
fn literal_english_intervals_need_not_be_complete_pinyin() {
    let request = request("woaiGitHub", &["wo", "ai", "git", "h", "u", "b"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"我爱 GitHub", "segmentation":["wo","ai","git","hub"]},
                {"text":"我爱 GitLab", "segmentation":["wo","ai","git","hub"]}
            ])
        ),
        vec!["我爱 GitHub"]
    );
}

#[test]
fn rejects_added_deleted_rewritten_and_invalid_syllables_before_applying_limit() {
    let mut request = request("nihao", &["ni", "hao"]);
    request.candidate_limit = 1;
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"你很好", "segmentation":["ni","hen","hao"]},
                {"text":"你", "segmentation":["ni","hao"]},
                {"text":"再见", "segmentation":["ni","hao"]},
                {"text":"你好", "segmentation":["n","ihao"]},
                {"text":"你好", "segmentation":["ni","hao"]}
            ])
        ),
        vec!["你好"]
    );
}

#[test]
fn preserves_legacy_text_only_replies_when_the_local_path_aligns() {
    let request = request("nihao", &["ni", "hao"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"你好"}, {"text":"你好"}, {"text":"合成"}
            ])
        ),
        vec!["你好"]
    );
}

#[test]
fn missing_raw_input_keeps_other_schemes_on_their_existing_path() {
    let request = request("", &["wang", "ai"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"万该", "segmentation":["wan","gai"]}, {"text":"王爱"}
            ])
        ),
        vec!["王爱"]
    );
}

#[test]
fn polyphonic_characters_and_umlaut_keyboard_aliases_align() {
    for (raw, local, text) in [
        ("chongqing", vec!["chong", "qing"], "重庆"),
        ("nv'er", vec!["nv", "er"], "女儿"),
        ("lue", vec!["lve"], "略"),
        ("jv", vec!["ju"], "居"),
    ] {
        assert_eq!(
            parse(&request(raw, &local), json!([{"text":text}])),
            vec![text]
        );
    }
}

#[test]
fn malformed_metadata_and_uncovered_output_are_rejected_without_panics() {
    let request = request("nihao", &["ni", "hao"]);
    for entries in [
        json!([{"text":"你好", "segmentation":null}]),
        json!([{"text":"你好", "segmentation":[]}]),
        json!([{"text":"你好", "segmentation":["ni", ""]}]),
        json!([{"text":"你好", "segmentation":["ni", "hao", "a"]}]),
        json!([{"text":"你好!", "segmentation":["ni", "hao"]}]),
        json!([{"text":"你好1", "segmentation":["ni", "hao"]}]),
    ] {
        assert!(parse(&request, entries).is_empty());
    }
    assert!(parse_candidate_response(b"{}", &request).is_none());
}

#[test]
fn pinyin_keyboard_aliases_do_not_rewrite_literal_english_letters() {
    let request = request("lue", &["lve"]);
    assert_eq!(
        parse(
            &request,
            json!([
                {"text":"LVE", "segmentation":["lve"]},
                {"text":"略", "segmentation":["lve"]},
                {"text":"LUE", "segmentation":["lve"]}
            ])
        ),
        vec!["略", "LUE"]
    );
}

#[test]
fn english_spelling_correction_and_generic_reply_parsing_remain_available() {
    let request = AiSuggestionRequest {
        mode: AiInputMode::English,
        input: "synthetci".into(),
        segmented_pinyin: vec![],
        context: String::new(),
        candidate_limit: 3,
        sentence_punctuation: false,
    };
    assert_eq!(
        parse(&request, json!([{"text":"synthetic"}])),
        vec!["synthetic"]
    );
    let body =
        json!({"choices":[{"message":{"content":"{\"candidates\":[{\"text\":\"合成回复\"}]}"}}]});
    assert_eq!(
        super::super::parse_chat_completion_response(&serde_json::to_vec(&body).unwrap(), 3)
            .unwrap()
            .candidates[0]
            .text,
        "合成回复"
    );
}

#[test]
fn invalid_entries_do_not_discard_neighbors_or_consume_the_limit() {
    let mut request = request("nihao", &["ni", "hao"]);
    request.candidate_limit = 2;
    for invalid in [
        json!(null),
        json!({}),
        json!(42),
        json!(true),
        json!("synthetic"),
        json!([]),
        json!({"text":null}),
        json!({"text":12}),
        json!({"text":true}),
        json!({"text":{}}),
        json!({"text":[]}),
        json!({"text":"bad\ntext"}),
        json!({"text":"你".repeat(1366)}),
    ] {
        assert_eq!(
            parse(
                &request,
                json!([
                    {"text":"你好"}, invalid, {"text":"拟好"}, {"text":"你好"}, {"text":"你号"}
                ])
            ),
            vec!["你好", "拟好"]
        );
    }
}
