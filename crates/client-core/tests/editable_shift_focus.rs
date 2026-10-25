use msime_client_core::preferences::{KeybindingPreferences, Preferences, PreferencesStore};

#[test]
fn editable_shift_focus_defaults_off_and_round_trips() {
    let missing: KeybindingPreferences = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(!missing.switch_language_shift_requires_editable);
    assert!(!KeybindingPreferences::default().switch_language_shift_requires_editable);
    for enabled in [true, false] {
        let preferences: KeybindingPreferences = serde_json::from_value(serde_json::json!({
            "switch_language_shift_requires_editable": enabled
        }))
        .unwrap();
        let saved = serde_json::to_value(&preferences).unwrap();
        assert_eq!(saved["switch_language_shift_requires_editable"], enabled);
        let reopened: KeybindingPreferences = serde_json::from_value(saved).unwrap();
        assert_eq!(reopened.switch_language_shift_requires_editable, enabled);
        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        store
            .save(
                0,
                Preferences {
                    keybindings: reopened,
                    ..Preferences::default()
                },
            )
            .unwrap();
        assert_eq!(
            store
                .load()
                .unwrap()
                .preferences
                .keybindings
                .switch_language_shift_requires_editable,
            enabled
        );
    }
}
