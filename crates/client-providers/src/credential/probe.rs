//! 仅在用户明确要求时测试凭据，加载设置时不发探测请求。
use serde_json::{json, Value};
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

/// 探测仅保留固定错误类别，不向设置页返回服务端文字或凭据。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeFailure {
    Connection,
    Timeout,
    InvalidResponse,
    OutputLimit,
}

/// 注入的传输只返回状态或错误类别，不返回私有响应内容。
pub trait ProbeTransport {
    fn post(&self, endpoint: &str, token: &str, body: &Value) -> Result<u16, ProbeFailure>;
}

/// 客户端按 `ai::endpoint` 的公共设置建立：不跟随重定向，https 地址只走 https，局域网的 http 地址直连、不走代理。
pub struct ChatProbeTransport;
impl ProbeTransport for ChatProbeTransport {
    fn post(&self, endpoint: &str, token: &str, body: &Value) -> Result<u16, ProbeFailure> {
        let url = crate::ai::endpoint::validate(endpoint).map_err(|_| ProbeFailure::Connection)?;
        let client = crate::ai::endpoint::blocking_client_builder(&url)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| ProbeFailure::Connection)?;
        // 空 Token 是不鉴权的服务（Ollama），不带 Authorization。
        let request = client.post(endpoint);
        let request = if token.is_empty() {
            request
        } else {
            request.bearer_auth(token)
        };
        let response = request.json(body).send().map_err(|error| {
            if error.is_timeout() {
                ProbeFailure::Timeout
            } else {
                ProbeFailure::Connection
            }
        })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Ok(status);
        }
        // 实际对话需要可用正文，不能只看 HTTP 成功或把纯思考当作答案。
        let bytes = crate::bounded_io::read_bounded(response, 256 * 1024).map_err(|error| {
            if error.kind() == std::io::ErrorKind::InvalidData {
                ProbeFailure::InvalidResponse
            } else if error.kind() == std::io::ErrorKind::TimedOut
                || error
                    .get_ref()
                    .and_then(|error| error.downcast_ref::<reqwest::Error>())
                    .is_some_and(reqwest::Error::is_timeout)
            {
                ProbeFailure::Timeout
            } else {
                ProbeFailure::Connection
            }
        })?;
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|_| ProbeFailure::InvalidResponse)?;
        validate_completion(&document)?;
        Ok(status)
    }
}

fn validate_completion(document: &Value) -> Result<(), ProbeFailure> {
    if document.get("error").is_some_and(|error| !error.is_null()) {
        return Err(ProbeFailure::InvalidResponse);
    }
    if document
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        == Some("length")
    {
        return Err(ProbeFailure::OutputLimit);
    }
    let valid = document
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .is_some_and(|content| {
            let content = content.trim();
            !content.is_empty() && content.len() <= 16 * 1024
        });
    if valid {
        Ok(())
    } else {
        Err(ProbeFailure::InvalidResponse)
    }
}

pub fn test_chat(service: &str, config: &Value, transport: &impl ProbeTransport) -> ProbeResult {
    if !matches!(service, "ai.assistant" | "voice.polish") {
        return result(false, "此配置测试类型尚未接入。");
    }
    let value = |key| config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    // Ollama 不鉴权（`ai::provider_uses_token`）：不看也不发 Token。
    let uses_token = service != "ai.assistant" || crate::ai::provider_uses_token(value("provider"));
    let token = if uses_token { value("token") } else { "" };
    let endpoint = value("endpoint");
    let model = value("model");
    if uses_token && !crate::credential::usable_token(token) {
        return result(false, "请先填写有效的 API Key。");
    }
    // AI 辅助的接口地址按 `ai::endpoint` 检查，本机和局域网的 http 也可以；语音润色仍只收 https。
    if service == "ai.assistant" {
        if !crate::ai::endpoint::is_allowed(endpoint)
            || model.is_empty()
            || !crate::text::is_bounded_text(model, 256)
        {
            return result(
                false,
                "请填写有效的接口地址（https://，或本机、局域网的 http://）和模型名。",
            );
        }
    } else if !crate::credential::valid_https_endpoint_and_model(endpoint, model) {
        return result(false, "请填写有效的 HTTPS 接口地址和模型名。");
    }
    // 给强制思考模型留出生成正文的预算；这是上限，不要求生成 512 个 token。
    let mut body = json!({"model": model, "stream": false, "max_tokens": 512,
        "messages": [{"role": "user", "content": "Reply OK"}]});
    match value("provider") {
        "deepseek" => body["thinking"] = json!({"type": "disabled"}),
        "siliconflow" => body["enable_thinking"] = json!(false),
        _ => {}
    }
    // 聚合服务不能按 provider 判断模型能力，只对文档明确支持的模型降低探测思考量。
    let canonical = model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .to_ascii_lowercase();
    let is_family = |name: &str| {
        canonical == name
            || canonical
                .strip_prefix(name)
                .is_some_and(|suffix| suffix.starts_with('-'))
    };
    let effort = if is_family("gpt-6-luna") || is_family("gpt-6-sol") {
        Some("none")
    } else if is_family("gpt-6-astra") || is_family("gpt-6.1-sol") {
        Some("low")
    } else {
        None
    };
    if let Some(effort) = effort {
        body.as_object_mut().unwrap().remove("max_tokens");
        body["max_completion_tokens"] = json!(512);
        body["reasoning_effort"] = json!(effort);
    } else if is_family("glm-5.3") {
        body.as_object_mut().unwrap().remove("max_tokens");
        body["max_completion_tokens"] = json!(512);
        body["reasoning_effort"] = json!("low");
    }
    match transport.post(endpoint, token, &body) {
        Ok(200..=299) if !uses_token => result(true, "连接成功，模型配置有效。"),
        Ok(200..=299) => result(true, "连接成功，API Key 和模型配置有效。"),
        Ok(401 | 403) => result(false, "认证失败，请检查 API Key 和访问权限。"),
        Ok(429) => result(false, "服务限流或额度不足，请稍后重试。"),
        Ok(_) => result(false, "服务拒绝请求，请检查接口地址和模型配置。"),
        Err(ProbeFailure::OutputLimit) => result(
            false,
            "服务已响应，但模型耗尽测试输出预算，未完成回复。可换用思考较少的模型后重试。",
        ),
        Err(ProbeFailure::InvalidResponse) => result(
            false,
            "服务已响应，但没有返回有效的对话正文，请检查模型与接口格式。",
        ),
        Err(ProbeFailure::Timeout) => {
            result(false, "模型响应超时，请稍后重试或换用响应更快的模型。")
        }
        Err(ProbeFailure::Connection) => result(false, "连接失败，请检查接口地址和网络后重试。"),
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
        fn post(&self, endpoint: &str, token: &str, body: &Value) -> Result<u16, ProbeFailure> {
            assert_eq!(endpoint, "https://fixture.invalid/chat");
            assert_eq!(token, "synthetic-key");
            self.bodies.borrow_mut().push(body.clone());
            self.status.ok_or(ProbeFailure::Connection)
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
        for provider in [
            "deepseek",
            "siliconflow",
            "openai",
            "groq",
            "minimax",
            "kimi",
            "zhipu",
            "qwen",
            "volcengine",
            "openrouter",
            "custom",
        ] {
            for service in ["ai.assistant", "voice.polish"] {
                assert!(test_chat(service, &config(provider), &fake).ok);
            }
        }
        let bodies = fake.bodies.borrow();
        assert_eq!(bodies[0]["thinking"], json!({"type":"disabled"}));
        assert_eq!(bodies[2]["enable_thinking"], false);
        assert!(bodies[4].get("thinking").is_none());
        for body in bodies.iter() {
            assert_eq!(body["max_tokens"], 512);
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
            ("endpoint", "https:///chat"),
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
    fn ollama_chat_test_sends_no_token() {
        struct Recording(RefCell<Vec<String>>);
        impl ProbeTransport for Recording {
            fn post(&self, _: &str, token: &str, _: &Value) -> Result<u16, ProbeFailure> {
                self.0.borrow_mut().push(token.into());
                Ok(200)
            }
        }
        let transport = Recording(RefCell::default());
        let mut request = config("ollama");
        request["endpoint"] = json!("http://localhost:11434/v1/chat/completions");
        let tested = test_chat("ai.assistant", &request, &transport);
        assert!(tested.ok);
        request["token"] = json!("");
        assert!(test_chat("ai.assistant", &request, &transport).ok);
        // 即使请求里带着旧 Token 也不发；语音润色没有不鉴权的服务。
        assert_eq!(*transport.0.borrow(), ["", ""]);
        assert!(!test_chat("voice.polish", &request, &transport).ok);
        request["endpoint"] = json!("http://8.8.8.8/v1/chat/completions");
        assert!(!test_chat("ai.assistant", &request, &transport).ok);
        assert_eq!(transport.0.borrow().len(), 2);
    }
    #[test]
    fn credential_chat_accepts_local_network_http_only_for_ai_assistant() {
        struct Recording(RefCell<Vec<String>>);
        impl ProbeTransport for Recording {
            fn post(&self, endpoint: &str, _: &str, _: &Value) -> Result<u16, ProbeFailure> {
                self.0.borrow_mut().push(endpoint.into());
                Ok(200)
            }
        }
        let transport = Recording(RefCell::default());
        let mut request = config("openai");
        request["endpoint"] = json!("http://192.168.1.20:1234/v1/chat/completions");
        assert!(test_chat("ai.assistant", &request, &transport).ok);
        assert!(!test_chat("voice.polish", &request, &transport).ok);
        request["endpoint"] = json!("http://8.8.8.8/v1/chat/completions");
        assert!(!test_chat("ai.assistant", &request, &transport).ok);
        assert_eq!(
            *transport.0.borrow(),
            ["http://192.168.1.20:1234/v1/chat/completions"]
        );
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

    #[test]
    fn credential_chat_distinguishes_completion_from_truncated_thinking() {
        let completion = |content: Value, reasoning: &str, finish: &str| {
            json!({"choices": [{"message": {"content": content,
                "reasoning_content": reasoning}, "finish_reason": finish}]})
        };
        assert_eq!(
            validate_completion(&completion(json!("OK"), "synthetic reasoning", "stop")),
            Ok(())
        );
        for content in [Value::Null, json!(""), json!("partial")] {
            assert_eq!(
                validate_completion(&completion(content, "synthetic reasoning", "length")),
                Err(ProbeFailure::OutputLimit)
            );
        }
        for content in [
            Value::Null,
            json!(""),
            json!("   "),
            json!("x".repeat(16385)),
        ] {
            assert_eq!(
                validate_completion(&completion(content, "synthetic reasoning", "stop")),
                Err(ProbeFailure::InvalidResponse)
            );
        }
        let mut error = completion(json!("OK"), "", "stop");
        error["error"] = json!({"message": "synthetic private error"});
        assert_eq!(
            validate_completion(&error),
            Err(ProbeFailure::InvalidResponse)
        );
    }

    #[test]
    fn credential_probe_uses_only_documented_model_parameters() {
        for (model, effort, completion_limit) in [
            ("gpt-6-luna", Some("none"), true),
            ("openai/gpt-6-sol-2026-10-01", Some("none"), true),
            ("gpt-6-astra", Some("low"), true),
            ("gpt-6.1-sol", Some("low"), true),
            ("GLM-5.3-FLASH", Some("low"), true),
            ("gpt-4.1", None, false),
            ("synthetic-model", None, false),
        ] {
            let fake = Fake {
                status: Some(200),
                bodies: RefCell::default(),
            };
            let mut request = config("everyapi");
            request["model"] = json!(model);
            assert!(test_chat("ai.assistant", &request, &fake).ok);
            let bodies = fake.bodies.borrow();
            let body = &bodies[0];
            assert_eq!(body["model"], model);
            assert_eq!(body.get("reasoning_effort").and_then(Value::as_str), effort);
            let limit = if completion_limit {
                "max_completion_tokens"
            } else {
                "max_tokens"
            };
            let absent = if completion_limit {
                "max_tokens"
            } else {
                "max_completion_tokens"
            };
            assert_eq!(body[limit], 512);
            assert!(body.get(absent).is_none());
            assert!(body.get("thinking").is_none());
        }
    }

    #[test]
    fn credential_chat_reports_fixed_failure_categories() {
        struct Failing(ProbeFailure);
        impl ProbeTransport for Failing {
            fn post(&self, _: &str, _: &str, _: &Value) -> Result<u16, ProbeFailure> {
                Err(self.0)
            }
        }
        let failures = [
            (ProbeFailure::Connection, "连接失败"),
            (ProbeFailure::Timeout, "模型响应超时"),
            (ProbeFailure::InvalidResponse, "没有返回有效的对话正文"),
            (ProbeFailure::OutputLimit, "耗尽测试输出预算"),
        ];
        for (failure, explanation) in failures {
            let result = test_chat("ai.assistant", &config("everyapi"), &Failing(failure));
            assert!(!result.ok);
            assert!(result.message.contains(explanation));
            assert!(!result.message.contains("synthetic"));
            assert!(result.message.len() < 256);
        }
    }

    #[test]
    fn credential_chat_budget_allows_the_reasoning_server_to_finish() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let body_start;
            let body_len;
            loop {
                let mut buffer = [0; 1024];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
                assert!(request.len() <= 16384);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    body_start = end + 4;
                    body_len = std::str::from_utf8(&request[..end])
                        .unwrap()
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    break;
                }
            }
            while request.len() < body_start + body_len {
                let mut buffer = [0; 1024];
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
                assert!(request.len() <= 16384);
            }
            let body: Value =
                serde_json::from_slice(&request[body_start..body_start + body_len]).unwrap();
            // 合成服务先消耗思考预算，只有剩余预算足够时才返回正文。
            let enough = body["max_tokens"].as_u64().unwrap() > 32;
            let reply = json!({"choices":[{"message":{"reasoning_content":"synthetic reasoning",
                "content":if enough {"OK"} else {""}},
                "finish_reason":if enough {"stop"} else {"length"}}]})
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.len(),
                reply
            )
            .unwrap();
        });
        let mut request = config("everyapi");
        request["model"] = json!("synthetic-reasoning-model");
        request["endpoint"] = json!(endpoint);
        assert!(test_chat("ai.assistant", &request, &ChatProbeTransport).ok);
        server.join().unwrap();
    }

    #[test]
    fn credential_chat_rejects_success_status_without_a_completion() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            assert!(stream.read(&mut request).unwrap() > 0);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
        });
        let mut request = config("openai");
        request["endpoint"] = json!(endpoint);
        assert!(!test_chat("ai.assistant", &request, &ChatProbeTransport).ok);
        server.join().unwrap();
    }
}
