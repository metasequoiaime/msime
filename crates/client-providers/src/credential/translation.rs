//! Translation configuration probes. No user input or provider errors leave this boundary.
use crate::credential::probe::ProbeResult;
use crate::translation;
use serde_json::Value;
use std::time::Duration;

pub struct Request {
    pub endpoint: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}
pub trait Transport {
    fn post(&self, request: &Request) -> Option<(u16, String)>;
}
pub struct HttpTransport;
impl Transport for HttpTransport {
    fn post(&self, request: &Request) -> Option<(u16, String)> {
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .ok()?;
        let mut post = client.post(&request.endpoint).body(request.body.clone());
        for (key, value) in &request.headers {
            post = post.header(key, value);
        }
        let response = post.send().ok()?;
        let status = response.status().as_u16();
        let bytes = crate::bounded_io::read_bounded(response, 256 * 1024).ok()?;
        Some((status, String::from_utf8(bytes).ok()?))
    }
}

fn usable(value: &str) -> bool {
    !value.is_empty()
        && crate::text::is_bounded_text(value, 4096)
        && !value.starts_with('<')
        && !value.starts_with("FAKESECRET_")
        && !crate::credential::is_all_asterisks(value)
}

fn request(service: &str, config: &Value, milliseconds: u64) -> Option<Request> {
    let get = |key| config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let mut headers = vec![("Content-Type".into(), "application/json".into())];
    let (endpoint, body) = match service {
        "translation.tencent" => {
            let (id, key) = (get("secret_id"), get("secret_key"));
            if !usable(id) || !usable(key) {
                return None;
            }
            let region = get("region");
            if region.len() > 64 || !crate::is_ascii_alphanumeric_dash(region) {
                return None;
            }
            let seconds = i64::try_from(milliseconds / 1000).ok()?;
            let date = time::OffsetDateTime::from_unix_timestamp(seconds)
                .ok()?
                .date()
                .to_string();
            let body = translation::tencent_tmt_payload("zh", "en", &["测试".into()])?;
            let authorization =
                translation::tencent_tc3_authorization(id, key, seconds, &date, body.as_bytes());
            headers = translation::tencent_tmt_headers(region, seconds, &authorization);
            ("https://tmt.tencentcloudapi.com".into(), body)
        }
        "translation.niutrans" => {
            let (id, key) = (get("app_id"), get("apikey"));
            if !usable(id) || !usable(key) {
                return None;
            }
            let stamp = milliseconds.to_string();
            let signature = translation::niutrans_auth_string(id, key, "zh", "en", &stamp, "测试");
            // Use the URL form serializer; the signature covers raw UTF-8 values.
            let mut form = reqwest::Url::parse("https://fixture.invalid").ok()?;
            form.query_pairs_mut().extend_pairs([
                ("from", "zh"),
                ("to", "en"),
                ("appId", id),
                ("timestamp", &stamp),
                ("srcText", "测试"),
                ("authStr", &signature),
            ]);
            headers[0].1 = "application/x-www-form-urlencoded; charset=utf-8".into();
            (
                "https://api.niutrans.com/v2/text/translate".into(),
                form.query()?.into(),
            )
        }
        "translation.custom" => {
            let endpoint = get("endpoint");
            if !translation::is_secure_endpoint(endpoint) {
                return None;
            }
            let key = get("api_key");
            if !key.is_empty() && !usable(key) {
                return None;
            }
            // 与运行时描述器同一个构造函数，DeepL 官方地址在这里同样换成 DeepL 的鉴权和正文。
            let request =
                translation::custom_translation_request(endpoint, key, "测试", "zh", "en")?;
            headers = request.headers;
            (request.url, request.body.to_string())
        }
        _ => return None,
    };
    Some(Request {
        endpoint,
        headers,
        body,
    })
}

pub fn test(
    service: &str,
    config: &Value,
    milliseconds: u64,
    transport: &impl Transport,
) -> ProbeResult {
    let Some(request) = request(service, config, milliseconds) else {
        return ProbeResult {
            ok: false,
            message: "请填写有效的翻译服务凭据和接口地址。".into(),
        };
    };
    let reply = transport.post(&request);
    let status = reply.as_ref().map(|(status, _)| *status);
    let translated = reply
        .and_then(|(status, body)| {
            if !(200..300).contains(&status) || body.len() > 256 * 1024 {
                return None;
            }
            match service {
                "translation.tencent" => {
                    let root: Value = serde_json::from_str(&body).ok()?;
                    if root
                        .get("Response")?
                        .get("Error")
                        .is_some_and(|error| !error.is_null())
                    {
                        return None;
                    }
                    translation::parse_tencent_tmt_response(&body, 1)?
                        .into_iter()
                        .next()
                }
                "translation.niutrans" => {
                    let root: Value = serde_json::from_str(&body).ok()?;
                    if root.get("errorCode").is_some() || root.get("errorMsg").is_some() {
                        return None;
                    }
                    root.get("tgtText")?.as_str().map(str::to_owned)
                }
                _ => translation::parse_translation_response(&body),
            }
        })
        .and_then(|text| translation::format_translation_gloss(&text))
        .filter(|text| text.len() <= 4096);
    let deepl =
        service == "translation.custom" && translation::is_deepl_api_endpoint(&request.endpoint);
    ProbeResult {
        ok: translated.is_some(),
        message: if translated.is_some() {
            "连接成功，翻译服务配置有效。"
        } else if deepl {
            deepl_failure_message(status)
        } else {
            "测试失败：服务未返回有效译文，请检查凭据、接口地址和网络。"
        }
        .into(),
    }
}

/// DeepL 用状态码区分常见错误，测试按钮据此给出能照着改的提示；只用状态码，不转述服务端正文。鸿蒙设置页拿不到这个函数，`CustomTranslationProbePolicy.failureMessage` 有一份逐字相同的文案，改这里要一起改：scripts/test-harmony-custom-translation-probe.py 和 platforms/harmony/tests/custom-translation-probe.test.ts 都会核对。
fn deepl_failure_message(status: Option<u16>) -> &'static str {
    match status {
        Some(403) => {
            "测试失败：DeepL 拒绝了这个 API Key（HTTP 403），请检查密钥是否正确、是否已停用。"
        }
        Some(456) => "测试失败：DeepL API 的字符额度已用完（HTTP 456）。",
        Some(429) => "测试失败：DeepL 请求过于频繁（HTTP 429），请稍后再试。",
        _ => "测试失败：DeepL 未返回有效译文，请检查 API Key 和网络。",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fake<'a>(u16, &'a str);
    impl Transport for Fake<'_> {
        fn post(&self, _: &Request) -> Option<(u16, String)> {
            Some((self.0, self.1.into()))
        }
    }
    fn config() -> Value {
        json!({"secret_id":"synthetic-id", "secret_key":"synthetic-key", "region":"ap-guangzhou",
            "app_id":"synthetic-app", "apikey":"synthetic-key",
            "endpoint":"https://fixture.invalid/translate", "api_key":""})
    }
    #[test]
    fn translation_object_fallback_passes_the_connection_probe() {
        for body in [
            r#"{"data":{"text":"","translation":"synthetic"}}"#,
            r#"{"code":200,"result":[{"text":"","data":"synthetic"}]}"#,
            r#"{"translations":[{"text":"","translation":"synthetic"}]}"#,
        ] {
            let result = test("translation.custom", &config(), 0, &Fake(200, body));
            assert!(result.ok, "{body}: {}", result.message);
        }
    }

    #[test]
    fn translation_probe_request_signatures_and_payloads() {
        let timestamp = 1_700_000_000_000;
        let tencent = request("translation.tencent", &config(), timestamp).unwrap();
        assert_eq!(tencent.endpoint, "https://tmt.tencentcloudapi.com");
        let authorization = translation::tencent_tc3_authorization(
            "synthetic-id",
            "synthetic-key",
            1_700_000_000,
            "2023-11-14",
            tencent.body.as_bytes(),
        );
        assert!(tencent
            .headers
            .contains(&("Authorization".into(), authorization)));
        let niu = request("translation.niutrans", &config(), timestamp).unwrap();
        let form = reqwest::Url::parse(&format!("https://fixture.invalid?{}", niu.body)).unwrap();
        let pairs: std::collections::HashMap<_, _> = form.query_pairs().into_owned().collect();
        assert_eq!(pairs["srcText"], "测试");
        assert_eq!(
            pairs["authStr"],
            translation::niutrans_auth_string(
                "synthetic-app",
                "synthetic-key",
                "zh",
                "en",
                "1700000000000",
                "测试"
            )
        );
        assert!(!pairs.contains_key("apikey"));
        let custom = request("translation.custom", &config(), timestamp).unwrap();
        assert!(!custom.headers.iter().any(|(k, _)| k == "Authorization"));
        assert_eq!(
            serde_json::from_str::<Value>(&custom.body).unwrap()["target_lang"],
            "EN"
        );
    }
    #[test]
    fn translation_probe_requires_actual_translation_not_http_success() {
        for (service, body) in [
            (
                "translation.tencent",
                r#"{"Response":{"TargetTextList":["fixture"]}}"#,
            ),
            ("translation.niutrans", r#"{"tgtText":"fixture"}"#),
            ("translation.custom", r#"{"data":"fixture"}"#),
        ] {
            assert!(test(service, &config(), 0, &Fake(200, body)).ok);
            assert!(!test(service, &config(), 0, &Fake(401, body)).ok);
            assert!(!test(service, &config(), 0, &Fake(200, "{}")).ok);
            let failed = test(
                service,
                &config(),
                0,
                &Fake(200, r#"{"errorMsg":"synthetic-private-response"}"#),
            );
            assert!(!failed.ok);
            assert!(!failed.message.contains("synthetic-private-response"));
        }
        assert!(
            !test(
                "translation.tencent",
                &config(),
                0,
                &Fake(
                    200,
                    r#"{"Response":{"TargetTextList":["fixture"],"Error":{}}}"#
                )
            )
            .ok
        );
    }

    #[test]
    fn translation_probe_rejects_glosses_the_host_cannot_apply() {
        let response = |service: &str, gloss: &str| {
            match service {
                "translation.tencent" => json!({"Response":{"TargetTextList":[gloss]}}),
                "translation.niutrans" => json!({"tgtText":gloss}),
                "translation.custom" => json!({"data":gloss}),
                _ => unreachable!(),
            }
            .to_string()
        };
        for service in [
            "translation.tencent",
            "translation.niutrans",
            "translation.custom",
        ] {
            let at_limit = response(service, &"x".repeat(4096));
            assert!(test(service, &config(), 0, &Fake(200, &at_limit)).ok);
            for unusable in ["synthetic\u{0}gloss".to_owned(), "好".repeat(1366)] {
                let body = response(service, &unusable);
                assert!(!test(service, &config(), 0, &Fake(200, &body)).ok);
            }
        }
    }

    #[test]
    fn translation_probe_accepts_a_null_tencent_error_with_valid_text() {
        let body = r#"{"Response":{"TargetTextList":["synthetic"],"Error":null}}"#;
        assert!(test("translation.tencent", &config(), 0, &Fake(200, body)).ok);
    }

    /// 记下实际发出的请求，供核对 DeepL 的鉴权和正文。
    struct Recording<'a>(u16, &'a str, std::cell::RefCell<Vec<Request>>);
    impl Transport for Recording<'_> {
        fn post(&self, request: &Request) -> Option<(u16, String)> {
            self.2.borrow_mut().push(Request {
                endpoint: request.endpoint.clone(),
                headers: request.headers.clone(),
                body: request.body.clone(),
            });
            Some((self.0, self.1.into()))
        }
    }

    fn deepl_config(endpoint: &str, key: &str) -> Value {
        json!({"endpoint": endpoint, "api_key": key})
    }

    #[test]
    fn custom_translation_probe_speaks_deepl_to_official_endpoints() {
        let reply = r#"{"translations":[{"detected_source_language":"ZH","text":"Test"}]}"#;
        let transport = Recording(200, reply, Default::default());
        let result = test(
            "translation.custom",
            &deepl_config(" https://api.deepl.com/v2/translate ", " synthetic-key:fx "),
            0,
            &transport,
        );
        assert!(result.ok, "{}", result.message);
        let sent = transport.2.borrow();
        assert_eq!(sent.len(), 1);
        // Free 密钥被改到 Free 主机。
        assert_eq!(sent[0].endpoint, "https://api-free.deepl.com/v2/translate");
        assert!(sent[0].headers.contains(&(
            "Authorization".into(),
            "DeepL-Auth-Key synthetic-key:fx".into()
        )));
        assert!(!sent[0]
            .headers
            .iter()
            .any(|(_, value)| value.starts_with("Bearer")));
        assert_eq!(
            serde_json::from_str::<Value>(&sent[0].body).unwrap(),
            json!({"text": ["测试"], "source_lang": "ZH", "target_lang": "EN-US"})
        );
    }

    #[test]
    fn custom_translation_probe_needs_a_key_for_deepl() {
        struct Never;
        impl Transport for Never {
            fn post(&self, _: &Request) -> Option<(u16, String)> {
                panic!("unexpected request")
            }
        }
        let result = test(
            "translation.custom",
            &deepl_config("https://api-free.deepl.com/v2/translate", ""),
            0,
            &Never,
        );
        assert!(!result.ok);
    }

    #[test]
    fn custom_translation_probe_explains_deepl_status_codes() {
        let deepl = deepl_config(
            "https://api-free.deepl.com/v2/translate",
            "synthetic-key:fx",
        );
        for (status, body, expected) in [
            (403, r#"{"message":"Forbidden"}"#, "HTTP 403"),
            (456, r#"{"message":"Quota Exceeded"}"#, "HTTP 456"),
            (429, r#"{"message":"Too many requests"}"#, "HTTP 429"),
        ] {
            let result = test("translation.custom", &deepl, 0, &Fake(status, body));
            assert!(!result.ok);
            assert!(result.message.contains(expected), "{}", result.message);
            assert!(!result.message.contains("Quota Exceeded"));
        }
        let result = test("translation.custom", &deepl, 0, &Fake(500, "{}"));
        assert!(!result.ok);
        assert!(result.message.contains("DeepL"));
        // DeepLX 地址的失败提示保持原样，不提 DeepL。
        let result = test("translation.custom", &config(), 0, &Fake(403, "{}"));
        assert!(!result.message.contains("DeepL"));
    }

    #[test]
    fn custom_translation_rejects_missing_url_authority() {
        let mut invalid = config();
        invalid["endpoint"] = json!("https:///translate");
        assert!(request("translation.custom", &invalid, 0).is_none());
    }

    #[test]
    fn custom_translation_rejects_remote_http_before_sending_credentials() {
        let mut invalid = config();
        invalid["endpoint"] = json!("http://translate.example/api");
        invalid["api_key"] = json!("synthetic-api-key");
        assert!(request("translation.custom", &invalid, 0).is_none());

        invalid["endpoint"] = json!("http://127.0.0.1:8080/api");
        assert!(request("translation.custom", &invalid, 0).is_some());
    }

    #[test]
    fn translation_probe_validates_before_transport() {
        struct Never;
        impl Transport for Never {
            fn post(&self, _: &Request) -> Option<(u16, String)> {
                panic!("unexpected request")
            }
        }
        for (service, key, value) in [
            ("translation.tencent", "secret_key", ""),
            ("translation.niutrans", "apikey", "<placeholder>"),
            ("translation.custom", "endpoint", "file:///fixture"),
            (
                "translation.custom",
                "endpoint",
                "https://user:pass@fixture.invalid",
            ),
            ("translation.custom", "api_key", "a\r\nb"),
        ] {
            let mut input = config();
            input[key] = json!(value);
            assert!(!test(service, &input, 0, &Never).ok);
        }
    }
}
