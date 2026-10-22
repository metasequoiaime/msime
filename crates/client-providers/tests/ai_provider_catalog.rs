use msime_client_core::preferences::AI_PROVIDERS;
use msime_client_providers::ai::provider_catalog::{ui_provider_catalog, PROVIDERS};
use std::collections::BTreeSet;

#[test]
fn catalog_covers_preferences_once_and_keeps_custom_as_the_fallback() {
    let ids: BTreeSet<_> = PROVIDERS.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), PROVIDERS.len());
    assert_eq!(ids, AI_PROVIDERS.into_iter().collect());
    let custom = PROVIDERS.last().unwrap();
    assert_eq!(custom.id, "custom");
    assert!(custom.endpoint.is_empty() && custom.model.is_empty());
    assert!(custom.models.is_empty() && custom.documentation.is_empty());
    for preset in PROVIDERS {
        assert!(!preset.title.is_empty());
        if !matches!(preset.id, "custom" | "ollama") {
            assert!(!preset.endpoint.is_empty() && !preset.model.is_empty());
            assert!(!preset.models.is_empty() && !preset.documentation.is_empty());
        }
    }
}

#[test]
fn empty_optional_metadata_keeps_the_existing_ui_object_shape() {
    let catalog = serde_json::to_value(ui_provider_catalog()).unwrap();
    let rows = catalog.as_array().unwrap();
    let ollama = rows.iter().find(|p| p["id"] == "ollama").unwrap();
    assert!(ollama.get("models").is_none());
    assert!(ollama.get("documentation").is_some());
    let custom = rows.last().unwrap();
    assert!(custom.get("models").is_none());
    assert!(custom.get("documentation").is_none());
}

#[test]
fn android_catalog_keeps_complete_fields_when_ui_omits_empty_metadata() {
    let android = serde_json::to_value(PROVIDERS).unwrap();
    for row in android.as_array().unwrap() {
        assert!(row["models"].is_array());
        assert!(row["documentation"].is_string());
        assert_eq!(row["desktop_only"], row["id"] == "ollama");
    }
    for row in serde_json::to_value(ui_provider_catalog())
        .unwrap()
        .as_array()
        .unwrap()
    {
        assert!(row.get("desktop_only").is_none());
    }
}
