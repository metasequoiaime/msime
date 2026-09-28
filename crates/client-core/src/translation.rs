//! Parsing and validation helpers for DeepLX-compatible custom translation services.
//!
//! The learned-translation store lives in [`store`]; this module is the
//! translation request contract itself.

pub mod store;

use hmac::{Hmac, KeyInit, Mac};
use md5::Md5;
use serde_json::Value;
use sha2::Digest;
use sha2::Sha256;

const MAX_SOURCE_CHARS: usize = 40;
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
        || texts
            .iter()
            .any(|text| text.is_empty() || !crate::text::is_bounded_chars(text, MAX_SOURCE_CHARS))
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
    Some(
        values
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect(),
    )
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

pub fn is_supported_endpoint(endpoint: &str) -> bool {
    !endpoint.is_empty()
        && crate::text::is_bounded_text(endpoint, 2048)
        && (endpoint.starts_with("https://") || endpoint.starts_with("http://"))
}

pub fn parse_translation_response(response: &str) -> Option<String> {
    let root: Value = serde_json::from_str(response).ok()?;
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

fn value_as_text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            value.as_object().and_then(|object| {
                ["text", "translation", "data"]
                    .iter()
                    .find_map(|key| object.get(*key).and_then(Value::as_str).map(str::to_owned))
            })
        })
        .filter(|text| !text.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_supported_endpoints_only() {
        assert!(is_supported_endpoint("https://translate.example/api"));
        assert!(is_supported_endpoint("http://localhost:8080/translate"));
        assert!(!is_supported_endpoint("ftp://translate.example"));
        assert!(!is_supported_endpoint("https://bad\n.example"));
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
        assert!(tencent_tmt_payload("zh", "en", &["字".repeat(41)]).is_none());
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
