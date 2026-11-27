use msime_client_core::preferences::{KeybindingPreferences, Preferences, PreferencesStore};

#[test]
fn symbol_panel_left_option_hold_defaults_off_and_round_trips() {
    let missing: KeybindingPreferences = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(!missing.symbol_panel_left_option_hold);
    assert!(!KeybindingPreferences::default().symbol_panel_left_option_hold);
    assert!(
        !Preferences::default()
            .keybindings
            .symbol_panel_left_option_hold
    );
    for enabled in [true, false] {
        let preferences: KeybindingPreferences = serde_json::from_value(serde_json::json!({
            "symbol_panel_left_option_hold": enabled
        }))
        .unwrap();
        let saved = serde_json::to_value(&preferences).unwrap();
        assert_eq!(saved["symbol_panel_left_option_hold"], enabled);
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
                .symbol_panel_left_option_hold,
            enabled
        );
    }
}
