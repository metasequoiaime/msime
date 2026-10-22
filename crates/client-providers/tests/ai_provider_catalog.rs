use msime_client_core::preferences::AI_PROVIDERS;
use msime_client_providers::ai::provider_catalog::AI_PROVIDER_PRESETS;
use std::collections::BTreeSet;

#[test]
fn catalog_covers_preferences_once_and_keeps_custom_as_the_fallback() {
    let ids: BTreeSet<_> = AI_PROVIDER_PRESETS.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), AI_PROVIDER_PRESETS.len());
    assert_eq!(ids, AI_PROVIDERS.into_iter().collect());
    let custom = AI_PROVIDER_PRESETS.last().unwrap();
    assert_eq!(custom.id, "custom");
    assert!(custom.endpoint.is_empty() && custom.model.is_empty());
    assert!(custom.models.is_empty() && custom.documentation.is_empty());
    for preset in AI_PROVIDER_PRESETS {
        assert!(!preset.title.is_empty());
        if !matches!(preset.id, "custom" | "ollama") {
            assert!(!preset.endpoint.is_empty() && !preset.model.is_empty());
            assert!(!preset.models.is_empty() && !preset.documentation.is_empty());
        }
    }
}

#[test]
fn empty_optional_metadata_keeps_the_existing_ui_object_shape() {
    let catalog = serde_json::to_value(AI_PROVIDER_PRESETS).unwrap();
    let rows = catalog.as_array().unwrap();
    let ollama = rows.iter().find(|p| p["id"] == "ollama").unwrap();
    assert!(ollama.get("models").is_none());
    assert!(ollama.get("documentation").is_some());
    let custom = rows.last().unwrap();
    assert!(custom.get("models").is_none());
    assert!(custom.get("documentation").is_none());
}
