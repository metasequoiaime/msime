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
use std::io::Read;
use std::time::{Duration, Instant};

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_millis(2500);
const BATCH_BUDGET: Duration = Duration::from_secs(6);
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

/// One signed Tencent TMT call: the credentials, the region they are scoped to
/// and the moment the signature covers.
pub struct TencentTmtRequest<'a> {
    pub secret_id: &'a str,
    pub secret_key: &'a str,
    pub region: &'a str,
    pub timestamp: i64,
    pub date: &'a str,
    pub source: &'a str,
    pub target: &'a str,
}

pub fn translate_tencent_batch(
    request_info: &TencentTmtRequest<'_>,
    texts: &[String],
) -> Vec<Option<String>> {
    let TencentTmtRequest {
        secret_id,
        secret_key,
        region,
        timestamp,
        date,
        source,
        target,
    } = *request_info;
    let results = vec![None; texts.len()];
    let Some(payload) = tencent_tmt_payload(source, target, texts) else {
        return results;
    };
    let authorization =
        tencent_tc3_authorization(secret_id, secret_key, timestamp, date, payload.as_bytes());
    if authorization.is_empty() {
        return results;
    }
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(REQUEST_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(_) => return results,
    };
    let mut request = client.post("https://tmt.tencentcloudapi.com").body(payload);
    for (name, value) in tencent_tmt_headers(region, timestamp, &authorization) {
        request = request.header(name, value);
    }
    let mut response = match request.send() {
        Ok(response) if response.status().is_success() => response,
        _ => return results,
    };
    let body = match read_bounded_body(&mut response).and_then(|body| String::from_utf8(body).ok())
    {
        Some(body) => body,
        None => return results,
    };
    parse_tencent_tmt_response(&body, texts.len())
        .into_iter()
        .flatten()
        .map(Some)
        .collect()
}

pub fn tencent_tmt_payload(source: &str, target: &str, texts: &[String]) -> Option<String> {
    if source.is_empty()
        || target.is_empty()
        || texts.is_empty()
        || texts.len() > 50
        || texts.iter().any(|text| {
            text.is_empty()
                || text.chars().count() > MAX_SOURCE_CHARS
                || text.chars().any(char::is_control)
        })
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

pub fn usable_tencent_secret(value: &str) -> bool {
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

pub fn usable_niutrans_credential(value: &str) -> bool {
    let trimmed = value.trim_matches([' ', '\t', '\r', '\n']);
    !trimmed.is_empty()
        && !(trimmed.starts_with('<') && trimmed.ends_with('>'))
        && !trimmed.starts_with("FAKESECRET_")
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

#[derive(Clone, PartialEq, Eq)]
pub struct TranslationConfig {
    pub endpoint: String,
    pub api_key: String,
}

pub fn translate_batch(
    config: &TranslationConfig,
    texts: &[String],
    source: &str,
    target: &str,
) -> Vec<Option<String>> {
    let mut cache = crate::cloud::candidates::TranslationCache::new(Duration::from_secs(480));
    translate_batch_cached(config, texts, source, target, &mut cache)
}

pub fn translate_batch_cached(
    config: &TranslationConfig,
    texts: &[String],
    source: &str,
    target: &str,
    cache: &mut crate::cloud::candidates::TranslationCache,
) -> Vec<Option<String>> {
    let mut results = vec![None; texts.len()];
    if texts.is_empty()
        || source.is_empty()
        || target.is_empty()
        || !is_supported_endpoint(&config.endpoint)
    {
        return results;
    }
    let client = match reqwest::blocking::Client::builder()
        .connect_timeout(REQUEST_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(_) => return results,
    };
    let started = Instant::now();
    let scope = format!("{}\0{}\0", source, target);
    let mut pending = Vec::new();
    for (index, text) in texts.iter().enumerate() {
        if text.is_empty()
            || text.chars().count() > MAX_SOURCE_CHARS
            || text.chars().any(char::is_control)
        {
            continue;
        }
        let cache_key = format!("{scope}{text}");
        if let Some(value) = cache.get(&cache_key) {
            results[index] = value;
        } else {
            pending.push((index, text));
        }
    }
    for (index, text) in pending {
        let timeout = request_timeout(started.elapsed());
        if timeout.is_zero() {
            break;
        }
        let mut request = client
            .post(&config.endpoint)
            .timeout(timeout)
            .json(&serde_json::json!({
                "text": text,
                "source_lang": source.to_ascii_uppercase(),
                "target_lang": target.to_ascii_uppercase(),
            }));
        if !config.api_key.is_empty() {
            request = request.bearer_auth(&config.api_key);
        }
        let response = match request.send() {
            Ok(response) if response.status().is_success() => response,
            _ => continue,
        };
        let value = read_translation_response(response);
        cache.remember(format!("{scope}{text}"), value.clone());
        results[index] = value;
    }
    results
}

fn request_timeout(elapsed: Duration) -> Duration {
    BATCH_BUDGET.saturating_sub(elapsed).min(REQUEST_TIMEOUT)
}

fn read_bounded_body(mut reader: impl Read) -> Option<Vec<u8>> {
    crate::bounded_io::read_bounded(&mut reader, MAX_RESPONSE_BYTES as u64).ok()
}

fn read_translation_response(reader: impl Read) -> Option<String> {
    let body = read_bounded_body(reader)?;
    parse_translation_response(std::str::from_utf8(&body).ok()?)
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
    fn response_limit_is_enforced_while_reading() {
        struct Endless {
            read: usize,
        }
        impl Read for Endless {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                buffer.fill(b' ');
                self.read += buffer.len();
                Ok(buffer.len())
            }
        }
        let mut endless = Endless { read: 0 };
        assert!(read_translation_response(&mut endless).is_none());
        assert_eq!(endless.read, MAX_RESPONSE_BYTES + 1);
        let mut boundary = br#"{"data":"synthetic"}"#.to_vec();
        boundary.resize(MAX_RESPONSE_BYTES, b' ');
        assert_eq!(
            read_translation_response(boundary.as_slice()).as_deref(),
            Some("synthetic")
        );
        boundary.push(b' ');
        assert!(read_translation_response(boundary.as_slice()).is_none());
        assert!(read_translation_response(&b"\xff"[..]).is_none());
    }

    #[test]
    fn tencent_response_body_is_bounded_before_decoding() {
        struct Endless {
            read: usize,
        }
        impl Read for Endless {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                buffer.fill(b'x');
                self.read += buffer.len();
                Ok(buffer.len())
            }
        }
        let mut endless = Endless { read: 0 };
        assert!(read_bounded_body(&mut endless).is_none());
        assert_eq!(endless.read, MAX_RESPONSE_BYTES + 1);
        let valid = br#"{"Response":{"TargetTextList":["synthetic"]}}"#;
        assert_eq!(
            read_bounded_body(valid.as_slice()).as_deref(),
            Some(valid.as_slice())
        );
    }

    #[test]
    fn request_timeout_respects_remaining_batch_budget() {
        assert_eq!(request_timeout(Duration::ZERO), REQUEST_TIMEOUT);
        assert_eq!(
            request_timeout(Duration::from_millis(5500)),
            Duration::from_millis(500)
        );
        assert_eq!(request_timeout(BATCH_BUDGET), Duration::ZERO);
        assert_eq!(request_timeout(Duration::from_secs(7)), Duration::ZERO);
    }

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
    fn translates_batch_with_deeplx_contract() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut request = String::new();
            let mut content_length = None;
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = Some(value.trim().parse::<usize>().unwrap());
                }
                request.push_str(&line);
            }
            let mut body = vec![0; content_length.unwrap()];
            reader.read_exact(&mut body).unwrap();
            request.push_str(std::str::from_utf8(&body).unwrap());
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer test-key"));
            assert!(request.contains("source_lang\":\"EN\""));
            let body = r#"{"data":"","translation":"你好"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        // Exercise the same validated, persisted preferences the host reads.
        let directory = tempfile::tempdir().unwrap();
        let store = crate::preferences::PreferencesStore::new(directory.path());
        let mut preferences = crate::preferences::Preferences::default();
        preferences.custom_translation = crate::preferences::CustomTranslationPreferences {
            enabled: true,
            endpoint: format!("http://{address}"),
            api_key: "test-key".into(),
        };
        store.save(0, preferences).unwrap();
        let saved = store.load().unwrap().preferences.custom_translation;
        assert!(saved.enabled);
        let config = TranslationConfig {
            endpoint: saved.endpoint,
            api_key: saved.api_key,
        };
        let result = translate_batch(&config, &["hello".into()], "en", "zh");
        server.join().unwrap();
        assert_eq!(result, vec![Some("你好".into())]);
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
    fn translation_inputs_over_source_limit_are_not_requested() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let server_done = done.clone();
        let server_accepted = accepted.clone();
        let server = std::thread::spawn(move || loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    server_accepted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut request = [0_u8; 4096];
                    let _ = stream.read(&mut request);
                    let body = r#"{"data":"unexpected"}"#;
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .unwrap();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if server_done.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("translation fixture failed: {error}"),
            }
        });
        let mut cache = crate::cloud::candidates::TranslationCache::new(Duration::from_secs(1));
        let config = TranslationConfig {
            endpoint: format!("http://{address}"),
            api_key: String::new(),
        };
        for text in [
            String::new(),
            "字".repeat(41),
            "before\0after".into(),
            "before\u{0085}after".into(),
        ] {
            assert_eq!(
                translate_batch_cached(&config, &[text], "zh", "en", &mut cache),
                vec![None]
            );
        }
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        server.join().unwrap();
        assert_eq!(accepted.load(std::sync::atomic::Ordering::Relaxed), 0);
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
        assert!(usable_niutrans_credential("real-value"));
        assert!(!usable_niutrans_credential("<YOUR_NIUTRANS_APP_ID>"));
        assert!(!usable_niutrans_credential("FAKESECRET_test"));
        assert!(!usable_niutrans_credential(" \n\t"));
    }

    #[test]
    fn persistence_requires_short_changed_gloss() {
        assert!(should_persist_translation("hello", "你好"));
        assert!(!should_persist_translation("hello", "hello"));
        assert!(!should_persist_translation("hello", &"字".repeat(33)));
    }

    #[test]
    fn rejects_placeholder_tencent_secrets() {
        assert!(usable_tencent_secret(" real-secret "));
        assert!(!usable_tencent_secret("<YOUR_TENCENT_SECRET_ID>"));
        assert!(!usable_tencent_secret("FAKESECRET_test"));
        assert!(!usable_tencent_secret(" \n\t"));
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
