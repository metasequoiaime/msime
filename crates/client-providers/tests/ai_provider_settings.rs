use msime_client_core::preferences::AI_PROVIDERS;
use msime_client_providers::ai::{
    endpoint,
    provider_catalog::{provider_update, AiProviderFields, PROVIDERS},
};
use std::collections::BTreeMap;

fn current(provider: &str, endpoint: &str, model: &str) -> AiProviderFields {
    AiProviderFields {
        enabled: true,
        provider: provider.into(),
        endpoint: endpoint.into(),
        model: model.into(),
        endpoints: BTreeMap::new(),
        models: BTreeMap::new(),
    }
}

#[test]
fn catalog_covers_the_runtime_allowlist_and_only_offers_valid_defaults() {
    assert_eq!(
        PROVIDERS.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        AI_PROVIDERS
    );
    for entry in PROVIDERS {
        assert!(!entry.title.is_empty());
        assert_eq!(entry.desktop_only, entry.id == "ollama");
        if entry.id == "custom" {
            assert!(entry.endpoint.is_empty() && entry.model.is_empty());
        } else {
            assert!(endpoint::validate(entry.endpoint).is_ok(), "{}", entry.id);
            assert!(entry.documentation.starts_with("https://"));
            if entry.id != "ollama" {
                assert!(entry.models.contains(&entry.model), "{}", entry.id);
            }
        }
    }
}

#[test]
fn switching_every_preset_replaces_default_fields_and_keeps_ai_enabled() {
    for previous in PROVIDERS {
        for next in PROVIDERS {
            let patch = provider_update(
                next.id,
                &current(previous.id, previous.endpoint, previous.model),
            )
            .unwrap();
            assert_eq!(patch.provider, next.id);
            assert_eq!(patch.endpoint, next.endpoint);
            assert_eq!(patch.model, next.model);
            assert_eq!(patch.enabled, next.id != "custom");
        }
    }
}

#[test]
fn selecting_the_initial_provider_fills_the_mobile_empty_defaults() {
    let patch = provider_update("deepseek", &current("deepseek", "", "")).unwrap();
    assert_eq!(patch.endpoint, "https://api.deepseek.com/chat/completions");
    assert!(!patch.model.is_empty());
    assert!(patch.enabled);
}

#[test]
fn custom_endpoint_and_model_return_through_the_slots() {
    // A8：切走时自定义值进原提供方的槽位，切回来按槽位回填；默认值不落槽位。
    let custom = current(
        "deepseek",
        "https://llm.internal.example/v1/chat/completions",
        "my-model",
    );
    let openai = PROVIDERS.iter().find(|entry| entry.id == "openai").unwrap();
    let patch = provider_update("openai", &custom).unwrap();
    assert_eq!(patch.endpoint, openai.endpoint);
    assert_eq!(patch.model, openai.model);
    assert_eq!(
        patch.endpoints.get("deepseek").map(String::as_str),
        Some("https://llm.internal.example/v1/chat/completions")
    );
    assert_eq!(
        patch.models.get("deepseek").map(String::as_str),
        Some("my-model")
    );
    let back = provider_update("deepseek", &patch).unwrap();
    assert_eq!(
        back.endpoint,
        "https://llm.internal.example/v1/chat/completions"
    );
    assert_eq!(back.model, "my-model");
    // 目录默认值不写槽位。
    let plain = provider_update("kimi", &back).unwrap();
    assert!(!plain.endpoints.contains_key("openai"));
}

#[test]
fn custom_endpoint_and_model_survive_switches_including_custom_provider() {
    let original = current(
        "deepseek",
        "https://llm.example.com/v1/chat/completions",
        "synthetic-model",
    );
    for next in PROVIDERS {
        // 切去别的提供方：现值进 deepseek 的槽位，新提供方用自己的默认值；切回来原样返回。
        let patch = provider_update(next.id, &original).unwrap();
        assert_eq!(
            patch.endpoints.get("deepseek").map(String::as_str),
            Some(original.endpoint.as_str())
        );
        assert_eq!(
            patch.models.get("deepseek").map(String::as_str),
            Some(original.model.as_str())
        );
        let back = provider_update("deepseek", &patch).unwrap();
        assert_eq!(back.endpoint, original.endpoint);
        assert_eq!(back.model, original.model);
        // custom 中间态的 enabled 本来就假（它的默认端点为空）；恢复的地址合法时 enabled 照中间态保持。
        assert_eq!(back.enabled, patch.enabled);
    }
}

#[test]
fn endpoint_and_model_are_replaced_independently() {
    let deepseek = PROVIDERS
        .iter()
        .find(|entry| entry.id == "deepseek")
        .unwrap();
    // 接口地址是默认值、模型是自定义：只有模型进槽位，两项都按新提供方回填。
    let patch = provider_update(
        "openai",
        &current("deepseek", deepseek.endpoint, "synthetic-model"),
    )
    .unwrap();
    assert_eq!(patch.endpoint, "https://api.openai.com/v1/chat/completions");
    assert_eq!(patch.model, "gpt-4.1-mini");
    assert_eq!(
        patch.models.get("deepseek").map(String::as_str),
        Some("synthetic-model")
    );
    assert!(!patch.endpoints.contains_key("deepseek"));
    let patch = provider_update(
        "openai",
        &current("deepseek", "https://fixture.invalid/v1", deepseek.model),
    )
    .unwrap();
    assert_eq!(patch.endpoint, "https://api.openai.com/v1/chat/completions");
    assert_eq!(patch.model, "gpt-4.1-mini");
    assert_eq!(
        patch.endpoints.get("deepseek").map(String::as_str),
        Some("https://fixture.invalid/v1")
    );
    assert!(!patch.models.contains_key("deepseek"));
}

#[test]
fn switches_do_not_enable_a_disabled_assistant_or_keep_an_unsafe_endpoint_enabled() {
    let mut original = current("deepseek", "", "");
    original.enabled = false;
    assert!(!provider_update("openai", &original).unwrap().enabled);
    for endpoint in [
        "http://fixture.invalid/v1",
        "file:///fixture",
        "https://fixture.invalid/v1#fragment",
    ] {
        // 不合规的地址留在 custom 的槽位里：切去 openai 用它的默认值，切回 custom 仍恢复这个地址并保持禁用。
        let patch =
            provider_update("openai", &current("custom", endpoint, "synthetic-model")).unwrap();
        assert_eq!(
            patch.endpoints.get("custom").map(String::as_str),
            Some(endpoint)
        );
        let back = provider_update("custom", &patch).unwrap();
        assert_eq!(back.endpoint, endpoint);
        assert!(!back.enabled);
    }
}

#[test]
fn invalid_fields_are_rejected_without_building_a_patch() {
    assert!(provider_update("unknown", &current("deepseek", "", "")).is_none());
    for invalid in [
        current("deepseek", "", &"x".repeat(257)),
        current("deepseek", &"x".repeat(4097), ""),
        current("deepseek", "", "synthetic\nmodel"),
    ] {
        assert!(provider_update("openai", &invalid).is_none());
    }
}
