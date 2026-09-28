//! Pure descriptors and response parsing for the host-owned NiuTrans v2 API.
use msime_client_core::cloud::dictionary::percent_encode;
use msime_client_core::is_bounded_text;
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
    translation::usable_credential(value) && is_bounded_text(value, 4096)
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
        || request.text.is_empty()
        || request.text.chars().count() > 40
        || request.text.chars().any(char::is_control)
        || request.timestamp.is_empty()
        || request.timestamp.len() > 20
        || !request.timestamp.bytes().all(|byte| byte.is_ascii_digit())
        || !["zh", "en", "fr", "ja", "es", "ru", "de", "ko"].contains(&source)
        || !["zh", "en", "fr", "ja", "es", "ru", "de", "ko"].contains(&target)
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
}
