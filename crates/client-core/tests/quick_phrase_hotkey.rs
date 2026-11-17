use msime_client_core::preferences::{
    Preferences, PreferencesStore, ToggleChordOwner, ToggleHotkeyRejection, ToggleHotkeySlot,
};

#[test]
fn quick_phrase_hotkey_is_opt_in_and_normalized_without_changing_legacy_defaults() {
    let defaults = Preferences::default();
    assert!(defaults.keybindings.enter_quick_phrase.is_empty());
    let value = serde_json::to_value(&defaults).unwrap();
    assert!(value["keybindings"].get("enter_quick_phrase").is_none());
    assert_eq!(
        serde_json::from_value::<Preferences>(value).unwrap(),
        defaults
    );
    let root = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(root.path());
    let snapshot = store.load().unwrap();
    let mut prefs = snapshot.preferences;
    prefs.keybindings.enter_quick_phrase = "control + keyk".into();
    store.save(snapshot.revision, prefs).unwrap();
    let saved = store.load().unwrap();
    assert_eq!(
        saved.preferences.keybindings.enter_quick_phrase,
        "Ctrl+KeyK"
    );
    assert_eq!(saved.preferences.local_modes, defaults.local_modes);
    let mut prefs = saved.preferences;
    prefs.keybindings.enter_quick_phrase.clear();
    store.save(saved.revision, prefs).unwrap();
    assert!(store
        .load()
        .unwrap()
        .preferences
        .keybindings
        .enter_quick_phrase
        .is_empty());
}

#[test]
fn quick_phrase_hotkey_rejects_typing_keys_and_conflicts_in_both_directions() {
    let mut prefs = Preferences::default();
    for invalid in ["KeyK", "Shift+KeyK", "Ctrl+Unknown"] {
        assert!(prefs
            .keybindings
            .check_toggle_hotkey(ToggleHotkeySlot::QuickPhrase, invalid, true)
            .is_err());
        prefs.keybindings.enter_quick_phrase = invalid.into();
        assert!(prefs.validate().is_err());
    }
    prefs.keybindings.enter_quick_phrase = "Ctrl+Period".into();
    assert!(prefs.validate().is_err());
    prefs.keybindings.enter_quick_phrase = "Ctrl+KeyK".into();
    assert!(prefs.validate().is_ok());
    assert!(matches!(
        prefs
            .keybindings
            .check_toggle_hotkey(ToggleHotkeySlot::VoiceInput, "Ctrl+KeyK", true),
        Err(ToggleHotkeyRejection::Conflict {
            owner: ToggleChordOwner::QuickPhrase,
            ..
        })
    ));
    prefs.keybindings.toggle_voice_input = "Ctrl+KeyK".into();
    assert!(prefs.validate().is_err());
}
