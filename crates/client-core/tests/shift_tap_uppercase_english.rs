use msime_client_core::preferences::{KeybindingPreferences, Preferences, PreferencesStore};

#[test]
fn shift_tap_uppercase_english_defaults_off_and_round_trips() {
    let missing: KeybindingPreferences = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(!missing.shift_tap_uppercase_english);
    assert!(!KeybindingPreferences::default().shift_tap_uppercase_english);
    assert!(
        !Preferences::default()
            .keybindings
            .shift_tap_uppercase_english
    );
    for enabled in [true, false] {
        let preferences: KeybindingPreferences = serde_json::from_value(serde_json::json!({
            "shift_tap_uppercase_english": enabled
        }))
        .unwrap();
        let saved = serde_json::to_value(&preferences).unwrap();
        assert_eq!(saved["shift_tap_uppercase_english"], enabled);
        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        store
            .save(
                0,
                Preferences {
                    keybindings: serde_json::from_value(saved).unwrap(),
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
                .shift_tap_uppercase_english,
            enabled
        );
    }
}
