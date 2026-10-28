//! 云候选的配置、HTTP 描述符、响应路径和进程内缓存；不执行网络请求。

use super::{
    build_google_url, parse_google_response, urlencoding, valid_cloud_input, MAX_CANDIDATE,
    MAX_RESPONSE,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudCandidateSource {
    #[default]
    Google,
    Baidu,
    Custom,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CloudHttpMethod {
    #[default]
    Get,
    Post,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudResponseFormat {
    #[default]
    Candidates,
    Google,
    Baidu,
    JsonPath,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct CloudCandidatePreferences {
    pub source: CloudCandidateSource,
    pub endpoint: String,
    pub method: CloudHttpMethod,
    pub headers_json: String,
    pub body_template: String,
    pub response_format: CloudResponseFormat,
    pub response_path: String,
    pub timeout_ms: u64,
    pub cache_enabled: bool,
    /// 另行允许向自定义服务发送刚上屏的文本，请求后续词；不随账号同步。
    pub prediction_enabled: bool,
}

impl Default for CloudCandidatePreferences {
    fn default() -> Self {
        Self {
            source: CloudCandidateSource::Google,
            endpoint: String::new(),
            method: CloudHttpMethod::Get,
            headers_json: "{}".into(),
            body_template: r#"{"query":"{query}"}"#.into(),
            response_format: CloudResponseFormat::Candidates,
            response_path: "data.0".into(),
            timeout_ms: super::REQUEST_TIMEOUT_MS,
            cache_enabled: true,
            prediction_enabled: false,
        }
    }
}

impl CloudCandidatePreferences {
    pub fn validate(&self) -> bool {
        if !(100..=10_000).contains(&self.timeout_ms)
            || self.headers_json.len() > 4096
            || serde_json::from_str::<BTreeMap<String, String>>(&self.headers_json)
                .ok()
                .is_none_or(|headers| {
                    headers.len() > 16
                        || headers
                            .iter()
                            .map(|(name, value)| name.len() + value.len())
                            .sum::<usize>()
                            > 4096
                        || !valid_headers(&headers)
                })
            || !valid_path(&self.response_path)
            || self.body_template.len() > 4096
            || self.endpoint.len() > 2048
        {
            return false;
        }
        if self.source != CloudCandidateSource::Custom {
            return true;
        }
        if self.endpoint.split_once("://").is_some_and(|(_, rest)| {
            rest.split(['/', '?'])
                .next()
                .is_some_and(|host| host.contains("{query}"))
        }) {
            return false;
        }
        if !valid_endpoint(&self.endpoint.replace("{query}", "query")) {
            return false;
        }
        let body = if self.method == CloudHttpMethod::Post {
            let Ok(body) = serde_json::from_str::<Value>(&self.body_template) else {
                return false;
            };
            if !body.is_object() {
                return false;
            }
            Some(body)
        } else {
            None
        };
        self.endpoint.contains("{query}") || body.as_ref().is_some_and(contains_query)
    }

    /// 完整配置的摘要只用于本机缓存和回包身份，不包含可读的令牌。
    pub fn identity(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        hex::encode(Sha256::digest(bytes))
    }

    pub fn recipient(&self) -> String {
        match self.source {
            CloudCandidateSource::Google => "Google input-tools（inputtools.google.com）".into(),
            CloudCandidateSource::Baidu => "百度输入法（olimenew.baidu.com）".into(),
            CloudCandidateSource::Custom => {
                url::Url::parse(&self.endpoint.replace("{query}", "query"))
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .unwrap_or_else(|| "所选自定义服务".into())
            }
        }
    }
}

fn contains_query(value: &Value) -> bool {
    match value {
        Value::String(value) => value.contains("{query}"),
        Value::Array(values) => values.iter().any(contains_query),
        Value::Object(values) => values.values().any(contains_query),
        _ => false,
    }
}

fn replace_query(value: &mut Value, query: &str) {
    match value {
        Value::String(value) => *value = value.replace("{query}", query),
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| replace_query(value, query)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| replace_query(value, query)),
        _ => {}
    }
}

/// 公网只走 HTTPS；本机 HTTP 供用户自己的服务使用。拒绝 URL 解析器可能各自解释的写法。
pub fn valid_endpoint(endpoint: &str) -> bool {
    if endpoint.is_empty()
        || !crate::text::is_bounded_text(endpoint, 2048)
        || endpoint.contains('\\')
        || endpoint.contains(' ')
    {
        return false;
    }
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    if !crate::endpoint::valid_endpoint_shape(endpoint, &url, url.scheme()) {
        return false;
    }
    match url.scheme() {
        "https" => endpoint.starts_with("https://"),
        "http" => {
            let authority = endpoint
                .strip_prefix("http://")
                .and_then(|rest| rest.split(['/', '?']).next());
            matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
                && authority.is_some_and(|value| {
                    ["localhost", "127.0.0.1", "[::1]"].iter().any(|host| {
                        value == *host
                            || value
                                .strip_prefix(host)
                                .is_some_and(|rest| rest.starts_with(':'))
                    })
                })
        }
        _ => false,
    }
}

fn valid_headers(headers: &BTreeMap<String, String>) -> bool {
    headers.len() <= 18
        && headers
            .iter()
            .map(|(name, value)| name.len() + value.len())
            .sum::<usize>()
            <= 4352
        && headers.iter().all(|(name, value)| {
            !name.is_empty()
                && name.len() <= 128
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
                && ![
                    "host",
                    "cookie",
                    "cookie2",
                    "content-length",
                    "connection",
                    "transfer-encoding",
                    "proxy-authorization",
                    "proxy-connection",
                    "accept-encoding",
                ]
                .contains(&name.to_ascii_lowercase().as_str())
                && value.is_ascii()
                && crate::text::is_bounded_text(value, 4096)
                && headers
                    .keys()
                    .filter(|key| key.eq_ignore_ascii_case(name))
                    .count()
                    == 1
        })
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 128
        && path.split('.').count() <= 16
        && path.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CloudHttpRequest {
    pub url: String,
    pub method: CloudHttpMethod,
    pub headers: BTreeMap<String, String>,
    pub body_utf8: String,
    pub timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub max_response_bytes: usize,
    pub provider_id: String,
    pub cache_key: String,
    pub cache_enabled: bool,
    pub response_format: CloudResponseFormat,
    pub response_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_response: Option<String>,
}

impl CloudHttpRequest {
    pub fn validate(&self) -> bool {
        valid_endpoint(&self.url)
            && valid_headers(&self.headers)
            && self.body_utf8.len() <= 8192
            && (100..=10_000).contains(&self.timeout_ms)
            && self.connect_timeout_ms > 0
            && self.connect_timeout_ms <= self.timeout_ms
            && self.max_response_bytes == MAX_RESPONSE
            && valid_path(&self.response_path)
            && self.provider_id.len() == 64
            && self
                .provider_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            && self.cache_key.len() == 64
            && self.cache_key.bytes().all(|byte| byte.is_ascii_hexdigit())
            && self
                .cached_response
                .as_ref()
                .is_none_or(|body| body.len() <= MAX_RESPONSE)
            && (self.method == CloudHttpMethod::Post || self.body_utf8.is_empty())
    }
}

type CachedAnswers = HashMap<String, (Vec<String>, Instant)>;
static CACHE: OnceLock<Mutex<CachedAnswers>> = OnceLock::new();

pub fn build_request(
    config: &CloudCandidatePreferences,
    query: &str,
    japanese: bool,
) -> Option<CloudHttpRequest> {
    build_request_for_input(config, query, japanese, None)
}

/// 英文仅发往用户配置的服务；请求显式携带模式和前文，不回退到拼音转换服务。
pub fn build_english_request(
    config: &CloudCandidatePreferences,
    input: &str,
    context: &str,
) -> Option<CloudHttpRequest> {
    if config.source != CloudCandidateSource::Custom
        || !input.bytes().all(|byte| byte.is_ascii_alphabetic())
        || !crate::text::is_bounded_text_with_options(context, 16 * 1024, true)
    {
        return None;
    }
    build_request_for_input(config, input, false, Some(("english", context)))
}

fn build_request_for_input(
    config: &CloudCandidatePreferences,
    query: &str,
    japanese: bool,
    context_mode: Option<(&str, &str)>,
) -> Option<CloudHttpRequest> {
    if !valid_cloud_input(query) || !config.validate() {
        return None;
    }
    let builtin = config.source != CloudCandidateSource::Custom;
    if config.source == CloudCandidateSource::Baidu
        && (japanese
            || !query
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic() || byte == b'\''))
    {
        return None;
    }
    let mut url = match config.source {
        CloudCandidateSource::Google => build_google_url(query, japanese)?,
        CloudCandidateSource::Baidu => format!(
            "https://olimenew.baidu.com/py?input={}&inputtype=py&resultcoding=utf-8",
            urlencoding(query)
        ),
        CloudCandidateSource::Custom => config.endpoint.replace("{query}", &urlencoding(query)),
    };
    let method = if builtin {
        CloudHttpMethod::Get
    } else {
        config.method
    };
    let mut body_utf8 = String::new();
    if method == CloudHttpMethod::Post {
        let mut body: Value = serde_json::from_str(&config.body_template).ok()?;
        replace_query(&mut body, query);
        if let Some((mode, context)) = context_mode {
            body["mode"] = Value::String(mode.into());
            body["context"] = Value::String(context.into());
        }
        body_utf8 = serde_json::to_string(&body).ok()?;
    }
    if method == CloudHttpMethod::Get {
        if let Some((mode, context)) = context_mode {
            let mut parsed = url::Url::parse(&url).ok()?;
            // 同名参数由本次英文请求决定，避免模板里的拼音模式覆盖它。
            let pairs: Vec<_> = parsed
                .query_pairs()
                .filter(|(key, _)| key != "mode" && key != "context")
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect();
            parsed.set_query(None);
            parsed
                .query_pairs_mut()
                .extend_pairs(pairs)
                .append_pair("mode", mode)
                .append_pair("context", context);
            url = parsed.into();
        }
    }
    let mut headers: BTreeMap<String, String> = if builtin {
        BTreeMap::new()
    } else {
        serde_json::from_str(&config.headers_json).ok()?
    };
    if !headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("Accept"))
    {
        headers.insert("Accept".into(), "application/json".into());
    }
    if method == CloudHttpMethod::Post
        && !headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case("Content-Type"))
    {
        headers.insert("Content-Type".into(), "application/json".into());
    }
    let provider_id = config.identity();
    let cache_input = context_mode.map_or_else(
        || format!("{provider_id}:{japanese}:{query}"),
        |(mode, context)| format!("{provider_id}:{mode}:{query}:{context}"),
    );
    let cache_key = hex::encode(Sha256::digest(cache_input));
    let mut request = CloudHttpRequest {
        url,
        method,
        headers,
        body_utf8,
        timeout_ms: config.timeout_ms,
        connect_timeout_ms: config.timeout_ms.min(super::CONNECT_TIMEOUT_MS),
        max_response_bytes: MAX_RESPONSE,
        provider_id,
        cache_key,
        cache_enabled: config.cache_enabled,
        response_format: match config.source {
            CloudCandidateSource::Google => CloudResponseFormat::Google,
            CloudCandidateSource::Baidu => CloudResponseFormat::Baidu,
            CloudCandidateSource::Custom => config.response_format,
        },
        response_path: config.response_path.clone(),
        cached_response: None,
    };
    if config.cache_enabled {
        if let Ok(mut cache) = CACHE.get_or_init(Mutex::default).lock() {
            cache.retain(|_, (_, deadline)| *deadline > Instant::now());
            if let Some((candidate, _)) = cache.get(&request.cache_key) {
                request.cached_response = Some(serde_json::json!({"candidates": candidate.iter().map(|text| serde_json::json!({"text":text})).collect::<Vec<_>>() }).to_string());
            }
        }
    }
    request.validate().then_some(request)
}

/// 上屏后云预测独立于拼音转换，只发送本次上屏的有界汉字上下文。
/// 内置拼音服务没有后续词契约，不能把汉字误发成拼音。
pub fn build_prediction_request(
    config: &CloudCandidatePreferences,
    context: &str,
) -> Option<CloudHttpRequest> {
    if !config.prediction_enabled || config.source != CloudCandidateSource::Custom {
        return None;
    }
    build_request_for_input(config, context, false, Some(("prediction", context)))
}

pub fn parse_response(request: &CloudHttpRequest, body: &[u8]) -> Option<String> {
    parse_responses(request, body).into_iter().next()
}

/// 返回有界、去重的纯文本候选；不执行服务回包里的链接、指令或广告动作。
pub fn parse_responses(request: &CloudHttpRequest, body: &[u8]) -> Vec<String> {
    if !request.validate() || body.len() > MAX_RESPONSE {
        return Vec::new();
    }
    let format = if request
        .cached_response
        .as_ref()
        .is_some_and(|cached| cached.as_bytes() == body)
    {
        CloudResponseFormat::Candidates
    } else {
        request.response_format
    };
    let mut candidates = Vec::new();
    let mut push = |value: &Value| {
        let Some(raw) = value.as_str() else { return };
        let text = raw.trim();
        if candidates.len() < 8
            && !text.is_empty()
            && crate::text::is_bounded_text(raw, MAX_CANDIDATE)
            && !candidates.iter().any(|row| row == text)
        {
            candidates.push(text.to_owned());
        }
    };
    if format == CloudResponseFormat::Google {
        if let Some(text) = parse_google_response(body) {
            push(&Value::String(text));
        }
    } else if let Ok(root) = serde_json::from_slice::<Value>(body) {
        match format {
            CloudResponseFormat::Candidates => {
                if let Some(rows) = root.get("candidates").and_then(Value::as_array) {
                    for row in rows.iter().take(64) {
                        push(&row["text"]);
                    }
                }
            }
            CloudResponseFormat::Baidu => {
                if root["status"] == "T" && root["errno"] == "0" {
                    if let Some(rows) = root["result"][0].as_array() {
                        for row in rows.iter().take(64) {
                            push(&row[0]);
                        }
                    }
                }
            }
            CloudResponseFormat::JsonPath => {
                let mut value = Some(&root);
                for segment in request.response_path.split('.') {
                    value = value.and_then(|value| {
                        if value.is_array() {
                            value.get(segment.parse::<usize>().ok()?)
                        } else {
                            value.get(segment)
                        }
                    });
                }
                if let Some(value) = value {
                    if let Some(rows) = value.as_array() {
                        for row in rows.iter().take(64) {
                            push(if row.is_string() { row } else { &row["text"] });
                        }
                    } else {
                        push(value);
                    }
                }
            }
            CloudResponseFormat::Google => unreachable!(),
        }
    }
    if request.cache_enabled && request.cached_response.is_none() {
        if let Ok(mut cache) = CACHE.get_or_init(Mutex::default).lock() {
            if cache.len() >= super::MAX_CACHE_ENTRIES {
                cache.clear();
            }
            let ttl = if candidates.is_empty() { 60 } else { 600 };
            cache.insert(
                request.cache_key.clone(),
                (
                    candidates.clone(),
                    Instant::now() + Duration::from_secs(ttl),
                ),
            );
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commercial_baidu_uses_its_own_contract_and_rejects_other_input_modes() {
        let config = CloudCandidatePreferences {
            source: CloudCandidateSource::Baidu,
            cache_enabled: false,
            ..Default::default()
        };
        let request = build_request(&config, "ni'hao", false).unwrap();
        assert_eq!(
            request.url,
            "https://olimenew.baidu.com/py?input=ni%27hao&inputtype=py&resultcoding=utf-8"
        );
        let body = br#"{"status":"T","errno":"0","result":[[["\u4f60\u597d",5,{}],["\u4f60\u53f7",4,{}]]]}"#;
        assert_eq!(parse_responses(&request, body), ["你好", "你号"]);
        assert!(parse_responses(
            &request,
            br#"{"status":"F","errno":"1","result":[[["x"]]]}"#
        )
        .is_empty());
        assert!(build_request(&config, "konnichiha", true).is_none());
        assert!(build_english_request(&config, "hello", "").is_none());
        assert!(build_prediction_request(&config, "你好").is_none());
    }

    #[test]
    fn prediction_requires_separate_permission_and_preserves_all_cached_suffixes() {
        let mut config = custom("prediction");
        config.cache_enabled = true;
        config.method = CloudHttpMethod::Post;
        config.body_template = r#"{"query":"{query}"}"#.into();
        assert!(build_prediction_request(&config, "吃").is_none());
        config.prediction_enabled = true;
        let prediction = build_prediction_request(&config, "吃").unwrap();
        let body: Value = serde_json::from_str(&prediction.body_utf8).unwrap();
        assert_eq!(
            body,
            serde_json::json!({"query":"吃","mode":"prediction","context":"吃"})
        );
        assert_ne!(
            prediction.cache_key,
            build_request(&config, "吃", false).unwrap().cache_key
        );
        let response = serde_json::to_vec(&serde_json::json!({"candidates":[
            {"text":" 饭 "}, {"text":"饭"}, {"text":"东西"}, {"text":"坏\n词"}, {"text":""}
        ]}))
        .unwrap();
        assert_eq!(parse_responses(&prediction, &response), ["饭", "东西"]);
        let cached = build_prediction_request(&config, "吃").unwrap();
        assert_eq!(
            parse_responses(&cached, cached.cached_response.as_ref().unwrap().as_bytes()),
            ["饭", "东西"]
        );
        config.prediction_enabled = false;
        assert!(build_prediction_request(&config, "吃").is_none());
    }

    #[test]
    fn prediction_get_parameters_override_template_mode_and_encode_committed_text() {
        let mut config = custom("prediction-get");
        config.prediction_enabled = true;
        config.endpoint.push_str("&mode=pinyin&context=old");
        let request = build_prediction_request(&config, "天气").unwrap();
        let parsed = url::Url::parse(&request.url).unwrap();
        let pairs: BTreeMap<_, _> = parsed.query_pairs().collect();
        assert_eq!(pairs.get("mode").unwrap(), "prediction");
        assert_eq!(pairs.get("context").unwrap(), "天气");
        assert!(request.url.contains("%E5%A4%A9%E6%B0%94"));
    }

    fn custom(name: &str) -> CloudCandidatePreferences {
        CloudCandidatePreferences {
            source: CloudCandidateSource::Custom,
            endpoint: format!("https://{name}.example.invalid/candidates?q={{query}}"),
            cache_enabled: false,
            ..Default::default()
        }
    }

    #[test]
    fn english_online_cloud_requests_separate_mode_context_and_cache() {
        assert!(
            build_english_request(&CloudCandidatePreferences::default(), "accessability", "")
                .is_none()
        );
        for method in [CloudHttpMethod::Get, CloudHttpMethod::Post] {
            let mut config = custom("english");
            config.method = method;
            let request =
                build_english_request(&config, "accessability", "Synthetic preceding sentence.")
                    .unwrap();
            assert_ne!(
                request.cache_key,
                build_request(&config, "accessability", false)
                    .unwrap()
                    .cache_key
            );
            assert_ne!(
                request.cache_key,
                build_english_request(&config, "accessability", "Another sentence.")
                    .unwrap()
                    .cache_key
            );
            if method == CloudHttpMethod::Post {
                let body: Value = serde_json::from_str(&request.body_utf8).unwrap();
                assert_eq!(body["mode"], "english");
                assert_eq!(body["context"], "Synthetic preceding sentence.");
            } else {
                let url = url::Url::parse(&request.url).unwrap();
                assert!(url
                    .query_pairs()
                    .any(|(key, value)| key == "mode" && value == "english"));
                assert!(url.query_pairs().any(
                    |(key, value)| key == "context" && value == "Synthetic preceding sentence."
                ));
            }
        }
        assert!(build_english_request(&custom("invalid"), "word1", "").is_none());
    }

    #[test]
    fn get_encodes_only_the_current_query_and_preserves_google() {
        let config = custom("encoded");
        let request = build_request(&config, "ni hao'&?", false).unwrap();
        assert_eq!(
            request.url,
            "https://encoded.example.invalid/candidates?q=ni%20hao%27%26%3F"
        );
        assert_eq!(request.method, CloudHttpMethod::Get);
        assert!(request.body_utf8.is_empty());
        assert_eq!(request.timeout_ms, 2000);
        let google = build_request(&Default::default(), "ni hao", false).unwrap();
        assert_eq!(google.url, build_google_url("ni hao", false).unwrap());
        assert_eq!(
            parse_response(&google, br#"["SUCCESS",[["ni hao",["\u4f60\u597d"]]]]"#),
            Some("你好".into())
        );
        assert!(build_request(&config, "bad\n", false).is_none());
    }

    #[test]
    fn post_substitutes_json_string_values_without_injection() {
        let config = CloudCandidatePreferences {
            endpoint: "http://127.0.0.1:8764/v1/chat/completions".into(),
            method: CloudHttpMethod::Post,
            headers_json: r#"{"Authorization":"Bearer synthetic","content-type":"application/json"}"#.into(),
            body_template: r#"{"model":"synthetic","messages":[{"role":"user","content":"候选 {query}"}],"extra":"{query}"}"#.into(),
            timeout_ms: 200,
            ..custom("post")
        };
        let request = build_request(&config, "ni\"hao\\", false).unwrap();
        let body: Value = serde_json::from_str(&request.body_utf8).unwrap();
        assert_eq!(body["messages"][0]["content"], "候选 ni\"hao\\");
        assert_eq!(body["extra"], "ni\"hao\\");
        assert_eq!(request.headers["Authorization"], "Bearer synthetic");
        assert_eq!(request.connect_timeout_ms, 200);
        assert_eq!(
            request
                .headers
                .keys()
                .filter(|name| name.eq_ignore_ascii_case("Content-Type"))
                .count(),
            1
        );
    }

    #[test]
    fn paths_and_standard_candidates_fail_closed() {
        let mut request = build_request(&custom("response"), "nihao", false).unwrap();
        assert_eq!(
            parse_response(
                &request,
                r#"{"candidates":[{"text":" 你好 "},{"text":"拟好"}]}"#.as_bytes()
            ),
            Some("你好".into())
        );
        for body in [
            r#"{"data":["你好"]}"#,
            r#"{"candidates":{}}"#,
            r#"{"candidates":[{"text":"bad\n"}]}"#,
            r#"{"candidates":[]}"#,
        ] {
            assert!(
                parse_response(&request, body.as_bytes()).is_none(),
                "{body}"
            );
        }
        request.response_format = CloudResponseFormat::JsonPath;
        request.response_path = "data.0.text".into();
        assert_eq!(
            parse_response(&request, r#"{"data":[{"text":"合成候选"}]}"#.as_bytes()),
            Some("合成候选".into())
        );
        request.response_path = "choices.0.message.content".into();
        assert_eq!(
            parse_response(
                &request,
                r#"{"choices":[{"message":{"content":"合成回答"}}]}"#.as_bytes()
            ),
            Some("合成回答".into())
        );
        assert!(parse_response(&request, &vec![b'x'; MAX_RESPONSE + 1]).is_none());
    }

    #[test]
    fn configuration_rejects_unsafe_urls_headers_templates_and_limits() {
        let valid = custom("valid");
        assert!(valid.validate());
        for endpoint in [
            "http://public.example/api?q={query}",
            "https://u:p@example.invalid/?q={query}",
            "https://example.invalid/?q={query}#fragment",
            "file:///tmp/{query}",
            "https://127.0.0.1\\@example.invalid/?q={query}",
            "https://{query}.example.invalid/",
            "http://127.1/?q={query}",
            "https://example.invalid/api",
        ] {
            assert!(
                !CloudCandidatePreferences {
                    endpoint: endpoint.into(),
                    ..valid.clone()
                }
                .validate(),
                "{endpoint}"
            );
        }
        for headers in [
            r#"{"Authorization":"synthetic\r\nInjected: x"}"#,
            r#"{"Host":"another.invalid"}"#,
            r#"{"Cookie":"synthetic"}"#,
            r#"{"Content-Length":"20"}"#,
            r#"{"X-Test":"a","x-test":"b"}"#,
            r#"{"X-Test":42}"#,
        ] {
            assert!(
                !CloudCandidatePreferences {
                    headers_json: headers.into(),
                    ..valid.clone()
                }
                .validate(),
                "{headers}"
            );
        }
        for timeout_ms in [0, 99, 10001, u64::MAX] {
            assert!(!CloudCandidatePreferences {
                timeout_ms,
                ..valid.clone()
            }
            .validate());
        }
        for body_template in ["not json", "[]", "{}"] {
            assert!(!CloudCandidatePreferences {
                method: CloudHttpMethod::Post,
                endpoint: "https://example.invalid/api".into(),
                body_template: body_template.into(),
                ..valid.clone()
            }
            .validate());
        }
    }

    #[test]
    fn automatic_headers_fit_beside_the_full_user_header_budget() {
        let headers: BTreeMap<String, String> = (0..16)
            .map(|index| (format!("X-Synthetic-{index}"), "x".repeat(230)))
            .collect();
        let config = CloudCandidatePreferences {
            method: CloudHttpMethod::Post,
            headers_json: serde_json::to_string(&headers).unwrap(),
            ..custom("headers-budget")
        };
        let request = build_request(&config, "nihao", false).unwrap();
        assert_eq!(request.headers.len(), 18);
        assert!(request.validate());
    }

    #[test]
    fn cache_is_scoped_to_provider_credentials_language_and_query_and_can_be_disabled() {
        let mut config = custom("cache-scoping");
        config.cache_enabled = true;
        let request = build_request(&config, "nihao", false).unwrap();
        assert!(request.cached_response.is_none());
        assert_eq!(
            parse_response(&request, r#"{"candidates":[{"text":"你好"}]}"#.as_bytes()),
            Some("你好".into())
        );
        let cached = build_request(&config, "nihao", false).unwrap();
        assert_eq!(
            parse_response(&cached, cached.cached_response.as_ref().unwrap().as_bytes()),
            Some("你好".into())
        );
        assert!(build_request(&config, "nihao", true)
            .unwrap()
            .cached_response
            .is_none());
        assert!(build_request(&config, "ni", false)
            .unwrap()
            .cached_response
            .is_none());
        config.headers_json = r#"{"Authorization":"Bearer another-synthetic-token"}"#.into();
        assert!(build_request(&config, "nihao", false)
            .unwrap()
            .cached_response
            .is_none());
        let empty = build_request(&config, "empty", false).unwrap();
        assert!(parse_response(&empty, b"{\"candidates\":[]}").is_none());
        assert!(build_request(&config, "empty", false)
            .unwrap()
            .cached_response
            .is_some());
        config.cache_enabled = false;
        assert!(build_request(&config, "empty", false)
            .unwrap()
            .cached_response
            .is_none());
        let preferences = crate::preferences::Preferences {
            cloud_candidate_provider: config,
            ..Default::default()
        };
        assert!(!preferences
            .redacted_for_diagnostics()
            .to_string()
            .contains("another-synthetic-token"));
        let exported = crate::settings_document::export_document(&preferences);
        assert!(!format!("{exported:?}").contains("another-synthetic-token"));
    }
}
