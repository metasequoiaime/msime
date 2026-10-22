use msime_client_core::preferences::AI_PROVIDERS;
use msime_client_providers::ai::{
    endpoint,
    provider_catalog::{provider_update, AiProviderFields, PROVIDERS},
};

fn current(provider: &str, endpoint: &str, model: &str) -> AiProviderFields {
    AiProviderFields {
        enabled: true,
        provider: provider.into(),
        endpoint: endpoint.into(),
        model: model.into(),
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
fn custom_endpoint_and_model_survive_switches_including_custom_provider() {
    let original = current(
        "deepseek",
        "http://127.0.0.1:1234/v1/chat/completions",
        "synthetic-model",
    );
    for next in PROVIDERS {
        let patch = provider_update(next.id, &original).unwrap();
        assert_eq!(patch.endpoint, original.endpoint);
        assert_eq!(patch.model, original.model);
        assert!(patch.enabled);
    }
}

#[test]
fn endpoint_and_model_are_replaced_independently() {
    let deepseek = PROVIDERS
        .iter()
        .find(|entry| entry.id == "deepseek")
        .unwrap();
    let patch = provider_update(
        "openai",
        &current("deepseek", deepseek.endpoint, "synthetic-model"),
    )
    .unwrap();
    assert_eq!(patch.endpoint, "https://api.openai.com/v1/chat/completions");
    assert_eq!(patch.model, "synthetic-model");
    let patch = provider_update(
        "openai",
        &current("deepseek", "https://fixture.invalid/v1", deepseek.model),
    )
    .unwrap();
    assert_eq!(patch.endpoint, "https://fixture.invalid/v1");
    assert_eq!(patch.model, "gpt-4.1-mini");
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
        let patch =
            provider_update("openai", &current("custom", endpoint, "synthetic-model")).unwrap();
        assert_eq!(patch.endpoint, endpoint);
        assert!(!patch.enabled);
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
