//! Explicit user-requested credential probes; never run while loading settings.
use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;

#[derive(serde::Serialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub message: String,
}

fn result(ok: bool, message: &str) -> ProbeResult {
    ProbeResult {
        ok,
        message: message.into(),
    }
}

/// Injected transport returns only a status, never private response/error text.
pub trait ProbeTransport {
    fn post(&self, endpoint: &str, token: &str, body: &Value) -> Option<u16>;
}

pub struct HttpsProbeTransport;
impl ProbeTransport for HttpsProbeTransport {
    fn post(&self, endpoint: &str, token: &str, body: &Value) -> Option<u16> {
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .ok()?;
        let response = client
            .post(endpoint)
            .bearer_auth(token)
            .json(body)
            .send()
            .ok()?;
        let status = response.status().as_u16();
        // Match the source's response ceiling without retaining provider text.
        let mut limited = response.take(256 * 1024 + 1);
        let bytes = std::io::copy(&mut limited, &mut std::io::sink()).ok()?;
        (bytes <= 256 * 1024).then_some(status)
    }
}

pub fn test_chat(service: &str, config: &Value, transport: &impl ProbeTransport) -> ProbeResult {
    if !matches!(service, "ai.assistant" | "voice.polish") {
        return result(false, "此配置测试类型尚未接入。");
    }
    let value = |key| config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let token = value("token");
    let endpoint = value("endpoint");
    let model = value("model");
    if !crate::credential::usable_token(token) {
        return result(false, "请先填写有效的 API Key。");
    }
    if !crate::credential::valid_https_endpoint_and_model(endpoint, model) {
        return result(false, "请填写有效的 HTTPS 接口地址和模型名。");
    }
    // Same minimal chat request as the pinned Windows reference implementation.
    let mut body = json!({"model": model, "stream": false, "max_tokens": 1,
        "messages": [{"role": "user", "content": "Reply OK"}]});
    match value("provider") {
        "deepseek" => body["thinking"] = json!({"type": "disabled"}),
        "siliconflow" => body["enable_thinking"] = json!(false),
        _ => {}
    }
    match transport.post(endpoint, token, &body) {
        Some(200..=299) => result(true, "连接成功，API Key 和模型配置有效。"),
        Some(401 | 403) => result(false, "认证失败，请检查 API Key 和访问权限。"),
        Some(429) => result(false, "服务限流或额度不足，请稍后重试。"),
        Some(_) => result(false, "服务拒绝请求，请检查接口地址和模型配置。"),
        None => result(false, "连接失败或响应超出限制，请检查网络后重试。"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    struct Fake {
        status: Option<u16>,
        bodies: RefCell<Vec<Value>>,
    }
    impl ProbeTransport for Fake {
        fn post(&self, endpoint: &str, token: &str, body: &Value) -> Option<u16> {
            assert_eq!(endpoint, "https://fixture.invalid/chat");
            assert_eq!(token, "synthetic-key");
            self.bodies.borrow_mut().push(body.clone());
            self.status
        }
    }
    fn config(provider: &str) -> Value {
        json!({"provider": provider, "token": "synthetic-key",
            "endpoint": "https://fixture.invalid/chat", "model": "fixture-model"})
    }
    #[test]
    fn credential_chat_preserves_provider_payloads() {
        let fake = Fake {
            status: Some(200),
            bodies: RefCell::default(),
        };
        for provider in ["deepseek", "siliconflow", "openai", "groq"] {
            for service in ["ai.assistant", "voice.polish"] {
                assert!(test_chat(service, &config(provider), &fake).ok);
            }
        }
        let bodies = fake.bodies.borrow();
        assert_eq!(bodies[0]["thinking"], json!({"type":"disabled"}));
        assert_eq!(bodies[2]["enable_thinking"], false);
        assert!(bodies[4].get("thinking").is_none());
        for body in bodies.iter() {
            assert_eq!(body["max_tokens"], 1);
            assert_eq!(body["stream"], false);
            assert_eq!(body["messages"][0]["content"], "Reply OK");
            assert!(body.get("token").is_none());
        }
    }
    #[test]
    fn credential_chat_rejects_invalid_requests_before_transport() {
        let fake = Fake {
            status: Some(200),
            bodies: RefCell::default(),
        };
        for (key, value) in [
            ("token", ""),
            ("token", "a\r\nb"),
            ("token", "***"),
            ("endpoint", "http://fixture.invalid/chat"),
            ("endpoint", "https://user:pass@fixture.invalid/chat"),
            ("endpoint", "https://fixture.invalid/chat#fragment"),
            ("model", ""),
        ] {
            let mut request = config("openai");
            request[key] = json!(value);
            assert!(!test_chat("ai.assistant", &request, &fake).ok);
        }
        assert!(!test_chat("voice.asr", &config("openai"), &fake).ok);
        assert!(fake.bodies.borrow().is_empty());
    }
    #[test]
    fn credential_chat_returns_only_bounded_public_status() {
        for status in [None, Some(301), Some(401), Some(403), Some(429), Some(500)] {
            let fake = Fake {
                status,
                bodies: RefCell::default(),
            };
            let result = test_chat("voice.polish", &config("openai"), &fake);
            assert!(!result.ok);
            assert!(result.message.len() < 256);
            assert!(!result.message.contains("synthetic-key"));
        }
    }
}
