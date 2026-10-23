//! Jev（TypeSafe Choice API）协议适配层的合成 fixture 测试；不打真实网络。
use msime_client_core::preferences::{AiAssistantPreferences, Preferences};
use msime_client_providers::ai::{jev, AiError, AiInputMode, AiSuggestionRequest};
use serde_json::{json, Value};

fn pinyin_request() -> AiSuggestionRequest {
    AiSuggestionRequest {
        mode: AiInputMode::Pinyin,
        input: String::new(),
        segmented_pinyin: vec!["shi".into(), "shi".into()],
        context: "方案已经通过，明天开始".into(),
        candidate_limit: 3,
    }
}

fn candidates() -> Vec<String> {
    vec!["试试".into(), "实施".into(), "时事".into(), "事实".into()]
}

fn jev_config() -> AiAssistantPreferences {
    let mut config = Preferences::default().ai_assistant;
    config.enabled = true;
    config.provider = "jev".into();
    config.endpoint = jev::ENDPOINT.into();
    config.model = jev::MODEL.into();
    config.tokens.insert(
        "https://api.typesafe.ai:443".into(),
        "synthetic-token".into(),
    );
    config
}

#[test]
fn request_body_carries_state_and_choice_criteria() {
    let body = jev::build_request(
        jev::MODEL,
        "方案已经通过，明天开始",
        &["shi".into(), "shi".into()],
        &candidates(),
    );
    assert_eq!(body["model"], jev::MODEL);
    assert_eq!(body["state"]["preceding_text"], "方案已经通过，明天开始");
    assert_eq!(body["state"]["segmented_pinyin"], json!(["shi", "shi"]));
    let question = &body["questions"]["candidate"];
    assert_eq!(question["type"], "choice");
    assert!(question["instructions"].as_str().unwrap().len() > 32);
    let criteria = question["criteria"].as_object().unwrap();
    assert_eq!(criteria.len(), candidates().len() + 1);
    for (index, candidate) in candidates().iter().enumerate() {
        assert_eq!(criteria[&format!("c{index}")], json!(candidate));
    }
    assert_eq!(
        criteria["none"],
        "None of these candidates fits the pinyin and preceding text."
    );
}

#[test]
fn request_body_caps_the_candidate_pool_at_32() {
    let pool: Vec<String> = (0..40).map(|index| format!("候选{index}")).collect();
    let body = jev::build_request(jev::MODEL, "", &["ni".into()], &pool);
    let criteria = body["questions"]["candidate"]["criteria"]
        .as_object()
        .unwrap();
    assert_eq!(criteria.len(), jev::MAX_CANDIDATES + 1);
    assert!(criteria.contains_key("c31"));
    assert!(!criteria.contains_key("c32"));
    assert!(criteria.contains_key("none"));
}

#[test]
fn choice_response_maps_back_to_the_offered_candidate() {
    let candidates = candidates();
    let reply = json!({"answers":{"candidate":{"type":"choice","choice":"c2"}}});
    assert_eq!(
        jev::parse_choice(reply.to_string().as_bytes(), &candidates),
        Some("时事".to_owned())
    );
    // 服务弃权、未知槽位和非 choice 回答都不会变成输入文本。
    for body in [
        json!({"answers":{"candidate":{"type":"choice","choice":"none"}}}),
        json!({"answers":{"candidate":{"type":"choice","choice":"c9"}}}),
        json!({"answers":{"candidate":{"type":"ranked","choice":"c2"}}}),
        json!({"answers":{"candidate":{"type":"choice"}}}),
        json!({"answers":{}}),
    ] {
        assert_eq!(
            jev::parse_choice(body.to_string().as_bytes(), &candidates),
            None,
            "{body}"
        );
    }
    assert_eq!(jev::parse_choice(b"not json", &candidates), None);
    assert_eq!(jev::parse_choice(&[], &candidates), None);
}

#[test]
fn http_request_descriptor_posts_choice_body_to_the_official_endpoint() {
    let config = jev_config();
    let descriptor = jev::jev_http_request(&config, &pinyin_request(), &candidates())
        .unwrap()
        .unwrap();
    assert_eq!(descriptor["url"], jev::ENDPOINT);
    assert_eq!(descriptor["method"], "POST");
    assert_eq!(
        descriptor["headers"]["Authorization"],
        "Bearer synthetic-token"
    );
    assert_eq!(descriptor["headers"]["Content-Type"], "application/json");
    // Jev 的预算比 Chat Completions 紧：连接 1500ms、总 2500ms。
    assert_eq!(descriptor["timeout_ms"], 2500);
    assert_eq!(descriptor["connect_timeout_ms"], 1500);
    assert_eq!(descriptor["max_response_bytes"], 1048576);
    let body = &descriptor["body"];
    assert_eq!(body["state"]["segmented_pinyin"], json!(["shi", "shi"]));
    assert!(body["questions"]["candidate"]["criteria"].is_object());
}

#[test]
fn http_request_guards_disabled_wrong_endpoint_and_thin_pools() {
    let mut config = jev_config();
    config.enabled = false;
    assert_eq!(
        jev::jev_http_request(&config, &pinyin_request(), &candidates()).unwrap(),
        None
    );
    // Jev 只和官方端点通信；指向其他地址的配置是配置错误，不是「本次不发」。
    let mut config = jev_config();
    config.endpoint = "https://synthetic.invalid/v1/systemone".into();
    assert_eq!(
        jev::jev_http_request(&config, &pinyin_request(), &candidates()),
        Err(AiError::InvalidConfiguration)
    );
    // 一个候选无可选择，前文不必离机（Windows `candidates.size() < 2` 的提前返回）。
    assert_eq!(
        jev::jev_http_request(&jev_config(), &pinyin_request(), &["试试".into()]).unwrap(),
        None
    );
    assert_eq!(
        jev::jev_http_request(&jev_config(), &pinyin_request(), &[]).unwrap(),
        None
    );
    // 英文模式没有拼音分段可匹配。
    let mut english = pinyin_request();
    english.mode = AiInputMode::English;
    english.input = "synthetic".into();
    english.segmented_pinyin.clear();
    assert_eq!(
        jev::jev_http_request(&jev_config(), &english, &candidates()).unwrap(),
        None
    );
    // 没有可用凭据时构造不出带 Authorization 的描述符。
    let mut config = jev_config();
    config.tokens.clear();
    assert_eq!(
        jev::jev_http_request(&config, &pinyin_request(), &candidates()),
        Err(AiError::InvalidConfiguration)
    );
}

#[test]
fn provider_token_slot_resolves_against_the_official_origin() {
    // Windows 的 token_jev 槽在新架构按 provider id 存放；端点固定为官方地址时它能被解析。
    let mut config = jev_config();
    config.tokens.clear();
    config
        .tokens
        .insert("jev".into(), "synthetic-slot-token".into());
    let descriptor = jev::jev_http_request(&config, &pinyin_request(), &candidates())
        .unwrap()
        .unwrap();
    assert_eq!(
        descriptor["headers"]["Authorization"],
        "Bearer synthetic-slot-token"
    );
    // descriptor 形状与 chat_completion_http_request 一致，宿主 transport 可复用。
    let descriptor: Value = descriptor;
    for key in [
        "url",
        "method",
        "headers",
        "body",
        "timeout_ms",
        "connect_timeout_ms",
        "max_response_bytes",
    ] {
        assert!(descriptor.get(key).is_some(), "{key}");
    }
}
