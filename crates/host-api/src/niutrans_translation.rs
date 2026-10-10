//! Pure descriptors and response parsing for the host-owned NiuTrans v2 API.
use msime_client_core::cloud::dictionary::percent_encode;
use msime_client_core::translation;
use serde::Deserialize;
use serde_json::{json, Value};

const URL: &str = "https://api.niutrans.com/v2/text/translate";

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Config {
    enabled: bool,
    app_id: String,
    apikey: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    config: Config,
    text: String,
    source_language: String,
    target_language: String,
    timestamp: String,
}

fn valid_credential(value: &str) -> bool {
    translation::is_valid_credential(value)
}

pub fn descriptor(bytes: &[u8]) -> Result<Value, &'static str> {
    let request: Request = serde_json::from_slice(bytes).map_err(|_| "invalid NiuTrans request")?;
    if !request.config.enabled {
        return Ok(Value::Null);
    }
    let app_id = request.config.app_id.trim_matches([' ', '\t', '\r', '\n']);
    let apikey = request.config.apikey.trim_matches([' ', '\t', '\r', '\n']);
    let source = request.source_language.as_str();
    let target = request.target_language.as_str();
    if !valid_credential(app_id)
        || !valid_credential(apikey)
        || !translation::is_valid_source_text(&request.text)
        || request.timestamp.is_empty()
        || request.timestamp.len() > 20
        || !msime_client_core::is_ascii_digits(&request.timestamp)
        || !translation::is_supported_translation_pair(source, target)
    {
        return Err("invalid NiuTrans parameters");
    }
    let auth = translation::niutrans_auth_string(
        app_id,
        apikey,
        source,
        target,
        &request.timestamp,
        &request.text,
    );
    let body = format!(
        "from={}&to={}&appId={}&timestamp={}&srcText={}&authStr={auth}",
        percent_encode(source),
        percent_encode(target),
        percent_encode(app_id),
        percent_encode(&request.timestamp),
        percent_encode(&request.text),
    );
    Ok(json!({
        "url": URL,
        "method": "POST",
        "headers": {"Content-Type": "application/x-www-form-urlencoded; charset=utf-8"},
        "body_utf8": body,
        "timeout_ms": 2500,
        "max_response_bytes": 1048576
    }))
}

/// Whether a NiuTrans reply reports a failure (a body that is not a JSON object, or one carrying `errorCode`/`errorMsg`, which is how NiuTrans reports rate limits and credential errors) rather than an answer. `parse` returns `None` for both a failure and an answer with no text; only answers are negative-cached.
pub fn failed(bytes: &[u8]) -> bool {
    if bytes.len() > 1048576 {
        return true;
    }
    let Some(root) = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
    else {
        return true;
    };
    !root.is_object() || root.get("errorCode").is_some() || root.get("errorMsg").is_some()
}

pub fn parse(bytes: &[u8]) -> Option<Value> {
    if bytes.len() > 1048576 {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let root: Value = serde_json::from_str(text).ok()?;
    if root.get("errorCode").is_some() || root.get("errorMsg").is_some() {
        return None;
    }
    root.get("tgtText")
        .and_then(Value::as_str)
        .and_then(translation::format_translation_gloss)
        .filter(|text| text.len() <= 4096)
        .map(Value::String)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Value {
        json!({"config":{"enabled":true,"app_id":"app-id","apikey":"api-key"},
            "text":"hello","source_language":"en","target_language":"zh",
            "timestamp":"1704067200000"})
    }

    #[test]
    fn descriptor_signs_raw_text_and_url_encodes_body() {
        let value = descriptor(&serde_json::to_vec(&request()).unwrap()).unwrap();
        assert_eq!(value["url"], URL);
        assert_eq!(
            value["headers"]["Content-Type"],
            "application/x-www-form-urlencoded; charset=utf-8"
        );
        let body = value["body_utf8"].as_str().unwrap();
        assert!(body.contains("srcText=hello"));
        assert!(body.contains("authStr=6da3515e010ef871b66e4e31ff5ba580"));
    }

    #[test]
    fn an_empty_answer_is_not_a_failed_reply() {
        assert!(!failed(br#"{"tgtText":""}"#));
        assert!(!failed(br#"{"tgtText":"hello"}"#));
        assert!(failed(
            br#"{"errorCode":"13001","errorMsg":"rate limited"}"#
        ));
        assert!(failed(br#"{"errorMsg":"bad apikey"}"#));
        assert!(failed(b"not json"));
        assert!(failed(br#"[1]"#));
    }

    #[test]
    fn descriptor_rejects_invalid_credentials_and_parse_rejects_errors() {
        let mut invalid = request();
        invalid["config"]["apikey"] = json!("<YOUR_NIUTRANS_APIKEY>");
        assert_eq!(
            descriptor(&serde_json::to_vec(&invalid).unwrap()),
            Err("invalid NiuTrans parameters")
        );
        assert_eq!(
            parse(br#"{"tgtText":"  hello\nworld "}"#),
            Some(json!("hello world"))
        );
        assert!(parse(br#"{"errorCode":"401","tgtText":"hello"}"#).is_none());
        assert!(parse(br#"{"tgtText":42}"#).is_none());
    }

    #[test]
    fn parsed_glosses_obey_the_apply_translations_byte_limit() {
        let at_limit = "x".repeat(4096);
        let response = serde_json::to_vec(&json!({"tgtText": at_limit})).unwrap();
        assert_eq!(parse(&response), Some(json!(at_limit)));

        let beyond_limit = "好".repeat(1366);
        let response = serde_json::to_vec(&json!({"tgtText": beyond_limit})).unwrap();
        assert!(parse(&response).is_none());
    }
}
