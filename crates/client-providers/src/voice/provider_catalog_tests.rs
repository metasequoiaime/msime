use super::provider::{asr_provider_defaults, asr_request_format, polish_provider_defaults};
use super::provider_catalog::{ASR_PROVIDERS, POLISH_PROVIDERS};

#[test]
fn voice_catalog_preserves_settings_order_and_uses_runtime_defaults() {
    assert_eq!(
        ASR_PROVIDERS
            .iter()
            .map(|preset| preset.id)
            .collect::<Vec<_>>(),
        [
            "doubao",
            "siliconflow",
            "openai",
            "groq",
            "everyapi",
            "mistral",
            "bailian",
            "zhipu",
            "minimax",
            "custom"
        ]
    );
    assert_eq!(
        POLISH_PROVIDERS
            .iter()
            .map(|preset| preset.id)
            .collect::<Vec<_>>(),
        [
            "siliconflow",
            "openai",
            "deepseek",
            "groq",
            "minimax",
            "kimi",
            "zhipu",
            "qwen",
            "volcengine",
            "openrouter",
            "custom"
        ]
    );
    for (presets, defaults) in [
        (ASR_PROVIDERS, asr_provider_defaults as fn(&str) -> _),
        (POLISH_PROVIDERS, polish_provider_defaults as fn(&str) -> _),
    ] {
        let mut ids = std::collections::BTreeSet::new();
        for preset in presets {
            assert!(ids.insert(preset.id), "duplicate {}", preset.id);
            assert!(!preset.title.is_empty());
            assert_eq!(defaults(preset.id), Some((preset.endpoint, preset.model)));
            // 自定义服务没有预设地址和接入文档，全部由用户填写。
            if preset.id != "custom" {
                assert!(preset.documentation.starts_with("https://"));
            }
            assert!(preset.model.is_empty() || preset.models.contains(&preset.model));
        }
    }
    for provider in ["system", "local", "unknown"] {
        assert_eq!(asr_provider_defaults(provider), None);
    }
    // 兼容已有配置中的润色服务，但不向设置页新增选项；dashscope/moonshot 是
    // Windows 目录里的旧 id，与目录中的 qwen/kimi 指向同一服务，同样只解析不列出。
    for provider in ["everyapi", "mistral", "dashscope", "moonshot"] {
        assert!(polish_provider_defaults(provider).is_some());
        assert!(!POLISH_PROVIDERS.iter().any(|preset| preset.id == provider));
    }
}

/// 新增识别服务的端点与模型取自 Windows v0.11.1 `DefaultAsrEndpoint` /
/// `DefaultAsrModel`（server/src/voice-input/voice_providers.cpp），三家都按
/// multipart 上传：智谱与 MiniMax 官方文档确认各自的 OpenAI 兼容表单形状，
/// 自定义服务沿用同一路径由用户填地址。
#[test]
fn new_asr_presets_match_the_reference_endpoints() {
    let find = |id| ASR_PROVIDERS.iter().find(|preset| preset.id == id).unwrap();
    let zhipu = find("zhipu");
    assert_eq!(
        zhipu.endpoint,
        "https://open.bigmodel.cn/api/paas/v4/audio/transcriptions"
    );
    assert_eq!(zhipu.model, "glm-asr");
    let minimax = find("minimax");
    assert_eq!(minimax.endpoint, "https://api.minimax.cn/v1/speech_to_text");
    assert_eq!(minimax.model, "asr-1.0");
    let custom = find("custom");
    assert!(custom.endpoint.is_empty() && custom.model.is_empty());
    for provider in ["zhipu", "minimax", "custom"] {
        assert_eq!(
            asr_request_format(provider),
            Some("multipart"),
            "{provider}"
        );
    }
}

/// 新增润色服务的端点与模型取自 Windows v0.11.1 `DefaultPolishEndpoint` /
/// `DefaultPolishModel`（同上文件）；kimi/qwen 沿用 AI 目录的 id，
/// 对应 Windows 的 moonshot/dashscope。
#[test]
fn new_polish_presets_match_the_reference_endpoints() {
    let find = |id| {
        POLISH_PROVIDERS
            .iter()
            .find(|preset| preset.id == id)
            .unwrap()
    };
    let cases = [
        (
            "minimax",
            "https://api.minimax.cn/v1/chat/completions",
            "MiniMax-M3.1-Flash",
        ),
        (
            "kimi",
            "https://api.moonshot.cn/v1/chat/completions",
            "kimi-k2-0905-preview",
        ),
        (
            "zhipu",
            "https://open.bigmodel.cn/api/paas/v4/chat/completions",
            "glm-4-flash",
        ),
        (
            "qwen",
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions",
            "qwen-plus",
        ),
        (
            "volcengine",
            "https://ark.cn-beijing.volces.com/api/v3/chat/completions",
            "doubao-seed-1.6-flash",
        ),
        (
            "openrouter",
            "https://openrouter.ai/api/v1/chat/completions",
            "openrouter/auto",
        ),
    ];
    for (id, endpoint, model) in cases {
        let preset = find(id);
        assert_eq!(preset.endpoint, endpoint, "{id}");
        assert_eq!(preset.model, model, "{id}");
    }
    let custom = find("custom");
    assert!(custom.endpoint.is_empty() && custom.model.is_empty());
    // 旧 id 解析到与目录项相同的默认地址和模型。
    for (legacy, current) in [("dashscope", "qwen"), ("moonshot", "kimi")] {
        assert_eq!(
            polish_provider_defaults(legacy),
            Some((find(current).endpoint, find(current).model)),
            "{legacy}"
        );
    }
}

#[test]
fn voice_catalog_serializes_complete_presets_without_credentials() {
    let preset = serde_json::to_value(&ASR_PROVIDERS[0]).unwrap();
    assert_eq!(
        preset,
        serde_json::json!({
            "id": "doubao",
            "title": "豆包",
            "endpoint": "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
            "model": "",
            "models": [],
            "documentation": "https://www.volcengine.com/docs/6561/1354869"
        })
    );
}
