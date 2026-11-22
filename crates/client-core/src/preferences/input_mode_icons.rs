use super::{Preferences, PreferencesSnapshot, PreferencesStore};
use serde_json::json;

#[test]
fn new_and_legacy_documents_keep_brand_icons() {
    assert!(!Preferences::default().input_mode_glyph_icons);
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("input_mode_glyph_icons");
    let restored: PreferencesSnapshot = serde_json::from_value(document).unwrap();
    assert!(!restored.preferences.input_mode_glyph_icons);
}

#[test]
fn icon_choice_requires_a_json_boolean() {
    for invalid in [
        json!(null),
        json!(0),
        json!(1),
        json!("true"),
        json!([]),
        json!({}),
    ] {
        let mut document = serde_json::to_value(Preferences::default()).unwrap();
        document["input_mode_glyph_icons"] = invalid;
        assert!(serde_json::from_value::<Preferences>(document).is_err());
    }
}

#[test]
fn icon_choice_survives_restart_and_preserves_other_preferences() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["preferences"]["future_icon_option"] = json!({"keep": "synthetic"});
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();

    for enabled in [true, false] {
        let store = PreferencesStore::new(directory.path());
        let mut snapshot = store.load().unwrap();
        snapshot.preferences.input_mode_glyph_icons = enabled;
        store.save(snapshot.revision, snapshot.preferences).unwrap();
        let restarted = PreferencesStore::new(directory.path()).load().unwrap();
        assert_eq!(restarted.preferences.input_mode_glyph_icons, enabled);
        assert!(!restarted.preferences.input_mode_status_item);
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            persisted["preferences"]["future_icon_option"],
            document["preferences"]["future_icon_option"]
        );
    }
}
