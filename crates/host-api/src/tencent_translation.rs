//! Pure descriptors for a native, host-owned Tencent TMT transport.
use msime_client_core::is_bounded_text;
use msime_client_core::translation;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct Config {
    enabled: bool,
    secret_id: String,
    secret_key: String,
    region: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    config: Config,
    texts: Vec<String>,
    source_language: String,
    target_language: String,
    timestamp: i64,
}

pub fn descriptor(bytes: &[u8]) -> Result<Value, &'static str> {
    let request: Request = serde_json::from_slice(bytes).map_err(|_| "invalid Tencent request")?;
    if !request.config.enabled {
        return Ok(Value::Null);
    }
    let trim = |text: String| text.trim_matches([' ', '\t', '\r', '\n']).to_owned();
    let id = trim(request.config.secret_id);
    let key = trim(request.config.secret_key);
    let region = trim(request.config.region);
    let valid_token = |value: &str| {
        !value.is_empty() && is_bounded_text(value, 4096) && translation::usable_credential(value)
    };
    if !valid_token(&id)
        || !valid_token(&key)
        || !msime_client_core::is_bounded_ascii_identifier(&id, 4096)
        || region.len() > 64
        || !msime_client_core::is_ascii_alphanumeric_dash(&region)
        || !translation::is_supported_translation_pair(
            &request.source_language,
            &request.target_language,
        )
        || request.texts.is_empty()
        || request.texts.len() > 9
        || request
            .texts
            .iter()
            .any(|text| !translation::is_valid_source_text(text))
        || request.timestamp < 0
    {
        return Err("invalid Tencent parameters");
    }
    let utc = time::OffsetDateTime::from_unix_timestamp(request.timestamp)
        .map_err(|_| "invalid Tencent timestamp")?;
    let date = format!(
        "{:04}-{:02}-{:02}",
        utc.year(),
        utc.month() as u8,
        utc.day()
    );
    let payload = translation::tencent_tmt_payload(
        &request.source_language,
        &request.target_language,
        &request.texts,
    )
    .ok_or("invalid Tencent batch")?;
    let authorization = translation::tencent_tc3_authorization(
        &id,
        &key,
        request.timestamp,
        &date,
        payload.as_bytes(),
    );
    let headers: serde_json::Map<String, Value> =
        translation::tencent_tmt_headers(&region, request.timestamp, &authorization)
            .into_iter()
            .map(|(name, value)| (name, Value::String(value)))
            .collect();
    Ok(json!({
        "url":"https://tmt.tencentcloudapi.com", "method":"POST", "headers":headers,
        // These exact UTF-8 bytes were signed. The host must not reserialize JSON.
        "body_utf8":payload, "timeout_ms":2500, "max_response_bytes":1048576,
        "expected_count":request.texts.len()
    }))
}

pub fn parse(bytes: &[u8], expected: usize) -> Option<Value> {
    if bytes.len() > 1048576 || !(1..=9).contains(&expected) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let document: Value = serde_json::from_str(text).ok()?;
    if document
        .get("Response")?
        .get("Error")
        .is_some_and(|error| !error.is_null())
    {
        return None;
    }
    let values = translation::parse_tencent_tmt_response(text, expected)?;
    let mut glosses = Vec::with_capacity(values.len());
    glosses.extend(
        values.iter().map(|text| {
            translation::format_translation_gloss(text).filter(|text| text.len() <= 4096)
        }),
    );
    Some(json!(glosses))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Value {
        json!({"config":{"enabled":true,"secret_id":"AKIDsynthetic","secret_key":"synthetic","region":""},
            "texts":["测试"],"source_language":"zh","target_language":"en","timestamp":1704067200})
    }
    fn build(value: Value) -> Result<Value, &'static str> {
        descriptor(&serde_json::to_vec(&value).unwrap())
    }
    #[test]
    fn exact_payload_and_utc_scope_are_preserved() {
        let value = build(request()).unwrap();
        let payload = value["body_utf8"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(payload).unwrap(),
            json!({"Source":"zh","Target":"en","ProjectId":0,"SourceTextList":["测试"]})
        );
        assert_eq!(
            value["headers"]["Authorization"],
            translation::tencent_tc3_authorization(
                "AKIDsynthetic",
                "synthetic",
                1704067200,
                "2024-01-01",
                payload.as_bytes()
            )
        );
        assert_eq!(value["headers"]["X-TC-Region"], "ap-guangzhou");
        assert_eq!(value["expected_count"], 1);
        assert_eq!(value["timeout_ms"], 2500);
        assert_eq!(value["max_response_bytes"], 1048576);
        let mut previous = request();
        previous["timestamp"] = json!(1704067199_i64);
        assert!(build(previous).unwrap()["headers"]["Authorization"]
            .as_str()
            .unwrap()
            .contains("/2023-12-31/tmt/"));
    }
    #[test]
    fn rejects_invalid_inputs_without_echoing_secrets() {
        for (field, value) in [
            ("secret_id", "<placeholder>"),
            ("secret_key", "FAKESECRET_test"),
            ("secret_key", "secret\u{0}bad"),
            ("region", "ap-guangzhou\r\nInjected"),
        ] {
            let mut invalid = request();
            invalid["config"][field] = json!(value);
            assert_eq!(build(invalid), Err("invalid Tencent parameters"));
        }
        for (field, value) in [
            ("texts", json!(["x".repeat(41)])),
            ("texts", json!([])),
            ("texts", json!(vec!["x"; 10])),
            ("timestamp", json!(-1)),
            ("timestamp", json!(i64::MAX)),
            ("source_language", json!("unknown")),
        ] {
            let mut invalid = request();
            invalid[field] = value;
            assert!(build(invalid).is_err());
        }
        for codepoint in (0..=0x1f).chain(0x7f..=0x9f) {
            let control = char::from_u32(codepoint).unwrap();
            let mut invalid = request();
            invalid["texts"] = json!([format!("before{control}after")]);
            assert!(build(invalid).is_err());
        }
        let mut disabled = request();
        disabled["config"] = json!({"enabled":false});
        assert_eq!(build(disabled), Ok(Value::Null));
    }
    #[test]
    fn preserves_response_slots_and_rejects_error_documents() {
        assert_eq!(
            parse(
                br#"{"Response":{"TargetTextList":[" hello\nworld ",""]}}"#,
                2
            ),
            Some(json!(["hello world", null]))
        );
        for bytes in [
            b"invalid".as_slice(),
            b"\xff",
            br#"{"Response":{"TargetTextList":["one"]}}"#,
            br#"{"Response":{"Error":{"Code":"synthetic"},"TargetTextList":["one","two"]}}"#,
        ] {
            assert!(parse(bytes, 2).is_none());
        }
        assert!(parse(br#"{"Response":{"TargetTextList":[]}}"#, 0).is_none());
    }
}
