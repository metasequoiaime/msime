use msime_client_providers::voice::provider_settings::{
    provider_update, ProviderFields, ProviderService,
};
use serde_json::{json, Value};

fn update(service: ProviderService, provider: &str, current: Value) -> Value {
    serde_json::to_value(provider_update(
        service,
        provider,
        &serde_json::from_value::<ProviderFields>(current).unwrap(),
    ))
    .unwrap()
}

#[test]
fn recognition_switch_replaces_defaults_and_restores_the_target_slot() {
    let patch = update(
        ProviderService::Asr,
        "openai",
        json!({
            "provider":"doubao", "endpoint":"wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async",
            "model":"", "token":"synthetic-doubao", "tokens":{"openai":"synthetic-openai"}
        }),
    );
    assert_eq!(
        patch,
        json!({
            "provider":"openai", "endpoint":"https://api.openai.com/v1/audio/transcriptions",
            "model":"whisper-1", "token":"synthetic-openai",
            "tokens":{"doubao":"synthetic-doubao", "openai":"synthetic-openai"}
        })
    );
}

#[test]
fn offline_recognition_clears_default_fields_and_its_credential_slot() {
    for provider in ["system", "local"] {
        let patch = update(
            ProviderService::Asr,
            provider,
            json!({
                "provider":"openai", "endpoint":"https://api.openai.com/v1/audio/transcriptions",
                "model":"whisper-1", "token":"synthetic-openai", "tokens":{provider:"synthetic-unused"}
            }),
        );
        assert_eq!(
            patch,
            json!({
                "provider":provider, "endpoint":"", "model":"", "token":"",
                "tokens":{"openai":"synthetic-openai"}
            })
        );
        let restored = update(ProviderService::Asr, "openai", patch);
        assert_eq!(restored["token"], "synthetic-openai");
        assert_eq!(restored["model"], "whisper-1");
    }
}

#[test]
fn custom_fields_are_omitted_from_the_patch_even_for_offline_recognition() {
    for provider in ["system", "local", "groq"] {
        let patch = update(
            ProviderService::Asr,
            provider,
            json!({
                "provider":"openai", "endpoint":" https://example.invalid/custom ",
                "model":" synthetic-model "
            }),
        );
        assert!(patch.get("endpoint").is_none());
        assert!(patch.get("model").is_none());
    }
}

#[test]
fn every_shipped_recognition_default_can_be_replaced() {
    use msime_client_providers::voice::provider_catalog::ASR_PROVIDERS;
    for preset in ASR_PROVIDERS {
        let patch = update(
            ProviderService::Asr,
            "openai",
            json!({
                "endpoint":format!("\u{feff}\t{}\u{3000}",preset.endpoint), "model":preset.model
            }),
        );
        assert_eq!(
            patch["endpoint"],
            "https://api.openai.com/v1/audio/transcriptions"
        );
        assert_eq!(patch["model"], "whisper-1");
    }
}

#[test]
fn trimming_preserves_javascript_whitespace_rules() {
    for whitespace in [
        "\t\n\u{b}\u{c}\r ",
        "\u{a0}\u{1680}\u{2000}\u{200a}",
        "\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}",
    ] {
        let patch = update(
            ProviderService::Asr,
            "openai",
            json!({"endpoint":whitespace,"model":whitespace}),
        );
        assert_eq!(patch["model"], "whisper-1");
        assert!(patch.get("endpoint").is_some());
    }
    for untrimmed in ["\u{85}", "\u{180e}", "\u{200b}"] {
        let patch = update(ProviderService::Asr, "openai", json!({"model":untrimmed}));
        assert!(patch.get("model").is_none());
    }
}

#[test]
fn empty_token_removes_the_source_slot_and_whitespace_token_is_preserved() {
    let patch = update(
        ProviderService::Asr,
        "groq",
        json!({
            "provider":"openai", "token":"", "tokens":{"openai":"synthetic-old","groq":"synthetic-groq"}
        }),
    );
    assert_eq!(patch["tokens"], json!({"groq":"synthetic-groq"}));
    assert_eq!(patch["token"], "synthetic-groq");
    let patch = update(
        ProviderService::Asr,
        "openai",
        json!({"provider":"openai","token":" \t"}),
    );
    assert_eq!(patch["token"], " \t");
    assert_eq!(patch["tokens"]["openai"], " \t");
}

#[test]
fn switching_to_the_same_provider_uses_the_current_box() {
    let patch = update(
        ProviderService::Asr,
        "openai",
        json!({
            "provider":"openai", "token":"synthetic-new", "tokens":{"openai":"synthetic-old"}
        }),
    );
    assert_eq!(patch["token"], "synthetic-new");
    assert_eq!(patch["tokens"]["openai"], "synthetic-new");
}

#[test]
fn unknown_provider_only_swaps_tokens() {
    let patch = update(
        ProviderService::Asr,
        "unknown",
        json!({
            "provider":"openai", "token":"synthetic-openai", "endpoint":"", "model":""
        }),
    );
    assert_eq!(
        patch,
        json!({"provider":"unknown","token":"","tokens":{"openai":"synthetic-openai"}})
    );
}

#[test]
fn polish_switch_uses_only_the_settings_catalog_and_keeps_custom_models() {
    let patch = update(
        ProviderService::Polish,
        "deepseek",
        json!({
            "provider":"siliconflow", "endpoint":"https://api.siliconflow.cn/v1/chat/completions",
            "model":"Qwen/Qwen3-8B", "token":"synthetic-siliconflow"
        }),
    );
    assert_eq!(
        patch["endpoint"],
        "https://api.deepseek.com/chat/completions"
    );
    assert_eq!(patch["model"], "deepseek-v4-flash");
    assert_eq!(patch["tokens"]["siliconflow"], "synthetic-siliconflow");
    let patch = update(
        ProviderService::Polish,
        "groq",
        json!({"model":"synthetic-custom"}),
    );
    assert!(patch.get("model").is_none());
    // 运行时兼容的旧服务不额外加入设置目录，切换规则与原生页既有选项一致。
    let patch = update(ProviderService::Polish, "everyapi", json!({}));
    assert!(patch.get("endpoint").is_none());
    assert!(patch.get("model").is_none());
}

#[test]
fn fields_default_to_empty_and_the_source_snapshot_is_unchanged() {
    let current: ProviderFields = serde_json::from_value(json!({})).unwrap();
    let _patch = provider_update(ProviderService::Asr, "openai", &current);
    assert!(current.provider.is_empty());
    assert!(current.token.is_empty());
    assert!(current.tokens.is_empty());
    assert_eq!(
        update(
            ProviderService::Asr,
            "local",
            json!({"tokens":{"openai":"synthetic-old"},"token":"synthetic-orphan"})
        )["tokens"],
        json!({"openai":"synthetic-old"})
    );
}
