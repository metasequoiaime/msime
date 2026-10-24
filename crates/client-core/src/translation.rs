//! 翻译服务校验、释义格式与候选显示分段的共享规则。

use hmac::{Hmac, KeyInit, Mac};
use md5::Md5;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;

mod display;
pub use display::{translation_runs, TranslationRun, TranslationRunRole};

/// 候选可能是完整句子；按 Unicode 标量值限制长度，确保各宿主在发送私密文本前使用同一上限。
pub const MAX_SOURCE_CHARS: usize = 256;
const MAX_PERSIST_GLOSS_CHARS: usize = 32;

/// Tencent TC3 signing primitive. The caller owns credential lifetime.
pub fn tencent_tc3_derive(secret_key: &str, date: &str, service: &str, message: &str) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let sign = |key: &[u8], data: &str| -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts arbitrary keys");
        mac.update(data.as_bytes());
        mac.finalize().into_bytes().to_vec()
    };
    let date_key = sign(format!("TC3{secret_key}").as_bytes(), date);
    let service_key = sign(&date_key, service);
    let signing_key = sign(&service_key, "tc3_request");
    hex::encode(sign(&signing_key, message))
}

pub fn tencent_tc3_canonical_request(payload_sha256: &str) -> String {
    format!("POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:tmt.tencentcloudapi.com\nx-tc-action:texttranslatebatch\n\ncontent-type;host;x-tc-action\n{payload_sha256}")
}

pub fn tencent_tc3_sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

pub fn tencent_tc3_authorization(
    secret_id: &str,
    secret_key: &str,
    timestamp: i64,
    date: &str,
    payload: &[u8],
) -> String {
    if secret_id.is_empty() || secret_key.is_empty() || date.is_empty() {
        return String::new();
    }
    let canonical = tencent_tc3_canonical_request(&tencent_tc3_sha256_hex(payload));
    let scope = format!("{date}/tmt/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{scope}\n{}",
        tencent_tc3_sha256_hex(canonical.as_bytes())
    );
    let signature = tencent_tc3_derive(secret_key, date, "tmt", &string_to_sign);
    format!("TC3-HMAC-SHA256 Credential={secret_id}/{scope}, SignedHeaders=content-type;host;x-tc-action, Signature={signature}")
}

pub fn tencent_tmt_headers(
    region: &str,
    timestamp: i64,
    authorization: &str,
) -> Vec<(String, String)> {
    vec![
        (
            "Content-Type".into(),
            "application/json; charset=utf-8".into(),
        ),
        ("Host".into(), "tmt.tencentcloudapi.com".into()),
        ("X-TC-Action".into(), "TextTranslateBatch".into()),
        ("X-TC-Timestamp".into(), timestamp.to_string()),
        ("X-TC-Version".into(), "2018-03-21".into()),
        (
            "X-TC-Region".into(),
            if region.is_empty() {
                "ap-guangzhou"
            } else {
                region
            }
            .into(),
        ),
        ("Authorization".into(), authorization.into()),
    ]
}

pub fn tencent_tmt_payload(source: &str, target: &str, texts: &[String]) -> Option<String> {
    if source.is_empty()
        || target.is_empty()
        || texts.is_empty()
        || texts.len() > 50
        || texts.iter().any(|text| !is_valid_source_text(text))
    {
        return None;
    }
    Some(
        serde_json::json!({
            "Source": source,
            "Target": target,
            "ProjectId": 0,
            "SourceTextList": texts,
        })
        .to_string(),
    )
}

pub fn parse_tencent_tmt_response(response: &str, expected: usize) -> Option<Vec<String>> {
    let root: Value = serde_json::from_str(response).ok()?;
    let values = root.get("Response")?.get("TargetTextList")?.as_array()?;
    if values.len() != expected || values.iter().any(|value| value.as_str().is_none()) {
        return None;
    }
    let mut translations = Vec::with_capacity(expected);
    translations.extend(
        values
            .iter()
            .map(|value| value.as_str().unwrap().to_owned()),
    );
    Some(translations)
}

/// Source text accepted by every cloud translation provider.
pub fn is_valid_source_text(value: &str) -> bool {
    !value.is_empty() && crate::text::is_bounded_chars(value, MAX_SOURCE_CHARS)
}

pub fn is_supported_translation_language(value: &str) -> bool {
    matches!(value, "zh" | "en" | "fr" | "ja" | "es" | "ru" | "de" | "ko")
}

pub fn is_supported_translation_pair(source: &str, target: &str) -> bool {
    is_supported_translation_language(source) && is_supported_translation_language(target)
}

pub fn format_translation_gloss(text: &str) -> Option<String> {
    let mut output = String::with_capacity(text.len());
    let mut pending_space = false;
    for ch in text.chars() {
        if matches!(ch, ' ' | '\t' | '\r' | '\n') {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        if ch.is_control() {
            return None;
        }
        output.push(ch);
    }
    (!output.is_empty()).then_some(output)
}

pub fn should_persist_translation(key: &str, gloss: &str) -> bool {
    !key.is_empty()
        && !gloss.is_empty()
        && gloss.chars().count() <= MAX_PERSIST_GLOSS_CHARS
        && !gloss.eq_ignore_ascii_case(key)
}

pub fn usable_credential(value: &str) -> bool {
    let trimmed = value.trim_matches([' ', '\t', '\r', '\n']);
    !trimmed.is_empty()
        && !(trimmed.starts_with('<') && trimmed.ends_with('>'))
        && !trimmed.starts_with("FAKESECRET_")
}

pub fn is_valid_credential(value: &str) -> bool {
    usable_credential(value) && crate::text::is_bounded_text(value, 4096)
}

/// NiuTrans v2 authStr is the lower-case MD5 of the lexicographically sorted
/// request parameters plus the API key. The provider signs raw UTF-8 values;
/// URL encoding is only applied to the eventual form body by the host.
pub fn niutrans_auth_string(
    app_id: &str,
    apikey: &str,
    from: &str,
    to: &str,
    timestamp: &str,
    source_text: &str,
) -> String {
    let canonical = format!(
        "apikey={apikey}&appId={app_id}&from={from}&srcText={source_text}&timestamp={timestamp}&to={to}"
    );
    let digest = Md5::digest(canonical.as_bytes());
    hex::encode(digest)
}

/// 不管文字是什么都不请求释义的候选来源。Engine `CandidateSource::Emoji`（6）和 `CandidateSource::Kaomoji`（7）照参考实现的 `BuildTranslationQuery`：颜文字如 "(*Φ皿Φ*)" 带汉字，按文字判断会当成中文，把一张图送给释义模型，只会在它下面放噪音。`CandidateSource::MixedJapanese`（13）是全拼、双拼里混进来的日文转换，私、開始 这样的汉字是日文，当中文释义会在日文下面写出中文意思，还白白花掉账号的释义额度；Engine 也不为日文方案和临时日文出释义。`CandidateSource::Plugin`（14）是脚本插件在 `/` 指令里给的行：插件的文字不交给任何释义服务。`CandidateSource::CloudPending`（15）只是云请求的占位行，也不应发起释义请求。连续多语言行（18）含有不同语言的片段，不将整行作为中文请求释义。
pub fn source_gets_no_gloss(source: u8) -> bool {
    matches!(source, 6 | 7 | 13 | 14 | 15 | 16 | 18)
}

pub fn is_cloud_translatable_english(text: &str) -> bool {
    let mut has_letter = false;
    for ch in text.chars() {
        if ch.is_ascii_alphabetic() {
            has_letter = true;
        } else if !matches!(ch, ' ' | '-' | '\'') {
            return false;
        }
    }
    has_letter
}

/// Mirrors the reference `IsCloudTranslatableChinese`: any Han character makes the text translatable and only an emoji or pictograph rejects it, so mixed words such as "T恤" or "3D打印" are sent while digits, Latin letters and punctuation alone are not.
pub fn is_cloud_translatable_chinese(text: &str) -> bool {
    let mut has_han = false;
    for ch in text.chars() {
        if matches!(ch, '\u{200D}' | '\u{FE0E}' | '\u{FE0F}' | '\u{20E3}')
            || ('\u{2600}'..='\u{27BF}').contains(&ch)
            || ('\u{1F000}'..='\u{1FAFF}').contains(&ch)
            || ('\u{1F1E6}'..='\u{1F1FF}').contains(&ch)
        {
            return false;
        }
        has_han = has_han
            || ('\u{3400}'..='\u{4DBF}').contains(&ch)
            || ('\u{4E00}'..='\u{9FFF}').contains(&ch)
            || ('\u{F900}'..='\u{FAFF}').contains(&ch)
            || ('\u{20000}'..='\u{2CEAF}').contains(&ch)
            || ch == '\u{3007}';
    }
    has_han
}

/// Credentials and private input may only travel over TLS or to a local service.
pub fn is_secure_endpoint(endpoint: &str) -> bool {
    if !crate::text::is_bounded_text(endpoint, 2048) {
        return false;
    }
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    match url.scheme() {
        "https" => crate::endpoint::valid_endpoint_shape(endpoint, &url, "https"),
        "http" => {
            url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && matches!(
                    url.host_str(),
                    Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
                )
        }
        _ => false,
    }
}

pub fn is_supported_endpoint(endpoint: &str) -> bool {
    is_secure_endpoint(endpoint)
}

/// 设置页在自定义翻译 Endpoint 下面显示的问题，没有问题时为空。逐条照搬共享设置的 `translationEndpointIssue`（`packages/ui/src/settings/translation-validation.ts`），网址按同一套 WHATWG 规则解析，所以与那里的 `new URL` 判断一致；它只决定提示什么，能不能保存仍由 [`is_supported_endpoint`] 决定。原生 Windows 设置窗口通过 `msime_client_translation_endpoint_issue` 调用它。
pub fn endpoint_issue(endpoint: &str) -> &'static str {
    if endpoint.is_empty() {
        return "请填写完整的接口地址。";
    }
    if endpoint.len() > 2048 {
        return "接口地址过长。";
    }
    if endpoint
        .chars()
        .any(|character| character <= '\u{1f}' || character == '\u{7f}')
    {
        return "接口地址不能包含控制字符。";
    }
    let Ok(url) = url::Url::parse(endpoint) else {
        return "请填写以 http:// 或 https:// 开头的完整接口地址。";
    };
    // `new URL` 的 username、password、hash 和 hostname 为空字符串时不算有。
    if !url.username().is_empty()
        || url.password().is_some_and(|password| !password.is_empty())
        || url.fragment().is_some_and(|fragment| !fragment.is_empty())
        || url.host_str().is_none_or(str::is_empty)
    {
        return "接口地址不能包含凭据或片段。";
    }
    match url.scheme() {
        "https" => "",
        "http" if matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")) => "",
        "http" => "远程翻译服务必须使用 HTTPS；HTTP 仅支持本机回环地址。",
        _ => "请填写以 http:// 或 https:// 开头的完整接口地址。",
    }
}

/// DeepL 官方 API 的两个主机：Pro 密钥走 `api.deepl.com`，Free 密钥走 `api-free.deepl.com`。
const DEEPL_PRO_HOST: &str = "api.deepl.com";
const DEEPL_FREE_HOST: &str = "api-free.deepl.com";

/// 自定义翻译服务的一次请求：地址、请求头和 JSON 正文。带着凭据，不得写进日志。
#[derive(Debug, Clone, PartialEq)]
pub struct CustomTranslationRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Value,
}

/// 地址是否指向 DeepL 官方 API：https、默认端口，主机名恰好是 `api.deepl.com` 或 `api-free.deepl.com`。只看主机名，不看路径：自建 DeepLX 也提供 `/v2/translate`，按路径识别会改掉这些用户现有的请求格式。
pub fn is_deepl_api_endpoint(endpoint: &str) -> bool {
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    // `Url` 会把写明的 443 当作默认端口去掉，所以 `port()` 为空即默认端口。
    url.scheme() == "https"
        && url.port().is_none()
        && matches!(url.host_str(), Some(DEEPL_PRO_HOST | DEEPL_FREE_HOST))
}

/// DeepL 的 `target_lang`。中文和英文必须写变体：`ZH`、`EN` 只为兼容旧调用保留，文档要求改用 `ZH-HANS`/`ZH-HANT`、`EN-US`/`EN-GB`。候选释义一律简体中文，英文取美式拼写。其余语言（FR、JA、ES、RU、DE、KO）直接取大写代码。
fn deepl_target_language(target: &str) -> String {
    match target.to_ascii_lowercase().as_str() {
        "zh" => "ZH-HANS".into(),
        "en" => "EN-US".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// 构造自定义翻译服务的请求，运行时描述器和设置页的连接测试共用这一处。调用方负责先校验地址、密钥和文本；`endpoint`、`api_key` 应已去掉首尾空白。
///
/// - DeepL 官方 API（见 [`is_deepl_api_endpoint`]）：`Authorization: DeepL-Auth-Key <key>`，正文 `{"text":[文本],"source_lang","target_lang"}`，发往 `https://<主机>/v2/translate`。主机按密钥选：以 `:fx` 结尾的是 Free 密钥，走 `api-free.deepl.com`，否则走 `api.deepl.com`，与 DeepL 官方客户端库的判断相同；用户把 Free 密钥填到 Pro 地址（或反过来）时 DeepL 会回 403，这里直接改到对的主机。DeepL 没有密钥就不能用，空密钥返回 `None`。
/// - 其他地址按 DeepLX 兼容格式：有密钥时 `Authorization: Bearer <key>`，正文 `{"text":文本,"source_lang","target_lang"}`，地址原样使用。
///
/// 源语言照常写明，不交给 DeepL 自动识别：候选词往往只有两三个字，「手紙」这样的词自动识别会当成日语，而规划阶段已经按文字判断了方向。
pub fn custom_translation_request(
    endpoint: &str,
    api_key: &str,
    text: &str,
    source_language: &str,
    target_language: &str,
) -> Option<CustomTranslationRequest> {
    let mut headers = vec![("Content-Type".to_owned(), "application/json".to_owned())];
    if is_deepl_api_endpoint(endpoint) {
        if api_key.is_empty() {
            return None;
        }
        let host = if api_key.ends_with(":fx") {
            DEEPL_FREE_HOST
        } else {
            DEEPL_PRO_HOST
        };
        headers.push((
            "Authorization".to_owned(),
            format!("DeepL-Auth-Key {api_key}"),
        ));
        return Some(CustomTranslationRequest {
            url: format!("https://{host}/v2/translate"),
            headers,
            body: serde_json::json!({
                "text": [text],
                "source_lang": source_language.to_ascii_uppercase(),
                "target_lang": deepl_target_language(target_language),
            }),
        });
    }
    if !api_key.is_empty() {
        headers.push(("Authorization".to_owned(), format!("Bearer {api_key}")));
    }
    Some(CustomTranslationRequest {
        url: endpoint.to_owned(),
        headers,
        body: serde_json::json!({
            "text": text,
            "source_lang": source_language.to_ascii_uppercase(),
            "target_lang": target_language.to_ascii_uppercase(),
        }),
    })
}

/// 区分 DeepLX 兼容回复中的失败与空回答。不论有无 `code`，回复都必须带支持的结果结构，显式 `error` 优先表示失败；合法空结果仍算已回答，供宿主负缓存。
pub fn translation_response_failed(response: &str) -> bool {
    let Ok(root) = serde_json::from_str::<Value>(response) else {
        return true;
    };
    let Some(object) = root.as_object() else {
        return true;
    };
    if has_explicit_error(&root) {
        return true;
    }
    if let Some(code) = object.get("code") {
        if code.as_i64() != Some(200) && code.as_str() != Some("200") {
            return true;
        }
    }
    !["data", "translation", "result"]
        .iter()
        .any(|key| object.get(*key).is_some_and(has_supported_result_shape))
        && !object
            .get("translations")
            .and_then(Value::as_array)
            .is_some_and(|items| items.first().is_none_or(is_result_item_shape))
}

pub fn parse_translation_response(response: &str) -> Option<String> {
    let root: Value = serde_json::from_str(response).ok()?;
    if has_explicit_error(&root) {
        return None;
    }
    if let Some(code) = root.get("code") {
        let valid = code.as_i64() == Some(200) || code.as_str() == Some("200");
        if !valid {
            return None;
        }
    }
    for key in ["data", "translation", "result"] {
        if let Some(value) = root.get(key) {
            if let Some(text) = value_as_text(value) {
                return Some(text);
            }
            if let Some(first) = value.as_array().and_then(|items| items.first()) {
                if let Some(text) = value_as_text(first) {
                    return Some(text);
                }
            }
        }
    }
    root.get("translations")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(value_as_text)
}

fn has_explicit_error(root: &Value) -> bool {
    root.get("error").is_some_and(|error| match error {
        Value::Null | Value::Bool(false) => false,
        Value::String(message) => !message.is_empty(),
        _ => true,
    })
}

fn has_supported_result_shape(value: &Value) -> bool {
    is_result_item_shape(value)
        || value
            .as_array()
            .is_some_and(|items| items.first().is_none_or(is_result_item_shape))
}

fn is_result_item_shape(value: &Value) -> bool {
    match value {
        Value::String(_) => true,
        Value::Object(object) => ["text", "translation", "data"]
            .iter()
            .any(|key| object.get(*key).is_some_and(Value::is_string)),
        _ => false,
    }
}

fn value_as_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            value.as_object().and_then(|object| {
                ["text", "translation", "data"].iter().find_map(|key| {
                    object
                        .get(*key)
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned)
                })
            })
        })
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emoji_kaomoji_and_mixed_japanese_sources_get_no_gloss() {
        for source in [6, 7, 13, 14, 15, 16] {
            assert!(source_gets_no_gloss(source), "source {source}");
        }
        // 词库、生成、英文、云候选照文字判断。
        for source in [0, 1, 2, 3, 4, 5, 8, 9, 10, 11, 12] {
            assert!(!source_gets_no_gloss(source), "source {source}");
        }
    }

    #[test]
    fn accepts_supported_endpoints_only() {
        assert!(is_supported_endpoint("https://translate.example/api"));
        assert!(is_supported_endpoint("http://localhost:8080/translate"));
        assert!(is_supported_endpoint("http://127.0.0.1:8080/translate"));
        assert!(is_supported_endpoint("http://[::1]:8080/translate"));
        assert!(!is_supported_endpoint("http://translate.example/api"));
        assert!(!is_supported_endpoint("http://localhost.example/api"));
        assert!(!is_supported_endpoint("http://127.0.0.2/api"));
        assert!(!is_supported_endpoint("http://user:secret@localhost/api"));
        assert!(!is_supported_endpoint("http://localhost/api#fragment"));
        assert!(!is_supported_endpoint("ftp://translate.example"));
        assert!(!is_supported_endpoint("https://bad\n.example"));
        assert!(!is_supported_endpoint("https:///api"));
    }

    #[test]
    fn endpoint_issue_matches_the_settings_page() {
        assert_eq!(endpoint_issue(""), "请填写完整的接口地址。");
        assert_eq!(
            endpoint_issue(&format!("https://a.example/{}", "x".repeat(2048))),
            "接口地址过长。"
        );
        assert_eq!(
            endpoint_issue("https://a.example/\u{7f}"),
            "接口地址不能包含控制字符。"
        );
        for invalid in [
            "translate.example",
            "https://",
            "ftp://translate.example/api",
        ] {
            assert_eq!(
                endpoint_issue(invalid),
                "请填写以 http:// 或 https:// 开头的完整接口地址。",
                "{invalid}"
            );
        }
        for leaky in [
            "https://user@translate.example/api",
            "https://user:secret@translate.example/api",
            "https://translate.example/api#part",
            "mailto:someone@example.com",
        ] {
            assert_eq!(
                endpoint_issue(leaky),
                "接口地址不能包含凭据或片段。",
                "{leaky}"
            );
        }
        for fine in [
            "https://translate.example/api",
            "https://translate.example/api#",
            "http://localhost:1188/translate",
            "http://127.0.0.1/translate",
            "http://[::1]:1188/translate",
            "HTTP://LOCALHOST/translate",
        ] {
            assert_eq!(endpoint_issue(fine), "", "{fine}");
        }
        assert_eq!(
            endpoint_issue("http://translate.example/api"),
            "远程翻译服务必须使用 HTTPS；HTTP 仅支持本机回环地址。"
        );
    }

    #[test]
    fn parses_supported_response_shapes() {
        assert_eq!(
            parse_translation_response(r#"{"data":"hello"}"#).as_deref(),
            Some("hello")
        );
        assert_eq!(
            parse_translation_response(r#"{"translation":{"text":"hello"}}"#).as_deref(),
            Some("hello")
        );
        assert_eq!(
            parse_translation_response(r#"{"translations":[{"translation":"hello"}]}"#).as_deref(),
            Some("hello")
        );
        assert!(parse_translation_response(r#"{"code":500,"data":"nope"}"#).is_none());
    }

    #[test]
    fn empty_response_fields_do_not_hide_later_translations() {
        for response in [
            r#"{"data":"","translation":"synthetic"}"#,
            r#"{"data":{"text":""},"result":"synthetic"}"#,
            r#"{"data":[""],"translation":{"text":"synthetic"}}"#,
            r#"{"data":[{"text":""}],"translations":[{"text":"synthetic"}]}"#,
            r#"{"data":"","translation":"","result":"","translations":["synthetic"]}"#,
        ] {
            assert_eq!(
                parse_translation_response(response).as_deref(),
                Some("synthetic")
            );
        }
        for response in [
            r#"{"data":""}"#,
            r#"{"translations":[{"text":""}]}"#,
            r#"{"data":["","synthetic"]}"#,
            r#"{"code":500,"data":"","translation":"synthetic"}"#,
        ] {
            assert!(parse_translation_response(response).is_none());
        }
        assert_eq!(
            parse_translation_response(r#"{"data":"first","translation":"second"}"#).as_deref(),
            Some("first")
        );
    }

    #[test]
    fn translation_object_fallback_skips_empty_text_aliases() {
        for value in [
            serde_json::json!({"text":"", "translation":"synthetic"}),
            serde_json::json!({"text":"", "data":"synthetic"}),
            serde_json::json!({"translation":"", "data":"synthetic"}),
            serde_json::json!({"text":"", "translation":"", "data":"synthetic"}),
        ] {
            for mut document in [
                serde_json::json!({"data":value}),
                serde_json::json!({"translation":value}),
                serde_json::json!({"result":[value]}),
                serde_json::json!({"translations":[value]}),
            ] {
                for code in [
                    Value::Null,
                    serde_json::json!(200),
                    serde_json::json!("200"),
                ] {
                    if !code.is_null() {
                        document["code"] = code;
                    }
                    let response = document.to_string();
                    assert!(!translation_response_failed(&response), "{response}");
                    assert_eq!(
                        parse_translation_response(&response).as_deref(),
                        Some("synthetic"),
                        "{response}"
                    );
                }
            }
        }
    }

    #[test]
    fn translation_object_fallback_preserves_priority_and_empty_answers() {
        for (response, expected) in [
            (
                r#"{"data":{"text":"first","translation":"second","data":"third"}}"#,
                Some("first"),
            ),
            (
                r#"{"data":{"text":"","translation":"second","data":"third"}}"#,
                Some("second"),
            ),
            (r#"{"data":{"text":"","translation":"","data":""}}"#, None),
            (r#"{"data":[{"text":""},{"translation":"second"}]}"#, None),
            (
                r#"{"error":"synthetic failure","data":{"text":"","translation":"second"}}"#,
                None,
            ),
            (
                r#"{"code":500,"data":{"text":"","translation":"second"}}"#,
                None,
            ),
        ] {
            assert_eq!(
                parse_translation_response(response).as_deref(),
                expected,
                "{response}"
            );
        }
    }

    #[test]
    fn tencent_tc3_signature_is_deterministic_and_message_bound() {
        let first = tencent_tc3_derive("secret", "20240101", "tmt", "request");
        assert_eq!(
            first,
            tencent_tc3_derive("secret", "20240101", "tmt", "request")
        );
        assert_eq!(first.len(), 64);
        assert_ne!(
            first,
            tencent_tc3_derive("secret", "20240101", "tmt", "other")
        );
        assert_ne!(
            first,
            tencent_tc3_derive("different", "20240101", "tmt", "request")
        );
    }

    /// Known answers, computed independently from Tencent's published TC3-HMAC-SHA256
    /// construction rather than from this code.
    ///
    /// The determinism test above passes for any implementation that is stable and
    /// input-sensitive, including a wrong one - it compares this code against itself.
    /// These values come from the documented chain
    /// `HMAC(HMAC(HMAC("TC3"+key, date), service), "tc3_request")`, so a signature that
    /// drifts from the protocol fails here even while staying perfectly deterministic.
    #[test]
    fn tencent_tc3_matches_independently_computed_signatures() {
        assert_eq!(
            tencent_tc3_derive("secret", "20240101", "tmt", "request"),
            "601335ccd32d88ea0d9678b52335a23bd44d4ff79a5d72a60c5c1b9b7b2d317a"
        );
        let payload = br#"{"SourceTextList":["hello"],"Source":"en","Target":"zh","ProjectId":0}"#;
        assert_eq!(
            tencent_tc3_sha256_hex(payload),
            "66a3bcf3c82aee0cd2efbee35fc172ad45dd4b2aab2b7e304e5a7f03ae54e5c6"
        );
        // The whole header, so the credential scope and signed-header list are pinned
        // alongside the signature: a request is rejected for any of the three.
        assert_eq!(
            tencent_tc3_authorization("AKIDEXAMPLE", "secret", 1704067200, "20240101", payload),
            "TC3-HMAC-SHA256 Credential=AKIDEXAMPLE/20240101/tmt/tc3_request, \
SignedHeaders=content-type;host;x-tc-action, \
Signature=fdaffffbe1460ecd8cbc30e296ff6f49cc3b4af10b11e099462cca023fdb2c6c"
        );
    }

    #[test]
    fn tencent_tc3_canonical_request_matches_protocol_layout() {
        assert_eq!(
            tencent_tc3_canonical_request("payload-hash"),
            "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:tmt.tencentcloudapi.com\nx-tc-action:texttranslatebatch\n\ncontent-type;host;x-tc-action\npayload-hash"
        );
    }

    #[test]
    fn tencent_tmt_payload_matches_batch_contract() {
        let payload = tencent_tmt_payload("zh", "en", &["你好".into()]).unwrap();
        let value: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(value["Source"], "zh");
        assert_eq!(value["Target"], "en");
        assert_eq!(value["ProjectId"], 0);
        assert_eq!(value["SourceTextList"][0], "你好");
        assert!(tencent_tmt_payload("zh", "en", &["字".repeat(257)]).is_none());
        assert!(tencent_tmt_payload("zh", "en", &["字".repeat(256)]).is_some());
        assert!(tencent_tmt_payload("zh", "en", &[String::new()]).is_none());
        for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
            let control = char::from_u32(codepoint).unwrap();
            assert!(tencent_tmt_payload("zh", "en", &[format!("before{control}after")]).is_none());
        }
    }

    #[test]
    fn parses_tencent_tmt_response_with_exact_batch_size() {
        assert_eq!(
            parse_tencent_tmt_response(r#"{"Response":{"TargetTextList":["a","b"]}}"#, 2),
            Some(vec!["a".into(), "b".into()])
        );
        assert!(
            parse_tencent_tmt_response(r#"{"Response":{"TargetTextList":["a"]}}"#, 2).is_none()
        );
    }

    #[test]
    fn formats_translation_gloss_like_windows_provider() {
        assert_eq!(
            format_translation_gloss("  hello\tworld\n"),
            Some("hello world".into())
        );
        assert_eq!(format_translation_gloss("\u{0000}"), None);
    }

    #[test]
    fn formats_multiline_translation_gloss_onto_one_line() {
        // A host shows one row per target language, so a newline kept here would push the next target's gloss onto the wrong row.
        assert_eq!(
            format_translation_gloss("test\nfoo"),
            Some("test foo".into())
        );
        assert_eq!(
            format_translation_gloss("a\r\n\r\nb\n\tc"),
            Some("a b c".into())
        );
    }

    #[test]
    fn formats_translation_gloss_without_trailing_whitespace() {
        assert_eq!(
            format_translation_gloss("test foo "),
            Some("test foo".into())
        );
        assert_eq!(format_translation_gloss("test\n"), Some("test".into()));
        assert_eq!(format_translation_gloss(" \n\t"), None);
    }

    #[test]
    fn niutrans_auth_string_is_sorted_md5_and_credentials_filter_placeholders() {
        assert_eq!(
            niutrans_auth_string("app-id", "api-key", "en", "zh", "1704067200000", "hello"),
            "6da3515e010ef871b66e4e31ff5ba580"
        );
        assert!(usable_credential("real-value"));
        assert!(!usable_credential("<YOUR_NIUTRANS_APP_ID>"));
        assert!(!usable_credential("FAKESECRET_test"));
        assert!(!usable_credential(" \n\t"));
    }

    #[test]
    fn an_empty_answer_is_not_a_failed_reply() {
        assert!(!translation_response_failed(r#"{"code":200,"data":""}"#));
        assert!(!translation_response_failed(r#"{"data":"hello"}"#));
        assert!(translation_response_failed(
            r#"{"code":429,"message":"rate limited"}"#
        ));
        assert!(translation_response_failed(r#"{"code":"500"}"#));
        assert!(translation_response_failed("<html>Bad Gateway</html>"));
        assert!(translation_response_failed(r#"["data"]"#));
    }

    #[test]
    fn an_error_envelope_without_a_success_code_is_not_an_empty_answer() {
        for response in [
            r#"{"error":"rate limited"}"#,
            r#"{"error":"rate limited","data":""}"#,
            r#"{"message":"service unavailable"}"#,
            "{}",
        ] {
            assert!(translation_response_failed(response), "{response}");
        }
        assert!(!translation_response_failed(r#"{"data":""}"#));
        assert!(!translation_response_failed(r#"{"error":null,"data":""}"#));
    }

    #[test]
    fn an_error_envelope_does_not_expose_a_partial_translation() {
        let response = r#"{"error":"rate limited","data":"stale synthetic gloss"}"#;
        assert!(translation_response_failed(response));
        assert!(parse_translation_response(response).is_none());
    }

    #[test]
    fn unsupported_result_shapes_do_not_count_as_empty_answers() {
        for response in [
            "{}",
            r#"{"message":"temporarily unavailable"}"#,
            r#"{"data":null}"#,
            r#"{"translation":42}"#,
            r#"{"result":false}"#,
            r#"{"translations":"synthetic"}"#,
            r#"{"data":{}}"#,
            r#"{"data":[null]}"#,
        ] {
            for code in [
                None,
                Some(serde_json::json!(200)),
                Some(serde_json::json!("200")),
            ] {
                let mut document: Value = serde_json::from_str(response).unwrap();
                if let Some(code) = code {
                    document["code"] = code;
                }
                let document = document.to_string();
                assert!(translation_response_failed(&document), "{document}");
            }
        }
        for response in [
            r#"{"data":""}"#,
            r#"{"translations":[]}"#,
            r#"{"translation":{"text":""}}"#,
        ] {
            for code in [
                None,
                Some(serde_json::json!(200)),
                Some(serde_json::json!("200")),
            ] {
                let mut document: Value = serde_json::from_str(response).unwrap();
                if let Some(code) = code {
                    document["code"] = code;
                }
                let document = document.to_string();
                assert!(!translation_response_failed(&document), "{document}");
            }
        }
    }

    /// 与 Linux provider 共用的用例：两边对同一组地址和密钥必须构造出相同的请求。
    #[test]
    fn custom_translation_request_matches_the_shared_contract() {
        let contract: Value = serde_json::from_str(include_str!(
            "../../../shared/contracts/custom-translation/cases.json"
        ))
        .unwrap();
        let cases = contract["cases"].as_array().unwrap();
        assert!(cases.len() >= 10);
        for case in cases {
            let text = |key: &str| case[key].as_str().unwrap();
            let request = custom_translation_request(
                text("endpoint"),
                text("api_key"),
                text("text"),
                text("source_language"),
                text("target_language"),
            );
            let actual = request.map(|request| {
                let headers: serde_json::Map<String, Value> = request
                    .headers
                    .into_iter()
                    .map(|(name, value)| (name, Value::String(value)))
                    .collect();
                serde_json::json!({"url": request.url, "headers": headers, "body": request.body})
            });
            assert_eq!(
                actual.unwrap_or(Value::Null),
                case["request"],
                "{}",
                text("name")
            );
        }
    }

    #[test]
    fn deepl_endpoint_detection_matches_the_exact_official_hosts() {
        for endpoint in [
            "https://api.deepl.com/v2/translate",
            "https://api-free.deepl.com/v2/translate",
            "https://api-free.deepl.com",
            "https://Api-Free.DeepL.com/v2/translate",
        ] {
            assert!(is_deepl_api_endpoint(endpoint), "{endpoint}");
        }
        for endpoint in [
            "http://api.deepl.com/v2/translate",
            "https://api.deepl.com.example/v2/translate",
            "https://example.com/api.deepl.com/v2/translate",
            "https://deepl.com/v2/translate",
            "https://www.deepl.com/v2/translate",
            "https://api.deepl.com:8443/v2/translate",
            "https://deeplx.example/v2/translate",
            "not a url",
        ] {
            assert!(!is_deepl_api_endpoint(endpoint), "{endpoint}");
        }
    }

    /// DeepL v2 的成功回复带 `detected_source_language`，译文在 `translations[0].text`。
    #[test]
    fn parses_a_deepl_v2_reply() {
        let reply = r#"{"translations":[{"detected_source_language":"ZH","text":"Test"}]}"#;
        assert!(!translation_response_failed(reply));
        assert_eq!(parse_translation_response(reply).as_deref(), Some("Test"));
        let empty = r#"{"translations":[{"detected_source_language":"ZH","text":""}]}"#;
        assert!(!translation_response_failed(empty));
        assert!(parse_translation_response(empty).is_none());
    }

    /// DeepL 的错误（403 密钥无效、456 额度用完、429 过于频繁）走非 2xx 状态，宿主不会把正文交给解析；正文本身也只有 `message`，即使被交过来也只能算失败，不能当成空译文负缓存。
    #[test]
    fn a_deepl_error_body_is_a_failed_reply() {
        for reply in [
            r#"{"message":"Forbidden"}"#,
            r#"{"message":"Quota Exceeded"}"#,
            r#"{"message":"Too many requests"}"#,
            r#"{"message":"Wrong endpoint. Use https://api.deepl.com"}"#,
            "",
        ] {
            assert!(translation_response_failed(reply), "{reply}");
            assert!(parse_translation_response(reply).is_none(), "{reply}");
        }
    }

    #[test]
    fn persistence_requires_short_changed_gloss() {
        assert!(should_persist_translation("hello", "你好"));
        assert!(!should_persist_translation("hello", "hello"));
        assert!(!should_persist_translation("hello", &"字".repeat(33)));
    }

    #[test]
    fn rejects_placeholder_tencent_secrets() {
        assert!(usable_credential(" real-secret "));
        assert!(!usable_credential("<YOUR_TENCENT_SECRET_ID>"));
        assert!(!usable_credential("FAKESECRET_test"));
        assert!(!usable_credential(" \n\t"));
    }

    #[test]
    fn filters_cloud_translation_candidates_by_script() {
        assert!(is_cloud_translatable_english("hello-world"));
        assert!(!is_cloud_translatable_english("123"));
        assert!(is_cloud_translatable_chinese("你好"));
        assert!(!is_cloud_translatable_chinese("你好😀"));
        assert!(!is_cloud_translatable_chinese("你好☀️"));
        assert!(!is_cloud_translatable_chinese("你‍好"));
        assert!(!is_cloud_translatable_chinese("你⃣"));
        assert!(is_cloud_translatable_chinese("T恤"));
        assert!(is_cloud_translatable_chinese("3D打印"));
        assert!(is_cloud_translatable_chinese("\u{F900}"));
        assert!(is_cloud_translatable_chinese("\u{20000}"));
        assert!(!is_cloud_translatable_chinese("T"));
        assert!(!is_cloud_translatable_chinese("123"));
        assert!(!is_cloud_translatable_chinese(""));
    }
}
