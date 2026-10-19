use super::provider::{asr_provider_defaults, polish_provider_defaults};
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
            "bailian"
        ]
    );
    assert_eq!(
        POLISH_PROVIDERS
            .iter()
            .map(|preset| preset.id)
            .collect::<Vec<_>>(),
        ["siliconflow", "openai", "deepseek", "groq"]
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
            assert!(preset.documentation.starts_with("https://"));
            assert!(preset.model.is_empty() || preset.models.contains(&preset.model));
        }
    }
    for provider in ["system", "local", "unknown"] {
        assert_eq!(asr_provider_defaults(provider), None);
    }
    // 兼容已有配置中的润色服务，但不向设置页新增选项。
    for provider in ["everyapi", "mistral"] {
        assert!(polish_provider_defaults(provider).is_some());
        assert!(!POLISH_PROVIDERS.iter().any(|preset| preset.id == provider));
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
