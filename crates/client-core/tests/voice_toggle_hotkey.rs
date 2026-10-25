use msime_client_core::preferences::{
    Preferences, PreferencesError, PreferencesStore, ToggleChordOwner, ToggleHotkeyRejection,
    ToggleHotkeySlot, TOGGLE_PUNCTUATION_DEFAULT,
};

#[test]
fn voice_toggle_hotkey_defaults_unbound_and_persists_normalized_changes() {
    let defaults = Preferences::default();
    assert!(defaults.keybindings.voice_input_chord().is_none());
    let legacy = serde_json::to_value(&defaults).unwrap();
    assert!(legacy["keybindings"].get("toggle_voice_input").is_none());
    assert_eq!(
        serde_json::from_value::<Preferences>(legacy).unwrap(),
        defaults
    );
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    let mut preferences = snapshot.preferences;
    preferences.keybindings.toggle_voice_input = " option + control + f8 ".into();
    store.save(snapshot.revision, preferences).unwrap();
    let saved = store.load().unwrap();
    assert_eq!(
        saved.preferences.keybindings.toggle_voice_input,
        "Ctrl+Alt+F8"
    );
    assert_eq!(saved.preferences.voice_input, defaults.voice_input);
    assert_eq!(
        saved.preferences.keybindings.toggle_punctuation,
        defaults.keybindings.toggle_punctuation
    );
    let mut preferences = saved.preferences;
    preferences.keybindings.toggle_voice_input.clear();
    store.save(saved.revision, preferences).unwrap();
    assert!(store
        .load()
        .unwrap()
        .preferences
        .keybindings
        .voice_input_chord()
        .is_none());
}

#[test]
fn voice_toggle_hotkey_checks_conflicts_in_both_directions() {
    let mut preferences = Preferences::default();
    for chord in [
        "Ctrl+Period",
        "Ctrl+F9",
        "Ctrl+Shift+F9",
        "Ctrl+Alt+Shift+KeyR",
    ] {
        preferences.keybindings.toggle_voice_input = chord.into();
        assert!(preferences
            .keybindings
            .check_toggle_hotkey(ToggleHotkeySlot::VoiceInput, chord, true)
            .is_err());
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::ConflictingToggleHotkeys)
        ));
    }
    preferences.keybindings.toggle_voice_input = "Ctrl+Alt+F8".into();
    assert_eq!(
        preferences.keybindings.check_toggle_hotkey(
            ToggleHotkeySlot::Punctuation,
            "control+option+f8",
            true
        ),
        Err(ToggleHotkeyRejection::Conflict {
            owner: ToggleChordOwner::VoiceInput,
            chord: "Ctrl+Alt+F8".parse().unwrap(),
        })
    );
    preferences.keybindings.toggle_punctuation = "Ctrl+Alt+F8".into();
    assert!(matches!(
        preferences.validate(),
        Err(PreferencesError::ConflictingToggleHotkeys)
    ));
    preferences.keybindings.toggle_punctuation = TOGGLE_PUNCTUATION_DEFAULT.into();
    preferences.keybindings.toggle_voice_input = "Ctrl+Banana".into();
    assert!(matches!(
        preferences.validate(),
        Err(PreferencesError::InvalidToggleHotkey)
    ));
    preferences.keybindings.toggle_voice_input = "Ctrl+F9".into();
    preferences.voice_input.hotkey_ctrl_f9 = false;
    assert!(preferences.validate().is_ok());
}
