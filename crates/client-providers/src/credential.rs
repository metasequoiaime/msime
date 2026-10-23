//! Validation and probing for the third-party service credentials the user
//! supplies. Nothing here stores or logs a secret; the probes report only
//! whether a credential was accepted.

pub mod asr;
pub mod doubao;
pub mod doubao_auth;
pub mod probe;
pub mod translation;

pub(crate) fn is_all_asterisks(value: &str) -> bool {
    value.chars().all(|character| character == '*')
}

pub(crate) fn usable_token(value: &str) -> bool {
    !value.is_empty()
        && crate::text::is_bounded_text(value, 8192)
        && !value.starts_with('<')
        && !is_all_asterisks(value)
}

pub(crate) fn valid_https_endpoint_and_model(endpoint: &str, model: &str) -> bool {
    if endpoint.len() > 2048 || model.is_empty() || !crate::text::is_bounded_text(model, 256) {
        return false;
    }
    reqwest::Url::parse(endpoint)
        .ok()
        .is_some_and(|url| crate::endpoint::valid_endpoint_shape(endpoint, &url, "https"))
}

/// 设置页「测试配置」按钮背后的检查，给自己发请求的宿主用（Tauri 设置在 Windows 和 macOS 上，以及原生 Windows 设置窗口经 `msime_client_test_api_credential`）：`voice.asr` 是豆包时走它的 WebSocket，其余识别服务走 HTTP 上传；`translation.tencent`、`translation.niutrans`、`translation.custom` 各发一次翻译；`ai.assistant` 和 `voice.polish` 发一次最短的对话。`config` 是设置页给的同一份配置。配置不完整时直接回答、不联网；否则会联网，最长约 15 秒，不要在界面线程上调用。
pub fn test_service(service: &str, config: &serde_json::Value) -> probe::ProbeResult {
    if service == "voice.asr" {
        if config.get("provider").and_then(serde_json::Value::as_str) == Some("doubao") {
            return doubao::test(config, &doubao::WebSocketTransport);
        }
        return asr::test(config, &asr::HttpTransport);
    }
    // 「AI 辅助」翻译用的是同一套 Chat Completions 凭据，测试与 ai.assistant 走同一条探测。
    if service == "translation.ai" {
        return probe::test_chat("ai.assistant", config, &probe::ChatProbeTransport);
    }
    if service.starts_with("translation.") {
        let milliseconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
            .unwrap_or(0);
        return translation::test(service, config, milliseconds, &translation::HttpTransport);
    }
    probe::test_chat(service, config, &probe::ChatProbeTransport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // 只用不完整的配置：每一种都在联网之前就回答。
    #[test]
    fn test_service_answers_incomplete_configurations_without_the_network() {
        for service in [
            "translation.tencent",
            "translation.niutrans",
            "translation.custom",
        ] {
            let result = test_service(service, &json!({}));
            assert!(!result.ok, "{service}");
            assert_eq!(result.message, "请填写有效的翻译服务凭据和接口地址。");
        }
        let custom = test_service(
            "translation.custom",
            &json!({"endpoint": "http://translate.example/api", "api_key": ""}),
        );
        assert_eq!(custom.message, "请填写有效的翻译服务凭据和接口地址。");
        let unknown = test_service("unknown.service", &json!({}));
        assert!(!unknown.ok);
        assert_eq!(unknown.message, "此配置测试类型尚未接入。");
        let chat = test_service("ai.assistant", &json!({"token": ""}));
        assert_eq!(chat.message, "请先填写有效的 API Key。");
        let asr = test_service("voice.asr", &json!({"provider": "unknown"}));
        assert_eq!(asr.message, "此识别服务的凭据测试尚未接入。");
    }
}
