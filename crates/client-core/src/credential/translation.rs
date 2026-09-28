//! Translation configuration probes. No user input or provider errors leave this boundary.
use crate::credential::probe::ProbeResult;
use crate::translation;
use serde_json::{json, Value};
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
        && !value.chars().all(|c| c == '*')
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
            let url = reqwest::Url::parse(endpoint).ok()?;
            if !crate::text::is_bounded_text(endpoint, 2048)
                || !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return None;
            }
            let key = get("api_key");
            if !key.is_empty() {
                if !usable(key) {
                    return None;
                }
                headers.push(("Authorization".into(), format!("Bearer {key}")));
            }
            (
                endpoint.into(),
                json!({"text": "测试", "source_lang": "ZH", "target_lang": "EN"}).to_string(),
            )
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
    let translated = transport
        .post(&request)
        .and_then(|(status, body)| {
            if !(200..300).contains(&status) || body.len() > 256 * 1024 {
                return None;
            }
            match service {
                "translation.tencent" => {
                    let root: Value = serde_json::from_str(&body).ok()?;
                    if root.get("Response")?.get("Error").is_some() {
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
        .filter(|text| !text.trim().is_empty());
    ProbeResult {
        ok: translated.is_some(),
        message: if translated.is_some() {
            "连接成功，翻译服务配置有效。"
        } else {
            "测试失败：服务未返回有效译文，请检查凭据、接口地址和网络。"
        }
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(u16, &'static str);
    impl Transport for Fake {
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
