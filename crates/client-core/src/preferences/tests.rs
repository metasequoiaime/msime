//! Unit tests for the parent module, in their own file because the module
//! is large enough that mixing them with the implementation obscured both.
//! Same `mod tests` as before, so `use super::*` still names the parent.

use super::*;

#[test]
fn multilingual_input_defaults_off_and_legacy_documents_remain_compatible() {
    let defaults = Preferences::default();
    assert!(!defaults.multilingual_input.enabled);
    assert!(defaults.multilingual_input.active_languages().is_empty());
    let document = serde_json::to_value(&defaults).unwrap();
    assert!(document.get("multilingual_input").is_none());
    let restored: Preferences = serde_json::from_value(document).unwrap();
    assert_eq!(restored.multilingual_input, defaults.multilingual_input);
}

#[test]
fn multilingual_input_requires_two_distinct_languages_when_enabled() {
    let mut preferences = Preferences::default();
    preferences.multilingual_input.enabled = true;
    for languages in [
        vec![],
        vec![MultilingualLanguage::Chinese],
        vec![MultilingualLanguage::Chinese, MultilingualLanguage::Chinese],
    ] {
        preferences.multilingual_input.languages = languages;
        assert!(preferences.multilingual_input.active_languages().is_empty());
    }
    preferences.multilingual_input.languages = vec![
        MultilingualLanguage::Chinese,
        MultilingualLanguage::Japanese,
        MultilingualLanguage::Chinese,
    ];
    assert_eq!(
        preferences.multilingual_input.active_languages(),
        vec![
            MultilingualLanguage::Chinese,
            MultilingualLanguage::Japanese
        ]
    );
}

#[test]
fn multilingual_input_round_trips_language_selection_and_rejects_unknown_languages() {
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["multilingual_input"] = serde_json::json!({
        "enabled": true, "languages": ["chinese", "japanese"]
    });
    let preferences: Preferences = serde_json::from_value(document.clone()).unwrap();
    assert!(preferences.validate().is_ok());
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(serde_json::to_value(&saved.preferences).unwrap(), document);
    assert_eq!(store.load().unwrap().preferences, saved.preferences);
    let mut disabled = saved.preferences;
    disabled.multilingual_input.enabled = false;
    assert!(disabled.multilingual_input.active_languages().is_empty());
    store.save(saved.revision, disabled.clone()).unwrap();
    assert_eq!(store.load().unwrap().preferences, disabled);
    for invalid in [
        serde_json::json!({"enabled": true, "languages": ["chinese", "unknown"]}),
        serde_json::json!({"enabled": "true", "languages": ["chinese", "english"]}),
        serde_json::json!({"enabled": true, "languages": "chinese"}),
    ] {
        document["multilingual_input"] = invalid;
        assert!(serde_json::from_value::<Preferences>(document.clone()).is_err());
    }
}

#[test]
fn multilingual_input_preserves_future_fields_when_other_settings_change() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut document = serde_json::json!({
        "format_version": 1, "revision": 4,
        "preferences": serde_json::to_value(Preferences::default()).unwrap()
    });
    document["preferences"]["multilingual_input"] = serde_json::json!({
        "enabled": true, "languages": ["chinese", "japanese"],
        "future_policy": {"synthetic": true}
    });
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let mut snapshot = store.load().unwrap();
    snapshot.preferences.candidate_page_size = 7;
    store.save(snapshot.revision, snapshot.preferences).unwrap();
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(written["revision"], 5);
    assert_eq!(written["preferences"]["candidate_page_size"], 7);
    assert_eq!(
        written["preferences"]["multilingual_input"],
        document["preferences"]["multilingual_input"]
    );
}

#[test]
fn eighteen_key_quanpin_preferences_round_trip_without_changing_defaults() {
    let defaults = Preferences::default();
    let mut document = serde_json::to_value(&defaults).unwrap();
    document["touch_keyboard_layout"] = serde_json::json!("eighteen_key");
    document["touch_keyboard_schemes"] = serde_json::json!({
        "enabled": ["eighteen_key"],
        "selected": "eighteen_key"
    });
    let saved: Preferences = serde_json::from_value(document).unwrap();
    let value = serde_json::to_value(saved).unwrap();
    assert_eq!(value["touch_keyboard_layout"], "eighteen_key");
    assert_eq!(value["touch_keyboard_schemes"]["selected"], "eighteen_key");
    assert_eq!(
        defaults.touch_keyboard_layout,
        TouchKeyboardLayout::TwentySixKey
    );
    assert!(!serde_json::to_string(&defaults.touch_keyboard_schemes)
        .unwrap()
        .contains("eighteen_key"));
}

#[test]
fn touch_shuangpin_layout_defaults_persists_and_rejects_unsupported_layouts() {
    let defaults = Preferences::default();
    let legacy = serde_json::to_value(&defaults).unwrap();
    assert!(legacy.get("touch_shuangpin_layout").is_none());
    assert_eq!(
        serde_json::from_value::<Preferences>(legacy.clone())
            .unwrap()
            .touch_shuangpin_layout,
        TouchShuangpinLayout::TwentySixKey
    );
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let configured = Preferences {
        touch_shuangpin_layout: TouchShuangpinLayout::FourteenKey,
        shuangpin_profile: ShuangpinProfile::Microsoft,
        touch_keyboard_layout: TouchKeyboardLayout::NineKey,
        ..defaults
    };
    store.save(0, configured.clone()).unwrap();
    assert_eq!(store.load().unwrap().preferences, configured);
    let mut invalid = legacy.clone();
    invalid["touch_shuangpin_layout"] = "nine_key".into();
    assert!(serde_json::from_value::<Preferences>(invalid).is_err());
    let mut malformed = legacy;
    malformed["touch_japanese_toggle_input"] = "false".into();
    assert!(serde_json::from_value::<Preferences>(malformed).is_err());
}

#[test]
fn japanese_toggle_input_persists_disabled() {
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["touch_japanese_toggle_input"] = false.into();
    let disabled: Preferences = serde_json::from_value(value).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let saved = store.save(0, disabled).unwrap();
    assert_eq!(
        serde_json::to_value(&saved.preferences).unwrap()["touch_japanese_toggle_input"],
        false
    );
    assert_eq!(
        serde_json::to_value(store.load().unwrap().preferences).unwrap()
            ["touch_japanese_toggle_input"],
        false
    );
}

#[test]
fn candidate_translation_font_size_is_optional_bounded_and_round_trips() {
    let mut json = serde_json::to_value(Preferences::default()).unwrap();
    assert!(json.get("candidate_translation_font_size").is_none());
    for size in [12, 21, 32] {
        json["candidate_translation_font_size"] = serde_json::json!(size);
        let preferences: Preferences = serde_json::from_value(json.clone()).unwrap();
        assert!(preferences.validate().is_ok());
        assert_eq!(
            serde_json::to_value(preferences).unwrap()["candidate_translation_font_size"],
            size
        );
    }
    for size in [0, 11, 33, 255] {
        json["candidate_translation_font_size"] = serde_json::json!(size);
        let preferences: Preferences = serde_json::from_value(json.clone()).unwrap();
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidCandidateFontSize)
        ));
    }
    json["candidate_translation_font_size"] = serde_json::Value::Null;
    let preferences: Preferences = serde_json::from_value(json).unwrap();
    assert!(preferences.validate().is_ok());
    assert!(serde_json::to_value(preferences)
        .unwrap()
        .get("candidate_translation_font_size")
        .is_none());
}

#[test]
fn android_text_trackpad_defaults_and_legacy_documents_agree() {
    let defaults = serde_json::to_value(Preferences::default()).unwrap();
    assert_eq!(
        defaults["android_text_trackpad"],
        serde_json::json!({
            "enabled": false, "hold_to_open": false, "horizontal_dp": 12,
            "vertical_dp": 28, "tap_selection": true, "haptics": true,
            "trail": true, "magnifier": true
        })
    );
    let mut legacy = defaults.clone();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("android_text_trackpad");
    let restored: Preferences = serde_json::from_value(legacy).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), defaults);
}

#[test]
fn android_text_trackpad_validates_ranges_and_persists_all_options() {
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["android_text_trackpad"] = serde_json::json!({
        "enabled": true, "hold_to_open": true, "horizontal_dp": 4,
        "vertical_dp": 96, "tap_selection": false, "haptics": false,
        "trail": false, "magnifier": false
    });
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let preferences: Preferences = serde_json::from_value(document.clone()).unwrap();
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(
        serde_json::to_value(store.load().unwrap().preferences).unwrap(),
        document
    );
    for (key, invalid) in [
        ("horizontal_dp", 3),
        ("horizontal_dp", 49),
        ("vertical_dp", 7),
        ("vertical_dp", 97),
    ] {
        let mut invalid_document = document.clone();
        invalid_document["android_text_trackpad"][key] = invalid.into();
        let invalid_preferences: Preferences = serde_json::from_value(invalid_document).unwrap();
        assert!(invalid_preferences.validate().is_err(), "{key}: {invalid}");
    }
    for invalid in [
        serde_json::json!("true"),
        serde_json::json!(0),
        serde_json::Value::Null,
    ] {
        document["android_text_trackpad"]["enabled"] = invalid;
        assert!(serde_json::from_value::<Preferences>(document.clone()).is_err());
    }
    store.restore_defaults(saved.revision).unwrap();
    assert_eq!(store.load().unwrap().preferences, Preferences::default());
}

#[test]
fn backspace_selection_preference_keeps_legacy_default_and_persists_disabled() {
    let defaults = Preferences::default();
    assert!(defaults.backspace_reverts_selection);
    let legacy = serde_json::to_value(&defaults).unwrap();
    assert!(legacy.get("backspace_reverts_selection").is_none());
    assert!(
        serde_json::from_value::<Preferences>(legacy.clone())
            .unwrap()
            .backspace_reverts_selection
    );
    let mut malformed = legacy;
    malformed["backspace_reverts_selection"] = "false".into();
    assert!(serde_json::from_value::<Preferences>(malformed).is_err());

    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let disabled = Preferences {
        backspace_reverts_selection: false,
        ..defaults
    };
    let saved = store.save(0, disabled).unwrap();
    assert!(!saved.preferences.backspace_reverts_selection);
    assert!(
        !store
            .load()
            .unwrap()
            .preferences
            .backspace_reverts_selection
    );
    assert_eq!(
        serde_json::to_value(&saved.preferences).unwrap()["backspace_reverts_selection"],
        false
    );
}

#[cfg(unix)]
#[test]
fn stale_sweep_does_not_follow_a_replaced_directory() {
    use msime_path_trust::untrusted_symlink as symlink;
    use std::time::{Duration, SystemTime};

    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("preferences");
    std::fs::create_dir(&directory).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let abandoned = outside.path().join(".tmpOutside");
    std::fs::write(&abandoned, b"synthetic outside").unwrap();
    let old = SystemTime::now() - Duration::from_secs(48 * 60 * 60);
    std::fs::File::options()
        .write(true)
        .open(&abandoned)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(old))
        .unwrap();

    let moved = root.path().join("moved");
    std::fs::rename(&directory, &moved).unwrap();
    symlink(outside.path(), &directory).unwrap();

    sweep_stale_temporaries(&directory);

    assert!(abandoned.exists());
}

#[cfg(unix)]
#[test]
fn recovery_backup_does_not_follow_a_symlinked_directory() {
    use msime_path_trust::untrusted_symlink as symlink;

    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("preferences");
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), &directory).unwrap();
    let store = PreferencesStore::new(&directory);

    assert!(store
        .write_backup(
            b"synthetic damaged preferences",
            time::OffsetDateTime::now_utc(),
        )
        .is_err());
    assert!(outside.path().read_dir().unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn preference_store_rejects_a_symlinked_directory_without_writing_through_it() {
    let target = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let directory = parent.path().join("preferences");
    std::os::unix::fs::symlink(target.path(), &directory).unwrap();

    let result = PreferencesStore::new(&directory).load();

    assert!(matches!(result, Err(PreferencesError::Io(_))));
    assert!(!target.path().join("preferences.lock").exists());
}

#[cfg(unix)]
#[test]
fn preference_store_rejects_a_symlinked_document() {
    let directory = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    let outside_path = outside.path().join("preferences.json");
    std::fs::write(&outside_path, br#"{"formatVersion":1}"#).unwrap();
    std::os::unix::fs::symlink(&outside_path, &path).unwrap();

    assert!(matches!(
        PreferencesStore::new(directory.path()).load(),
        Err(PreferencesError::Io(_))
    ));
}

#[test]
fn ctrl_space_defaults_on_and_persists_an_explicit_disable() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    let mut value = serde_json::to_value(&snapshot.preferences).unwrap();
    assert_eq!(value["keybindings"]["switch_language_ctrl_space"], true);
    value["keybindings"]
        .as_object_mut()
        .unwrap()
        .remove("switch_language_ctrl_space");
    let legacy: Preferences = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(legacy).unwrap()["keybindings"]["switch_language_ctrl_space"],
        true
    );
    value["keybindings"]["switch_language_ctrl_space"] = false.into();
    let disabled: Preferences = serde_json::from_value(value).unwrap();
    store.save(snapshot.revision, disabled).unwrap();
    assert_eq!(
        serde_json::to_value(store.load().unwrap().preferences).unwrap()["keybindings"]
            ["switch_language_ctrl_space"],
        false
    );
}

#[test]
fn caps_lock_language_switch_defaults_off_and_persists_an_explicit_enable() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    assert!(!snapshot.preferences.keybindings.switch_language_caps_lock);
    // 早于这个开关的文档没有这个键，读出来是关。
    let mut value = serde_json::to_value(&snapshot.preferences).unwrap();
    assert_eq!(
        value["keybindings"]
            .as_object_mut()
            .unwrap()
            .remove("switch_language_caps_lock"),
        Some(false.into())
    );
    let legacy: Preferences = serde_json::from_value(value.clone()).unwrap();
    assert!(!legacy.keybindings.switch_language_caps_lock);
    // 打开它不改动其他切换键。
    value["keybindings"]["switch_language_caps_lock"] = true.into();
    let enabled: Preferences = serde_json::from_value(value).unwrap();
    store.save(snapshot.revision, enabled).unwrap();
    let keybindings = store.load().unwrap().preferences.keybindings;
    assert_eq!(
        keybindings,
        KeybindingPreferences {
            switch_language_caps_lock: true,
            ..KeybindingPreferences::default()
        }
    );
}

#[test]
fn japanese_toggle_defaults_off_and_persists_an_explicit_enable() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    assert!(
        !snapshot
            .preferences
            .keybindings
            .toggle_japanese_ctrl_shift_j
    );
    // 早于这个开关的文档没有这个键，读出来是关。
    let mut value = serde_json::to_value(&snapshot.preferences).unwrap();
    assert_eq!(
        value["keybindings"]
            .as_object_mut()
            .unwrap()
            .remove("toggle_japanese_ctrl_shift_j"),
        Some(false.into())
    );
    let legacy: Preferences = serde_json::from_value(value.clone()).unwrap();
    assert!(!legacy.keybindings.toggle_japanese_ctrl_shift_j);
    // 打开它不改动其他快捷键。
    value["keybindings"]["toggle_japanese_ctrl_shift_j"] = true.into();
    let enabled: Preferences = serde_json::from_value(value).unwrap();
    store.save(snapshot.revision, enabled).unwrap();
    let keybindings = store.load().unwrap().preferences.keybindings;
    assert_eq!(
        keybindings,
        KeybindingPreferences {
            toggle_japanese_ctrl_shift_j: true,
            ..KeybindingPreferences::default()
        }
    );
}

#[test]
fn trilingual_cycle_defaults_off_and_persists_an_explicit_enable() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    // 默认对齐旧版 Windows 正式版的出厂值。
    assert!(!snapshot.preferences.keybindings.trilingual_cycle);
    // 早于这个开关的文档没有这个键，读出来是关。
    let mut value = serde_json::to_value(&snapshot.preferences).unwrap();
    assert_eq!(
        value["keybindings"]
            .as_object_mut()
            .unwrap()
            .remove("trilingual_cycle"),
        Some(false.into())
    );
    let legacy: Preferences = serde_json::from_value(value.clone()).unwrap();
    assert!(!legacy.keybindings.trilingual_cycle);
    // 打开它，「切换中英文」那一组快捷键本身不变：循环与否是另一项。
    value["keybindings"]["trilingual_cycle"] = true.into();
    let enabled: Preferences = serde_json::from_value(value).unwrap();
    store.save(snapshot.revision, enabled).unwrap();
    let keybindings = store.load().unwrap().preferences.keybindings;
    assert_eq!(
        keybindings,
        KeybindingPreferences {
            trilingual_cycle: true,
            ..KeybindingPreferences::default()
        }
    );
}

#[test]
fn toggle_hotkeys_use_the_legacy_factory_defaults() {
    let keybindings = Preferences::default().keybindings;
    assert_eq!(keybindings.toggle_english_candidates, "Ctrl+Shift+KeyE");
    assert_eq!(keybindings.toggle_full_width, "Ctrl+Shift+Space");
    assert_eq!(keybindings.toggle_punctuation, "Ctrl+Period");
    // 出厂值不落盘：写出的文档不带这三个键；没有这三个键的旧文档读回来还是出厂值。
    let value = serde_json::to_value(Preferences::default()).unwrap();
    let object = value["keybindings"].as_object().unwrap();
    for key in [
        "toggle_english_candidates",
        "toggle_full_width",
        "toggle_punctuation",
    ] {
        assert!(
            !object.contains_key(key),
            "{key} must stay out of the default document"
        );
    }
    let restored: Preferences = serde_json::from_value(value).unwrap();
    assert_eq!(restored.keybindings, Preferences::default().keybindings);
}

#[test]
fn toggle_hotkeys_persist_custom_chords_and_disabled_bindings() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    let mut preferences = snapshot.preferences.clone();
    preferences.keybindings.toggle_punctuation = "Ctrl+Alt+Slash".into();
    preferences.keybindings.toggle_full_width = String::new();
    store.save(snapshot.revision, preferences).unwrap();
    let loaded = store.load().unwrap().preferences.keybindings;
    assert_eq!(
        loaded.punctuation_chord().unwrap().to_string(),
        "Ctrl+Alt+Slash"
    );
    // 空串是「不绑定」，不是回到出厂值。
    assert!(loaded.full_width_chord().is_none());
    assert_eq!(loaded.toggle_full_width, "");
}

#[test]
fn toggle_hotkeys_reject_unparseable_values() {
    for bad in ["Ctrl+", "Ctrl+Alt+Shift", "Ctrl+Banana", "Period+Ctrl"] {
        let mut preferences = Preferences::default();
        preferences.keybindings.toggle_punctuation = bad.into();
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidToggleHotkey)
        ));
    }
}

#[test]
fn toggle_hotkeys_reject_two_bindings_for_the_same_chord() {
    let mut preferences = Preferences::default();
    preferences.keybindings.toggle_punctuation = "control+shift+keye".into();
    assert!(matches!(
        preferences.validate(),
        Err(PreferencesError::ConflictingToggleHotkeys)
    ));
    preferences.keybindings.toggle_punctuation = "Shift+Ctrl+Space".into();
    assert!(matches!(
        preferences.validate(),
        Err(PreferencesError::ConflictingToggleHotkeys)
    ));
}

#[test]
fn toggle_hotkeys_reject_chords_owned_by_enabled_switches() {
    // 保留表随开关开合：简繁开着时 Ctrl+Shift+F 被占用，关掉后才绑得出去。
    for (text, reason) in [
        ("Ctrl+Shift+KeyF", "简繁切换"),
        ("Ctrl+Space", "Linux 语言切换"),
        ("Ctrl+Alt+Space", "语言切换"),
        ("Ctrl+F9", "语音切换"),
        ("Ctrl+Shift+F9", "Windows 语音只看 Ctrl 按住"),
        ("Ctrl+Alt+Shift+KeyC", "macOS 维护键"),
    ] {
        let mut preferences = Preferences::default();
        preferences.keybindings.toggle_punctuation = text.into();
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::ConflictingToggleHotkeys)
            ),
            "{text} must conflict ({reason})"
        );
    }
    let mut preferences = Preferences::default();
    preferences.keybindings.toggle_punctuation = "Ctrl+Shift+KeyF".into();
    preferences.keybindings.toggle_character_set_ctrl_shift_f = false;
    preferences.validate().unwrap();
}

#[test]
fn toggle_hotkeys_all_empty_disables_every_toggle() {
    let mut preferences = Preferences::default();
    preferences.keybindings.toggle_english_candidates = String::new();
    preferences.keybindings.toggle_full_width = String::new();
    preferences.keybindings.toggle_punctuation = String::new();
    preferences.validate().unwrap();
    assert!(preferences.keybindings.english_candidates_chord().is_none());
}

#[test]
fn toggle_hotkey_check_returns_the_canonical_spelling() {
    let keybindings = KeybindingPreferences::default();
    for (slot, text, expected) in [
        (
            ToggleHotkeySlot::EnglishCandidates,
            "Ctrl+Alt+KeyR",
            "Ctrl+Alt+KeyR",
        ),
        // 解析比文档宽：别名、大小写和修饰键顺序都归一成规范形。
        (
            ToggleHotkeySlot::Punctuation,
            "control+alt+slash",
            "Ctrl+Alt+Slash",
        ),
        (
            ToggleHotkeySlot::FullWidth,
            "shift+ctrl+period",
            "Ctrl+Shift+Period",
        ),
    ] {
        assert_eq!(
            keybindings.check_toggle_hotkey(slot, text, true),
            Ok(expected.to_string()),
            "{text}"
        );
    }
    // 空串是合法的「不绑定」，通过且回空。
    assert_eq!(
        keybindings.check_toggle_hotkey(ToggleHotkeySlot::Punctuation, "  ", true),
        Ok(String::new())
    );
    // 写回自己当前的绑定不算冲突（写法不同、指向同一按键也归一成同一个值）。
    assert_eq!(
        keybindings.check_toggle_hotkey(ToggleHotkeySlot::Punctuation, "control+period", true),
        Ok("Ctrl+Period".to_string())
    );
}

#[test]
fn toggle_hotkey_check_rejects_unparseable_text() {
    let keybindings = KeybindingPreferences::default();
    for bad in ["Ctrl+", "Ctrl+Alt+Shift", "Ctrl+Banana", "Period+Ctrl"] {
        assert!(
            matches!(
                keybindings.check_toggle_hotkey(ToggleHotkeySlot::Punctuation, bad, true),
                Err(ToggleHotkeyRejection::Invalid(_))
            ),
            "{bad} must fail"
        );
    }
}

#[test]
fn toggle_hotkey_shift_space_follows_language_switch() {
    let mut keybindings = KeybindingPreferences::default();
    for slot in [
        ToggleHotkeySlot::EnglishCandidates,
        ToggleHotkeySlot::FullWidth,
        ToggleHotkeySlot::Punctuation,
    ] {
        assert!(matches!(
            keybindings.check_toggle_hotkey(slot, "Shift+Space", false),
            Err(ToggleHotkeyRejection::Conflict { .. })
        ));
    }
    let mut preferences = Preferences::default();
    preferences.keybindings.toggle_punctuation = "Shift+Space".into();
    assert!(preferences.validate().is_err());
    keybindings.switch_language_shift = false;
    assert_eq!(
        keybindings.check_toggle_hotkey(ToggleHotkeySlot::Punctuation, "Shift+Space", false),
        Ok("Shift+Space".into())
    );
    preferences.keybindings.switch_language_shift = false;
    assert!(preferences.validate().is_ok());
}

#[test]
fn toggle_hotkey_reserves_macos_width_and_screen_keyboard() {
    let mut keys = KeybindingPreferences::default();
    for text in ["Alt+Shift+KeyH", "Ctrl+Shift+Meta+KeyK"] {
        assert!(matches!(
            keys.check_toggle_hotkey(ToggleHotkeySlot::Punctuation, text, false),
            Err(ToggleHotkeyRejection::Conflict { .. })
        ));
        let mut preferences = Preferences::default();
        preferences.keybindings.toggle_punctuation = text.into();
        assert!(preferences.validate().is_err());
    }
    keys.toggle_fullwidth_option_shift_h = false;
    assert_eq!(
        keys.check_toggle_hotkey(ToggleHotkeySlot::Punctuation, "Alt+Shift+KeyH", false),
        Ok("Alt+Shift+KeyH".into())
    );
}

#[test]
fn toggle_hotkey_check_names_the_conflicting_owner() {
    let keybindings = KeybindingPreferences {
        toggle_japanese_ctrl_shift_j: true,
        ..KeybindingPreferences::default()
    };
    // 撞另一个切换热键：owner 是那个槽位，设置页按它指出「与哪一项冲突」。
    for (slot, text, owner) in [
        (
            ToggleHotkeySlot::Punctuation,
            "ctrl+shift+keye",
            ToggleChordOwner::EnglishCandidates,
        ),
        (
            ToggleHotkeySlot::EnglishCandidates,
            "Ctrl+Period",
            ToggleChordOwner::Punctuation,
        ),
        // 撞已启用开关占用的组合键：owner 指到占用它的开关。
        (
            ToggleHotkeySlot::Punctuation,
            "Ctrl+Shift+KeyF",
            ToggleChordOwner::ToggleCharacterSetCtrlShiftF,
        ),
        (
            ToggleHotkeySlot::FullWidth,
            "Ctrl+Space",
            ToggleChordOwner::SwitchLanguageCtrlSpace,
        ),
        (
            ToggleHotkeySlot::FullWidth,
            "Ctrl+Shift+KeyJ",
            ToggleChordOwner::ToggleJapaneseCtrlShiftJ,
        ),
        (
            ToggleHotkeySlot::FullWidth,
            "Ctrl+F9",
            ToggleChordOwner::VoiceHotkey,
        ),
        (
            ToggleHotkeySlot::FullWidth,
            "Ctrl+Shift+F9",
            ToggleChordOwner::VoiceHotkey,
        ),
        // macOS 维护键没有开关，照样指名。
        (
            ToggleHotkeySlot::FullWidth,
            "Ctrl+Alt+Shift+KeyT",
            ToggleChordOwner::MacosMaintenance,
        ),
    ] {
        let Err(ToggleHotkeyRejection::Conflict {
            owner: actual,
            chord,
        }) = keybindings.check_toggle_hotkey(slot, text, true)
        else {
            panic!("{text} must conflict");
        };
        assert_eq!(actual, owner, "{text}");
        assert_eq!(chord, text.parse().unwrap(), "{text}");
    }
    // owner 有跨进程用的稳定标识：撞切换热键时用文档字段名，撞占用键时用开关字段名或 `voice_hotkey` / `macos_maintenance`。
    assert_eq!(
        ToggleChordOwner::EnglishCandidates.id(),
        "toggle_english_candidates"
    );
    assert_eq!(ToggleChordOwner::VoiceHotkey.id(), "voice_hotkey");
    assert_eq!(ToggleChordOwner::MacosMaintenance.id(), "macos_maintenance");

    // 占用随开关开合：关掉简繁、中日切换和语音热键后，这些组合键可以绑出去。
    let mut keybindings = KeybindingPreferences {
        toggle_character_set_ctrl_shift_f: false,
        toggle_japanese_ctrl_shift_j: false,
        ..KeybindingPreferences::default()
    };
    assert_eq!(
        keybindings.check_toggle_hotkey(ToggleHotkeySlot::FullWidth, "Ctrl+Shift+KeyF", false),
        Ok("Ctrl+Shift+KeyF".to_string())
    );
    assert_eq!(
        keybindings.check_toggle_hotkey(ToggleHotkeySlot::FullWidth, "Ctrl+F9", false),
        Ok("Ctrl+F9".to_string())
    );
    // 另一个切换热键解绑（空串）后，这个组合键也可以绑出去。
    keybindings.toggle_punctuation = String::new();
    assert_eq!(
        keybindings.check_toggle_hotkey(ToggleHotkeySlot::FullWidth, "Ctrl+Period", false),
        Ok("Ctrl+Period".to_string())
    );
}

#[test]
fn saving_canonicalizes_toggle_hotkey_spellings() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let snapshot = store.load().unwrap();
    let mut preferences = snapshot.preferences.clone();
    preferences.keybindings.toggle_punctuation = "control+alt+slash".into();
    // 与出厂值等价的别名写法归一后与出厂写法相同，一样不落盘。
    preferences.keybindings.toggle_english_candidates = "control+shift+keye".into();
    let saved = store.save(snapshot.revision, preferences).unwrap();
    assert_eq!(
        saved.preferences.keybindings.toggle_punctuation,
        "Ctrl+Alt+Slash"
    );
    assert_eq!(
        saved.preferences.keybindings.toggle_english_candidates,
        "Ctrl+Shift+KeyE"
    );
    let document = serde_json::to_value(&saved).unwrap();
    let keybindings = document["preferences"]["keybindings"].as_object().unwrap();
    assert_eq!(keybindings["toggle_punctuation"], "Ctrl+Alt+Slash");
    assert!(!keybindings.contains_key("toggle_english_candidates"));
}

#[test]
fn voice_commit_mode_defaults_for_legacy_documents() {
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["voice_input"]
        .as_object_mut()
        .unwrap()
        .remove("commit_mode");
    let restored: Preferences = serde_json::from_value(value).unwrap();
    assert_eq!(restored.voice_input.commit_mode, "tsf");
}

#[test]
fn oversized_preference_documents_are_rejected_before_loading() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.json");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(MAX_DOCUMENT_BYTES + 1)
        .unwrap();
    assert!(matches!(
        PreferencesStore::new(directory.path()).load(),
        Err(PreferencesError::DocumentTooLarge)
    ));
}

#[test]
fn doubao_auth_mode_defaults_and_roundtrips() {
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["voice_input"]
        .as_object_mut()
        .unwrap()
        .remove("doubao_auth_mode");
    let restored: Preferences = serde_json::from_value(value).unwrap();
    assert_eq!(restored.voice_input.doubao_auth_mode, "api_key");

    let mut explicit = Preferences::default();
    explicit.voice_input.doubao_auth_mode = "legacy".into();
    let roundtripped: Preferences =
        serde_json::from_value(serde_json::to_value(explicit).unwrap()).unwrap();
    assert_eq!(roundtripped.voice_input.doubao_auth_mode, "legacy");
}

#[test]
fn ai_assistant_without_a_provider_key_loads_with_the_default_provider() {
    assert_eq!(
        default_ai_provider(),
        AiAssistantPreferences::default().provider
    );
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut document = serde_json::to_value(PreferencesSnapshot {
        format_version: 1,
        revision: 3,
        preferences: Preferences::default(),
    })
    .unwrap();
    document["preferences"]["ai_assistant"] = serde_json::json!({ "enabled": true });
    std::fs::write(
        directory.path().join("preferences.json"),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();

    let snapshot = store
        .load()
        .expect("a section without provider still loads");
    assert_eq!(snapshot.preferences.ai_assistant.provider, "deepseek");
    let saved = store.save(3, snapshot.preferences.clone()).unwrap();
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn every_reachable_voice_provider_validates_and_others_are_rejected_on_save() {
    // 阿里云百炼（#6017）走整句识别，id 是 `bailian`。
    assert!(ASR_PROVIDERS.contains(&"bailian"));
    for provider in ASR_PROVIDERS {
        let preferences = Preferences {
            voice_input: VoiceInputPreferences {
                asr_provider: provider.into(),
                ..Preferences::default().voice_input
            },
            ..Preferences::default()
        };
        assert!(
            preferences.validate().is_ok(),
            "{provider} should be accepted"
        );
    }
    for provider in POLISH_PROVIDERS {
        let preferences = Preferences {
            voice_input: VoiceInputPreferences {
                polish_provider: provider.into(),
                ..Preferences::default().voice_input
            },
            ..Preferences::default()
        };
        assert!(preferences.validate().is_ok(), "{provider} should polish");
    }
    // Saving a provider no backend implements is refused rather than stored.
    for rejected in ["local_whisper", "cloud", "", "DOUBAO"] {
        let preferences = Preferences {
            voice_input: VoiceInputPreferences {
                asr_provider: rejected.into(),
                ..Preferences::default().voice_input
            },
            ..Preferences::default()
        };
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidVoiceInput)
            ),
            "{rejected} should be rejected"
        );
    }
    // Recognition has no DeepSeek profile even though polishing does.
    let preferences = Preferences {
        voice_input: VoiceInputPreferences {
            asr_provider: "deepseek".into(),
            ..Preferences::default().voice_input
        },
        ..Preferences::default()
    };
    assert!(preferences.validate().is_err());
}

#[test]
fn local_recognition_stores_an_absolute_model_path_and_refuses_anything_else() {
    let with_path = |path: &str| Preferences {
        voice_input: VoiceInputPreferences {
            asr_provider: "local".into(),
            asr_model_path: path.into(),
            ..Preferences::default().voice_input
        },
        ..Preferences::default()
    };
    // A Windows path is absolute too: the Windows host saves one, and the same document is validated wherever it is read.
    for accepted in [
        "",
        "/Users/someone/Library/Application Support/msime/voice-models/x-asr-zh-en-streaming",
        r"C:\Users\someone\AppData\Roaming\msime\voice-models\sense-voice-small",
        "D:/models/sense-voice-small",
        r"\\?\C:\models\x-asr-zh-en-streaming",
        r"\\fileserver\share\models\fun-asr-nano",
    ] {
        assert!(
            with_path(accepted).validate().is_ok(),
            "{accepted:?} should be accepted"
        );
    }
    // A relative path resolves against whichever process happens to read it, and a control character reaches the recognizer as a filename it cannot open. Both fail while the user holds the shortcut.
    for rejected in [
        "models/x-asr-zh-en-streaming",
        "~/models/x-asr-zh-en-streaming",
        "/models/x-asr\nzh-en-streaming",
        r"C:models\x-asr-zh-en-streaming",
        r"\models\x-asr-zh-en-streaming",
        "C:\\models\\x-asr\tzh-en-streaming",
    ] {
        assert!(
            matches!(
                with_path(rejected).validate(),
                Err(PreferencesError::InvalidVoiceInput)
            ),
            "{rejected:?} should be rejected"
        );
    }
    assert!(with_path(&"/".repeat(4097)).validate().is_err());
}

#[test]
fn local_model_mirror_is_empty_or_an_https_prefix() {
    let with_mirror = |mirror: &str| Preferences {
        voice_input: VoiceInputPreferences {
            asr_model_mirror: mirror.into(),
            ..Preferences::default().voice_input
        },
        ..Preferences::default()
    };
    assert!(Preferences::default()
        .voice_input
        .asr_model_mirror
        .is_empty());
    for accepted in [
        "",
        "https://ghproxy.example.test",
        "https://mirror.example.test/gh/",
    ] {
        assert!(
            with_mirror(accepted).validate().is_ok(),
            "{accepted:?} should be accepted"
        );
    }
    // A plain-HTTP mirror would let anyone on the path swap the model; the checksum still catches it, but the download should not be attempted at all.
    for rejected in [
        "http://ghproxy.example.test",
        "https://",
        "https:///path",
        "https://user:pass@mirror.example.test",
        "https://mirror.example.test?query=unexpected",
        "https://mirror.example.test/#fragment",
        "ghproxy.example.test",
        "https://mirror.example.test/\n",
        "https://mirror example.test",
    ] {
        assert!(
            matches!(
                with_mirror(rejected).validate(),
                Err(PreferencesError::InvalidVoiceInput)
            ),
            "{rejected:?} should be rejected"
        );
    }
    let long = format!("https://{}", "a".repeat(2048));
    assert!(with_mirror(&long).validate().is_err());
    // Older documents without the field still load, with no mirror.
    let mut legacy = serde_json::to_value(Preferences::default()).unwrap();
    legacy["voice_input"]
        .as_object_mut()
        .unwrap()
        .remove("asr_model_mirror");
    let loaded: Preferences = serde_json::from_value(legacy).unwrap();
    assert!(loaded.voice_input.asr_model_mirror.is_empty());
}

#[test]
fn the_shipped_defaults_are_themselves_reachable() {
    let defaults = Preferences::default();
    assert!(ASR_PROVIDERS.contains(&defaults.voice_input.asr_provider.as_str()));
    assert!(POLISH_PROVIDERS.contains(&defaults.voice_input.polish_provider.as_str()));
    assert!(defaults.validate().is_ok());
}

#[test]
fn system_voice_provider_round_trips_without_cloud_credentials() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut preferences = Preferences::default();
    preferences.voice_input.asr_provider = "system".into();
    preferences.voice_input.asr_endpoint.clear();
    preferences.voice_input.asr_model.clear();
    preferences.voice_input.asr_token.clear();
    let saved = store.save(0, preferences.clone()).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded, saved);
    assert_eq!(loaded.preferences, preferences);
    assert_eq!(loaded.preferences.voice_input.asr_provider, "system");
}

#[test]
fn candidate_english_gloss_is_opt_in_and_round_trips() {
    let defaults = Preferences::default();
    assert!(!defaults.candidate_english_gloss);
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("candidate_english_gloss");
    assert!(
        !serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .candidate_english_gloss
    );
    let enabled = Preferences {
        candidate_english_gloss: true,
        ..defaults
    };
    assert!(
        serde_json::from_str::<Preferences>(&serde_json::to_string(&enabled).unwrap())
            .unwrap()
            .candidate_english_gloss
    );
}

#[test]
fn candidate_pronunciation_is_opt_in_and_round_trips() {
    let defaults = Preferences::default();
    assert!(!defaults.candidate_pronunciation);
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("candidate_pronunciation");
    assert!(
        !serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .candidate_pronunciation
    );
    let enabled = Preferences {
        candidate_pronunciation: true,
        ..defaults
    };
    assert!(
        serde_json::from_str::<Preferences>(&serde_json::to_string(&enabled).unwrap())
            .unwrap()
            .candidate_pronunciation
    );
}

#[test]
fn candidate_translation_display_defaults_follow_windows_factory_config() {
    let defaults = Preferences::default();
    assert!(defaults.candidate_translation_show_pos);
    assert!(!defaults.candidate_translation_single_sense);

    // 旧文档缺这两个键时按 Windows v0.11.1 的出厂配置补上。
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    let fields = legacy.as_object_mut().unwrap();
    fields.remove("candidate_translation_show_pos");
    fields.remove("candidate_translation_single_sense");
    let parsed = serde_json::from_value::<Preferences>(legacy).unwrap();
    assert!(parsed.candidate_translation_show_pos);
    assert!(!parsed.candidate_translation_single_sense);

    // 用户显式保存的选择原样往返。
    let saved = Preferences {
        candidate_translation_show_pos: false,
        candidate_translation_single_sense: true,
        ..defaults
    };
    let round_tripped =
        serde_json::from_str::<Preferences>(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(!round_tripped.candidate_translation_show_pos);
    assert!(round_tripped.candidate_translation_single_sense);
}

#[test]
fn candidate_badge_defaults_follow_windows_factory_config() {
    let defaults = Preferences::default();
    assert!(defaults.candidate_fixed_badge);
    assert_eq!(defaults.candidate_fixed_badge_style, "paperclip");
    assert_eq!(defaults.candidate_ai_badge_style, "robot");

    // 旧文档缺这几个键时按 Windows v0.11.1 的出厂配置补上。
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    let fields = legacy.as_object_mut().unwrap();
    fields.remove("candidate_fixed_badge");
    fields.remove("candidate_fixed_badge_style");
    fields.remove("candidate_ai_badge_style");
    let parsed = serde_json::from_value::<Preferences>(legacy).unwrap();
    assert!(parsed.candidate_fixed_badge);
    assert_eq!(parsed.candidate_fixed_badge_style, "paperclip");
    assert_eq!(parsed.candidate_ai_badge_style, "robot");

    // 用户显式保存的选择原样往返。
    let saved = Preferences {
        candidate_fixed_badge: false,
        candidate_fixed_badge_style: "dot".to_string(),
        candidate_ai_badge_style: "whale".to_string(),
        ..defaults
    };
    let round_tripped =
        serde_json::from_str::<Preferences>(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert!(!round_tripped.candidate_fixed_badge);
    assert_eq!(round_tripped.candidate_fixed_badge_style, "dot");
    assert_eq!(round_tripped.candidate_ai_badge_style, "whale");
}

#[test]
fn english_suggestions_default_on_and_legacy_documents_preserve_it() {
    let defaults = Preferences::default();
    assert!(defaults.english_suggestions);
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("english_suggestions");
    assert!(
        serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .english_suggestions
    );
    let mut disabled = defaults;
    disabled.english_suggestions = false;
    assert!(
        !serde_json::from_str::<Preferences>(&serde_json::to_string(&disabled).unwrap())
            .unwrap()
            .english_suggestions
    );
}

// Usage reporting is on by default and can be turned off.
#[test]
fn usage_reporting_defaults_on_and_survives_a_save() {
    let defaults = Preferences::default();
    assert!(defaults.usage_reporting);
    let mut serialized = serde_json::to_value(&defaults).unwrap();
    assert_eq!(serialized["usage_reporting"], serde_json::Value::Bool(true));
    assert!(serialized.get("telemetry_enabled").is_none());
    // An absent key reads as on.
    serialized
        .as_object_mut()
        .unwrap()
        .remove("usage_reporting");
    assert!(
        serde_json::from_value::<Preferences>(serialized.clone())
            .unwrap()
            .usage_reporting
    );
    let off = serde_json::to_value(Preferences {
        usage_reporting: false,
        ..Preferences::default()
    })
    .unwrap();
    assert_eq!(off["usage_reporting"], serde_json::Value::Bool(false));
    let mut malformed = serialized;
    malformed["usage_reporting"] = "yes".into();
    assert!(serde_json::from_value::<Preferences>(malformed).is_err());

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let disabled = Preferences {
        usage_reporting: false,
        ..defaults
    };
    assert!(disabled.validate().is_ok());
    let saved = store.save(0, disabled).unwrap();
    assert!(!saved.preferences.usage_reporting);
    assert!(!store.load().unwrap().preferences.usage_reporting);
}

#[test]
fn app_logo_starts_hidden_but_an_upgraded_document_keeps_it() {
    let defaults = Preferences::default();
    assert!(!defaults.show_app_logo);
    assert!(!defaults.restored_to_defaults().show_app_logo);
    // 旧文档里没有这个字段时按原来的样子读成显示，升级不替用户把 logo 藏起来。
    let mut serialized = serde_json::to_value(&defaults).unwrap();
    serialized.as_object_mut().unwrap().remove("show_app_logo");
    assert!(
        serde_json::from_value::<Preferences>(serialized)
            .unwrap()
            .show_app_logo
    );

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let chosen = Preferences {
        show_app_logo: true,
        ..defaults
    };
    store.save(0, chosen).unwrap();
    assert!(store.load().unwrap().preferences.show_app_logo);
}

#[test]
fn shuangpin_keymap_hint_stays_out_of_the_document_until_chosen() {
    let defaults = Preferences::default();
    assert_eq!(defaults.shuangpin_keymap_hint, None);
    // 没选过时不写进文档，macOS 才分得出「从没选过」和「选了关」，前者仍读本机 defaults 里的旧选择。
    let serialized = serde_json::to_value(&defaults).unwrap();
    assert!(serialized.get("shuangpin_keymap_hint").is_none());
    assert_eq!(
        serde_json::from_value::<Preferences>(serialized)
            .unwrap()
            .shuangpin_keymap_hint,
        None
    );

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let chosen = Preferences {
        shuangpin_keymap_hint: Some(false),
        ..defaults
    };
    store.save(0, chosen).unwrap();
    // 选了关也是一个选择，照样存下来，免得 macOS 退回去读旧的开。
    assert_eq!(
        store.load().unwrap().preferences.shuangpin_keymap_hint,
        Some(false)
    );
    assert_eq!(
        serde_json::to_value(store.load().unwrap().preferences).unwrap()["shuangpin_keymap_hint"],
        false
    );
}

#[test]
fn cloud_candidates_start_off_but_an_upgraded_document_keeps_them() {
    let defaults = Preferences::default();
    assert!(!defaults.cloud_candidates);
    assert!(!defaults.restored_to_defaults().cloud_candidates);
    // 旧文档里没有这个字段时按原来的行为读成开启，升级不替用户改。
    let mut serialized = serde_json::to_value(&defaults).unwrap();
    serialized
        .as_object_mut()
        .unwrap()
        .remove("cloud_candidates");
    assert!(
        serde_json::from_value::<Preferences>(serialized)
            .unwrap()
            .cloud_candidates
    );

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let chosen = Preferences {
        cloud_candidates: true,
        ..defaults
    };
    store.save(0, chosen).unwrap();
    assert!(store.load().unwrap().preferences.cloud_candidates);
}

#[test]
fn candidate_translations_start_off_in_each_edition_and_the_first_document() {
    for edition in crate::edition::Edition::all() {
        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::for_edition(directory.path(), edition);
        let initial = store.load().unwrap();
        assert!(
            !initial.preferences.candidate_translations,
            "{}",
            edition.id
        );
        let first = store.write_first_document().unwrap();
        assert!(!first.preferences.candidate_translations);
        assert_eq!(store.load().unwrap(), first);
        let mut enabled = first.preferences;
        enabled.candidate_translations = true;
        assert!(
            !enabled
                .restored_to_defaults_for(edition)
                .candidate_translations
        );
    }
}

#[test]
fn candidate_translations_keep_legacy_and_explicit_choices() {
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("candidate_translations");
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("preferences.json"),
        serde_json::to_vec(&legacy).unwrap(),
    )
    .unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut current = store.load().unwrap();
    assert!(current.preferences.candidate_translations);
    for enabled in [false, true] {
        let mut chosen = current.preferences;
        chosen.candidate_translations = enabled;
        current = store.save(current.revision, chosen).unwrap();
        assert_eq!(store.load().unwrap(), current);
        assert_eq!(current.preferences.candidate_translations, enabled);
    }
}

#[test]
fn voice_language_default_uses_the_bcp47_region_form() {
    // 共享设置页（React 的 `defaultVoiceInput`）和 macOS 原生回退窗口都写 `zh-CN`。共享默认以前是
    // 小写的 `zh-cn`，同一份文档因此可能带着两种写法，界面和读盘比较只能靠不区分大小写兜住。
    // 统一成区域形式后，宿主和页面显示的是同一个码。
    let defaults = Preferences::default();
    assert_eq!(defaults.voice_input.language, "zh-CN");
    let serialized = serde_json::to_value(&defaults).unwrap();
    assert_eq!(serialized["voice_input"]["language"], "zh-CN");
    assert_eq!(
        serde_json::from_value::<Preferences>(serialized)
            .unwrap()
            .voice_input
            .language,
        "zh-CN"
    );
    // 只改出厂值：用户或旧版本写下的短形式原样保留，不在这里做迁移改写。
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy["voice_input"]["language"] = "zh-cn".into();
    assert_eq!(
        serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .voice_input
            .language,
        "zh-cn"
    );
}

#[test]
fn translation_account_is_an_explicit_choice_on_every_platform() {
    let defaults = Preferences::default();
    assert!(!defaults.translation_account);
    assert!(!defaults.restored_to_defaults().translation_account);
    // A stored document that never chose the account keeps it off.
    let unchosen = Preferences {
        translation_account: false,
        ..defaults.clone()
    };
    let mut serialized = serde_json::to_value(&unchosen).unwrap();
    assert_eq!(
        serialized["translation_account"],
        serde_json::Value::Bool(false)
    );
    serialized
        .as_object_mut()
        .unwrap()
        .remove("translation_account");
    assert!(
        !serde_json::from_value::<Preferences>(serialized.clone())
            .unwrap()
            .translation_account
    );
    let mut malformed = serialized;
    malformed["translation_account"] = "yes".into();
    assert!(serde_json::from_value::<Preferences>(malformed).is_err());

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let chosen = Preferences {
        translation_account: true,
        ..unchosen
    };
    assert!(chosen.validate().is_ok());
    assert_eq!(
        serde_json::to_value(&chosen).unwrap()["translation_account"],
        serde_json::Value::Bool(true)
    );
    let saved = store.save(0, chosen).unwrap();
    assert!(saved.preferences.translation_account);
    let loaded = store.load().unwrap().preferences;
    assert!(loaded.translation_account);
}

#[test]
fn secondary_candidate_translation_language_is_optional_and_round_trips() {
    let defaults = Preferences::default();
    let serialized = serde_json::to_value(&defaults).unwrap();
    assert!(!serialized
        .as_object()
        .unwrap()
        .contains_key("translation_secondary_language"));
    let mut enabled = defaults;
    enabled.translation_secondary_language = Some(TranslationTargetLanguage::Ja);
    let encoded = serde_json::to_string(&enabled).unwrap();
    assert_eq!(
        serde_json::from_str::<Preferences>(&encoded)
            .unwrap()
            .translation_secondary_language,
        Some(TranslationTargetLanguage::Ja)
    );
    let mut legacy_value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    legacy_value
        .as_object_mut()
        .unwrap()
        .remove("translation_secondary_language");
    let legacy = serde_json::from_value::<Preferences>(legacy_value).unwrap();
    assert_eq!(legacy.translation_secondary_language, None);
}

#[test]
fn wubi_code_hint_defaults_on_and_roundtrips() {
    let defaults = Preferences::default();
    assert!(defaults.wubi_code_hint);

    let disabled = Preferences {
        wubi_code_hint: false,
        ..defaults
    };
    assert!(
        !serde_json::from_str::<Preferences>(&serde_json::to_string(&disabled).unwrap())
            .unwrap()
            .wubi_code_hint
    );
}

#[test]
fn wubi_auto_commit_unique_defaults_on_and_roundtrips() {
    let defaults = Preferences::default();
    assert!(defaults.wubi_auto_commit_unique);

    // 没有这个键的文档是本次之前写的，读出来仍然是开：那是当时唯一可能的实际行为。
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("wubi_auto_commit_unique");
    assert!(
        serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .wubi_auto_commit_unique
    );

    let disabled = Preferences {
        wubi_auto_commit_unique: false,
        ..defaults
    };
    assert!(
        !serde_json::from_str::<Preferences>(&serde_json::to_string(&disabled).unwrap())
            .unwrap()
            .wubi_auto_commit_unique
    );
}

#[test]
fn wubi_z_wildcard_defaults_off_and_roundtrips() {
    let defaults = Preferences::default();
    // 来源出厂 `wubi_z_mode = "off"`：新键的缺省必须跟它一致。
    assert!(!defaults.wubi_z_wildcard);

    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy.as_object_mut().unwrap().remove("wubi_z_wildcard");
    assert!(
        !serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .wubi_z_wildcard
    );

    let enabled = Preferences {
        wubi_z_wildcard: true,
        ..defaults
    };
    assert!(
        serde_json::from_str::<Preferences>(&serde_json::to_string(&enabled).unwrap())
            .unwrap()
            .wubi_z_wildcard
    );
}

#[test]
fn wubi_fifth_code_top_commit_defaults_on_and_roundtrips() {
    let defaults = Preferences::default();
    // 来源出厂 `wubi_fifth_code_top_commit = true`，也是接入开关之前唯一可能的行为。
    assert!(defaults.wubi_fifth_code_top_commit);

    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("wubi_fifth_code_top_commit");
    assert!(
        serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .wubi_fifth_code_top_commit
    );

    let disabled = Preferences {
        wubi_fifth_code_top_commit: false,
        ..defaults
    };
    assert!(
        !serde_json::from_str::<Preferences>(&serde_json::to_string(&disabled).unwrap())
            .unwrap()
            .wubi_fifth_code_top_commit
    );
}

#[test]
fn wubi_mixed_pinyin_defaults_off_and_roundtrips() {
    let defaults = Preferences::default();
    assert!(!defaults.wubi_mixed_pinyin);
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy.as_object_mut().unwrap().remove("wubi_mixed_pinyin");
    assert!(
        !serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .wubi_mixed_pinyin
    );

    let enabled = Preferences {
        wubi_mixed_pinyin: true,
        ..defaults
    };
    assert!(
        serde_json::from_str::<Preferences>(&serde_json::to_string(&enabled).unwrap())
            .unwrap()
            .wubi_mixed_pinyin
    );
}

#[test]
fn diagnostic_logging_defaults_off_and_survives_a_round_trip() {
    let defaults = Preferences::default();
    assert!(!defaults.diagnostic_log.server && !defaults.diagnostic_log.tsf);

    // A configuration written before the field existed keeps logging off
    // rather than starting to write a file the user never asked for.
    let mut document = serde_json::to_value(&defaults).unwrap();
    document.as_object_mut().unwrap().remove("diagnostic_log");
    let legacy: Preferences = serde_json::from_value(document).unwrap();
    assert_eq!(legacy.diagnostic_log, DiagnosticLogPreferences::default());

    // The two hosts are separate processes and are enabled separately.
    let preferences = Preferences {
        diagnostic_log: DiagnosticLogPreferences {
            server: true,
            tsf: false,
            command_line_candidates: false,
        },
        ..Preferences::default()
    };
    let restored: Preferences =
        serde_json::from_str(&serde_json::to_string(&preferences).unwrap()).unwrap();
    assert!(restored.diagnostic_log.server && !restored.diagnostic_log.tsf);

    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let saved = store.save(0, preferences).unwrap();
    assert!(saved.preferences.diagnostic_log.server);
    assert_eq!(
        store.load().unwrap().preferences.diagnostic_log,
        saved.preferences.diagnostic_log
    );
}

#[test]
fn command_line_candidates_stay_off_until_the_user_turns_them_on() {
    // 新装和加这一项之前写下的文档都关着：命令行查候选会交出用户的词库，不能因为升级就打开。
    assert!(
        !Preferences::default()
            .diagnostic_log
            .command_line_candidates
    );
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["diagnostic_log"] = serde_json::json!({ "server": true, "tsf": false });
    let legacy: Preferences = serde_json::from_value(document).unwrap();
    assert!(legacy.diagnostic_log.server);
    assert!(!legacy.diagnostic_log.command_line_candidates);

    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut preferences = Preferences::default();
    preferences.diagnostic_log.command_line_candidates = true;
    store.save(0, preferences).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert!(loaded.diagnostic_log.command_line_candidates);
    assert!(!loaded.diagnostic_log.server && !loaded.diagnostic_log.tsf);
}

#[test]
fn fuzzy_pinyin_preserves_disabled_rules_and_rejects_unknown_ids() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let rules = [FuzzyPinyinRule::ZZh, FuzzyPinyinRule::FH]
        .into_iter()
        .collect();
    let fuzzy_pinyin = FuzzyPinyinPreferences {
        enabled: false,
        rules,
        seeded: false,
    };
    assert_eq!(fuzzy_pinyin.active_rules(), 0);
    let disabled = store
        .save(
            0,
            Preferences {
                fuzzy_pinyin,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(
        disabled.preferences.fuzzy_pinyin.rules,
        [FuzzyPinyinRule::ZZh, FuzzyPinyinRule::FH]
            .into_iter()
            .collect()
    );
    let mut enabled = disabled.preferences;
    enabled.fuzzy_pinyin.enabled = true;
    assert_eq!(enabled.fuzzy_pinyin.active_rules(), (1 << 0) | (1 << 4));
    let enabled = store.save(disabled.revision, enabled).unwrap();
    assert_eq!(store.load().unwrap(), enabled);

    let mut invalid = serde_json::to_value(enabled.preferences).unwrap();
    invalid["fuzzy_pinyin"]["rules"] = serde_json::json!(["z-zh", "unsupported"]);
    assert!(serde_json::from_value::<Preferences>(invalid).is_err());
}

#[test]
fn fuzzy_pinyin_first_enable_seeds_once_and_preserves_pruned_rules() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let disabled = store.save(0, Preferences::default()).unwrap();

    let mut first = disabled.preferences.clone();
    first.fuzzy_pinyin.enabled = true;
    let first = store.save(disabled.revision, first).unwrap();
    assert!(first.preferences.fuzzy_pinyin.seeded);
    assert_eq!(first.preferences.fuzzy_pinyin.rules.len(), 11);

    let mut pruned = first.preferences.clone();
    pruned.fuzzy_pinyin.rules = [FuzzyPinyinRule::ZZh].into_iter().collect();
    let pruned = store.save(first.revision, pruned).unwrap();
    let mut disabled = pruned.preferences.clone();
    disabled.fuzzy_pinyin.enabled = false;
    let disabled = store.save(pruned.revision, disabled).unwrap();
    let mut restored = disabled.preferences.clone();
    restored.fuzzy_pinyin.enabled = true;
    let restored = store.save(disabled.revision, restored).unwrap();
    assert_eq!(
        restored.preferences.fuzzy_pinyin.rules,
        [FuzzyPinyinRule::ZZh].into_iter().collect()
    );

    let mut empty = restored.preferences;
    empty.fuzzy_pinyin.enabled = false;
    empty.fuzzy_pinyin.rules.clear();
    let empty = store.save(restored.revision, empty).unwrap();
    let mut empty_enabled = empty.preferences;
    empty_enabled.fuzzy_pinyin.enabled = true;
    let empty_enabled = store.save(empty.revision, empty_enabled).unwrap();
    assert!(empty_enabled.preferences.fuzzy_pinyin.rules.is_empty());
}

#[test]
fn capture_obeys_shared_enablement_and_preserves_corrupt_history() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let file = directory.path().join("clipboard_history.json");
    let preferences = Preferences {
        clipboard_history: false,
        ..Preferences::default()
    };
    let disabled = store.save(0, preferences).unwrap();
    assert!(!store
        .capture_clipboard_text("synthetic disabled".into())
        .unwrap());
    assert!(!file.exists());
    let mut preferences = disabled.preferences;
    preferences.clipboard_history = true;
    let enabled = store.save(disabled.revision, preferences).unwrap();
    assert!(store
        .capture_clipboard_text("synthetic first".into())
        .unwrap());
    assert!(!store
        .capture_clipboard_text("synthetic\0invalid".into())
        .unwrap());
    let history: Vec<crate::clipboard::ClipboardHistoryEntry> =
        serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text, "synthetic first");
    let mut preferences = enabled.preferences;
    preferences.clipboard_history = false;
    let disabled = store.save(enabled.revision, preferences).unwrap();
    assert!(!store
        .capture_clipboard_text("synthetic stopped".into())
        .unwrap());
    let history: Vec<crate::clipboard::ClipboardHistoryEntry> =
        serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text, "synthetic first");
    fs::write(&file, b"broken synthetic document").unwrap();
    let mut preferences = disabled.preferences;
    preferences.clipboard_history = true;
    store.save(disabled.revision, preferences).unwrap();
    assert!(store
        .capture_clipboard_text("synthetic rejected".into())
        .is_err());
    assert_eq!(fs::read(&file).unwrap(), b"broken synthetic document");
}

#[test]
fn default_ime_mode_legacy_defaults_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("default_ime_mode");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), bytes).unwrap();
    // A document written before the field existed takes the platform default: English on Windows, as the source product's factory template, and Chinese everywhere else.
    let expected = if cfg!(windows) {
        DefaultImeMode::English
    } else {
        DefaultImeMode::Chinese
    };
    assert_eq!(DefaultImeMode::default(), expected);
    assert_eq!(Preferences::default().default_ime_mode, expected);
    assert_eq!(store.load().unwrap().preferences.default_ime_mode, expected);
    // An explicit English is still English; only the absent case moved.
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["default_ime_mode"] = "english".into();
    let saved = store
        .save(0, serde_json::from_value(value).unwrap())
        .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.default_ime_mode,
        DefaultImeMode::English
    );
    assert_eq!(saved.preferences.default_ime_mode, DefaultImeMode::English);
}

#[test]
fn local_mode_defaults_and_each_switch_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("local_modes");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.local_modes,
        LocalModePreferences::default()
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    for (revision, key) in [
        "unicode",
        "date_time",
        "quick_phrase",
        "emoji",
        "kaomoji",
        "super_jianpin",
        "temporary_english",
        "temporary_japanese",
    ]
    .iter()
    .enumerate()
    {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value["local_modes"][*key] = false.into();
        let saved = store
            .save(revision as u64, serde_json::from_value(value).unwrap())
            .unwrap();
        assert_eq!(store.load().unwrap(), saved);
    }
}

#[test]
fn generated_local_modes_are_off_by_default_and_when_absent() {
    let defaults = LocalModePreferences::default();
    assert!(
        !defaults.expression && !defaults.command && !defaults.mention && !defaults.mention_places
    );

    // A document without the switches loads with the factory values and the rest as written.
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut sparse = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let local_modes = sparse["preferences"]["local_modes"]
        .as_object_mut()
        .unwrap();
    assert_eq!(local_modes.remove("expression"), Some(false.into()));
    for key in ["command", "mention", "mention_places"] {
        assert_eq!(local_modes.remove(key), Some(false.into()), "{key}");
    }
    local_modes.insert("unicode".into(), false.into());
    let bytes = serde_json::to_vec(&sparse).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let loaded = store.load().unwrap().preferences.local_modes;
    assert_eq!(
        loaded,
        LocalModePreferences {
            unicode: false,
            ..LocalModePreferences::default()
        }
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);

    for (revision, key) in ["expression", "command", "mention", "mention_places"]
        .iter()
        .enumerate()
    {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value["local_modes"][*key] = true.into();
        let saved = store
            .save(revision as u64, serde_json::from_value(value).unwrap())
            .unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, saved);
        let modes = loaded.preferences.local_modes;
        assert_eq!(
            [
                modes.expression,
                modes.command,
                modes.mention,
                modes.mention_places
            ],
            [
                *key == "expression",
                *key == "command",
                *key == "mention",
                *key == "mention_places"
            ]
        );
        let written: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert!(written["preferences"]["local_modes"][*key]
            .as_bool()
            .unwrap());
    }

    // 别的键是更新版本的模式：读得进来，记为未知键，保存时原样写回。
    let mut unknown = serde_json::to_value(Preferences::default()).unwrap();
    unknown["local_modes"]["calculator"] = true.into();
    assert_eq!(unknown_paths(&unknown), [["local_modes", "calculator"]]);
}

#[test]
fn shift_letter_entry_defaults_on_and_survives_a_save() {
    assert!(LocalModePreferences::default().shift_letter_entry);

    // 早于总开关的文档没有这个键，读出来仍是开，其余模式开关照原样。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut sparse = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let local_modes = sparse["preferences"]["local_modes"]
        .as_object_mut()
        .unwrap();
    assert_eq!(local_modes.remove("shift_letter_entry"), Some(true.into()));
    local_modes.insert("date_time".into(), false.into());
    fs::write(store.path(), serde_json::to_vec(&sparse).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences.local_modes;
    assert!(loaded.shift_letter_entry && !loaded.date_time && loaded.unicode);

    // 关掉总开关只改它自己，各模式开关保留。
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["local_modes"]["shift_letter_entry"] = false.into();
    let saved = store
        .save(0, serde_json::from_value(value).unwrap())
        .unwrap();
    let modes = store.load().unwrap();
    assert_eq!(modes, saved);
    let modes = modes.preferences.local_modes;
    assert!(!modes.shift_letter_entry);
    assert_eq!(
        modes,
        LocalModePreferences {
            shift_letter_entry: false,
            ..LocalModePreferences::default()
        }
    );
}

#[test]
fn symbol_mode_defaults_on_and_is_written_only_when_off() {
    // 与 Windows 正式版的出厂配置一致：缺键读作开，文档里也不写这个键。
    assert!(LocalModePreferences::default().symbol);
    let written = serde_json::to_value(Preferences::default()).unwrap();
    assert!(written["local_modes"].get("symbol").is_none());

    // 早于符号模式的文档没有这个键，读出来仍是开，其余模式开关照原样。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut sparse = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let local_modes = sparse["preferences"]["local_modes"]
        .as_object_mut()
        .unwrap();
    assert_eq!(local_modes.remove("symbol"), None);
    local_modes.insert("emoji".into(), false.into());
    fs::write(store.path(), serde_json::to_vec(&sparse).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences.local_modes;
    assert!(loaded.symbol && !loaded.emoji && loaded.unicode);

    // 关掉后文档里写 false，读回来还是关。
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["local_modes"]["symbol"] = false.into();
    store
        .save(0, serde_json::from_value(value).unwrap())
        .unwrap();
    assert!(!store.load().unwrap().preferences.local_modes.symbol);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(on_disk["preferences"]["local_modes"]["symbol"], false);
}

#[test]
fn chaizi_is_unset_until_the_user_flips_it() {
    // 缺省是 `None`：host-api 按宿主形态读缺键（触屏开、桌面关），文档里也不写这个键，旧版本照常读得懂。
    assert_eq!(LocalModePreferences::default().chaizi, None);
    let written = serde_json::to_value(Preferences::default()).unwrap();
    assert!(written["local_modes"].get("chaizi").is_none());

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let snapshot = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    fs::write(store.path(), serde_json::to_vec(&snapshot).unwrap()).unwrap();
    assert_eq!(store.load().unwrap().preferences.local_modes.chaizi, None);

    // 用户动过开关后，明确的布尔值保存后原样读回。
    for (revision, flag) in [false, true].into_iter().enumerate() {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        value["local_modes"]["chaizi"] = flag.into();
        let saved = store
            .save(revision as u64, serde_json::from_value(value).unwrap())
            .unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded, saved);
        assert_eq!(loaded.preferences.local_modes.chaizi, Some(flag));
        let on_disk: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(on_disk["preferences"]["local_modes"]["chaizi"], flag);
    }
}

/// 序列化默认值后删掉一个键，等价于「早于这个键的文档」。
fn modes_without(key: &str) -> LocalModePreferences {
    let mut value = serde_json::to_value(LocalModePreferences::default()).unwrap();
    value.as_object_mut().unwrap().remove(key);
    serde_json::from_value(value).unwrap()
}

/// emoji / 颜文字快捷模式是必填成员，缺键的 `local_modes` 整份读不进、退回全默认（与旧版本一致）；文档里写明的值原样生效。Windows v0.11.1 出厂关，本仓出厂值为开，差异已记录、是否对齐留待产品决策。
#[test]
fn emoji_and_kaomoji_modes_default_on_and_keep_written_values() {
    let defaults = LocalModePreferences::default();
    assert!(defaults.emoji && defaults.kaomoji);
    for flag in [true, false] {
        let modes: LocalModePreferences = serde_json::from_value(serde_json::json!({
            "unicode": true, "date_time": true, "quick_phrase": true,
            "emoji": flag, "kaomoji": flag, "super_jianpin": true,
            "temporary_english": true, "temporary_japanese": true
        }))
        .unwrap();
        assert_eq!([modes.emoji, modes.kaomoji], [flag, flag], "{flag}");
    }
}

/// V 模式（Windows `utility.v_mode` 出厂开）本仓出厂值为关，缺键读作关；差异已记录。文档里写明的开不被覆盖。
#[test]
fn expression_mode_defaults_off() {
    assert!(!LocalModePreferences::default().expression);
    assert!(!modes_without("expression").expression);
    let mut value = serde_json::to_value(LocalModePreferences::default()).unwrap();
    value["expression"] = true.into();
    let modes: LocalModePreferences = serde_json::from_value(value).unwrap();
    assert!(modes.expression);
}

/// 行内日期时间混入（对应 Windows `utility.date_time_candidates`，出厂关）与 T 模式（date_time）是两个独立开关；缺键读作开——此前由 `date_time` 一并控制且默认开，保持同样的有效行为，开着时不写进文档。
#[test]
fn date_time_inline_defaults_on_and_is_written_only_when_off() {
    assert!(LocalModePreferences::default().date_time_inline);
    let written = serde_json::to_value(LocalModePreferences::default()).unwrap();
    assert!(written.get("date_time_inline").is_none());
    assert!(modes_without("date_time_inline").date_time_inline);
    assert!(modes_without("date_time_inline").date_time_menu);
    let mut value = serde_json::to_value(LocalModePreferences::default()).unwrap();
    value["date_time_inline"] = false.into();
    let modes: LocalModePreferences = serde_json::from_value(value).unwrap();
    assert!(!modes.date_time_inline);
}

/// 左右键候选导航（Windows `general.candidate_horizontal_arrow_navigation` 出厂关）本仓出厂值为开，缺键读作开；差异已记录。文档里写明的关不被覆盖。
#[test]
fn left_right_navigation_defaults_on() {
    assert!(NavigationPreferences::default().left_right);
    let mut value = serde_json::to_value(NavigationPreferences::default()).unwrap();
    value.as_object_mut().unwrap().remove("left_right");
    let navigation: NavigationPreferences = serde_json::from_value(value).unwrap();
    assert!(navigation.left_right && navigation.arrows);
    let mut value = serde_json::to_value(NavigationPreferences::default()).unwrap();
    value["left_right"] = false.into();
    let navigation: NavigationPreferences = serde_json::from_value(value).unwrap();
    assert!(!navigation.left_right);
}

/// 符号混输（Windows `general.symbol_mixed_input` 出厂关）本仓出厂值为开，缺键读作开、开着不写进文档；差异已记录。文档里写明的关不被覆盖。
#[test]
fn mixed_symbols_defaults_on() {
    assert!(MixedInputPreferences::default().symbols);
    let mut value = serde_json::to_value(MixedInputPreferences::default()).unwrap();
    value.as_object_mut().unwrap().remove("symbols");
    let mixed: MixedInputPreferences = serde_json::from_value(value).unwrap();
    assert!(mixed.symbols);
    let mut value = serde_json::to_value(MixedInputPreferences::default()).unwrap();
    value["symbols"] = false.into();
    let mixed: MixedInputPreferences = serde_json::from_value(value).unwrap();
    assert!(!mixed.symbols);
}

/// 辅助码开关（Windows v0.11.1 出厂关、门禁 pin 出厂开）本仓出厂值为开，缺键读作开；差异已记录。文档里写明的值原样生效。
#[test]
fn helpcode_enabled_defaults_on() {
    assert!(HelpcodePreferences::default().enabled);
    assert!(default_quanpin_helpcode().enabled && default_shuangpin_helpcode().enabled);
    let mut value = serde_json::to_value(default_shuangpin_helpcode()).unwrap();
    value.as_object_mut().unwrap().remove("enabled");
    let helpcode: HelpcodePreferences = serde_json::from_value(value).unwrap();
    assert!(helpcode.enabled);
    let mut value = serde_json::to_value(default_quanpin_helpcode()).unwrap();
    value.as_object_mut().unwrap().remove("enabled");
    let helpcode: HelpcodePreferences = serde_json::from_value(value).unwrap();
    assert!(helpcode.enabled);
    for flag in [true, false] {
        let helpcode: HelpcodePreferences = serde_json::from_value(serde_json::json!({
            "enabled": flag, "schema": "lantian"
        }))
        .unwrap();
        assert_eq!(helpcode.enabled, flag, "{flag}");
    }
}

/// Windows `appearance.candidate_window_fit_content` 出厂关（post-pin 键）：缺键读作关，开着时才写进文档；文档里写明的开不被覆盖。
#[test]
fn candidate_window_fit_content_defaults_off() {
    let preferences = Preferences::default();
    assert!(!preferences.candidate_window_fit_content);
    let written = serde_json::to_value(&preferences).unwrap();
    assert!(written.get("candidate_window_fit_content").is_none());
    let mut value = written.clone();
    value["candidate_window_fit_content"] = true.into();
    let loaded: Preferences = serde_json::from_value(value).unwrap();
    assert!(loaded.candidate_window_fit_content);
}

#[test]
fn plugin_preferences_are_off_by_default_and_absent_from_older_documents() {
    let defaults = PluginPreferences::default();
    assert!(!defaults.key_sound.enabled);
    assert_eq!(defaults.key_sound.mode, KeySoundMode::Keys);
    assert_eq!(
        defaults.key_sound.pack,
        crate::preferences::DEFAULT_SOUND_PACK
    );
    assert_eq!(
        defaults.melody.pack,
        crate::preferences::DEFAULT_MELODY_PACK
    );
    assert!(
        !defaults.commit_sound.enabled && !defaults.music.enabled && !defaults.achievements.enabled
    );
    assert!(defaults.music.pack.is_empty() && defaults.command_tables.is_empty());
    assert_eq!(defaults.effect_style, crate::preferences::EffectStyle::Off);
    assert_eq!(defaults.effect_intensity, 50);
    assert!(defaults.effect_pack.is_empty());
    assert!(defaults.phrase_tables.is_empty());
    assert!(
        defaults.helpcode_pack_quanpin.is_empty() && defaults.helpcode_pack_shuangpin.is_empty()
    );
    assert!(!defaults.combo_counter && !defaults.combo_tier_sound);

    // Untouched, the section is not written, so a build from before plugins reads the document; a document from before plugins loads with them off and is not rewritten.
    let serialized = serde_json::to_value(Preferences::default()).unwrap();
    assert!(serialized.get("plugins").is_none());
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let legacy = serde_json::to_vec(&PreferencesSnapshot::default()).unwrap();
    fs::write(store.path(), &legacy).unwrap();
    assert_eq!(store.load().unwrap().preferences.plugins, defaults);
    assert_eq!(fs::read(store.path()).unwrap(), legacy);

    // A partial section fills the rest from the defaults.
    let mut value = serialized.clone();
    value["plugins"] = serde_json::json!({ "key_sound": { "enabled": true } });
    let partial: Preferences = serde_json::from_value(value).unwrap();
    assert!(partial.plugins.key_sound.enabled);
    assert_eq!(partial.plugins.key_sound.volume, defaults.key_sound.volume);
    assert_eq!(partial.plugins.melody, defaults.melody);
    assert_eq!(partial.plugins.effect_style, defaults.effect_style);
    assert_eq!(partial.plugins.effect_intensity, 50);

    // Every part round-trips through the store.
    let chosen = Preferences {
        plugins: PluginPreferences {
            key_sound: KeySoundPreferences {
                enabled: true,
                mode: KeySoundMode::Melody,
                pack: "typewriter".into(),
                volume: 100,
            },
            commit_sound: CommitSoundPreferences { enabled: true },
            melody: MelodyPreferences {
                pack: "scale".into(),
            },
            music: MusicPreferences {
                enabled: true,
                pack: "rain".into(),
                volume: 0,
            },
            achievements: AchievementPreferences { enabled: true },
            command_tables: vec!["sig".into(), "work.notes".into()],
            effect_style: crate::preferences::EffectStyle::PowerMode,
            effect_intensity: 100,
            effect_pack: "neon".into(),
            combo_counter: true,
            combo_tier_sound: true,
            phrase_tables: vec!["office".into(), "names".into()],
            helpcode_pack_quanpin: "radicals".into(),
            helpcode_pack_shuangpin: "strokes".into(),
        },
        ..Preferences::default()
    };
    let saved = store.save(0, chosen.clone()).unwrap();
    assert_eq!(saved.preferences.plugins, chosen.plugins);
    assert_eq!(store.load().unwrap().preferences.plugins, chosen.plugins);
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        written["preferences"]["plugins"]["key_sound"]["mode"],
        "melody"
    );
    let plugins = &written["preferences"]["plugins"];
    assert_eq!(plugins["effect_style"], "power_mode");
    assert_eq!(plugins["effect_intensity"], 100);
    assert_eq!(plugins["effect_pack"], "neon");
    // No pack selected: the key is left out, so a build from before effect packs reads the section.
    let mut unselected = chosen.clone();
    unselected.plugins.effect_pack.clear();
    let unselected = serde_json::to_value(&unselected).unwrap();
    assert!(unselected["plugins"].get("effect_pack").is_none());
    assert_eq!(
        plugins["phrase_tables"],
        serde_json::json!(["office", "names"])
    );
    assert_eq!(plugins["helpcode_pack_quanpin"], "radicals");
    assert_eq!(plugins["helpcode_pack_shuangpin"], "strokes");
    // 没选短语表和辅助码表包时这三个键都不写，没有它们的旧版本照样能读。
    let mut unselected = chosen.clone();
    unselected.plugins.phrase_tables.clear();
    unselected.plugins.helpcode_pack_quanpin.clear();
    unselected.plugins.helpcode_pack_shuangpin.clear();
    let unselected = serde_json::to_value(&unselected).unwrap();
    for key in [
        "phrase_tables",
        "helpcode_pack_quanpin",
        "helpcode_pack_shuangpin",
    ] {
        assert!(unselected["plugins"].get(key).is_none(), "{key}");
    }
    assert_eq!(plugins["combo_counter"], true);
    assert_eq!(plugins["combo_tier_sound"], true);
    // Restoring defaults turns them all off again.
    assert_eq!(chosen.restored_to_defaults().plugins, defaults);

    // Closed to anything else, at every level.
    for (path, key) in [
        (None, "scripts"),
        (Some("key_sound"), "command"),
        (Some("music"), "url"),
    ] {
        let mut value = serialized.clone();
        value["plugins"] = serde_json::json!({});
        match path {
            None => value["plugins"][key] = true.into(),
            Some(section) => value["plugins"][section] = serde_json::json!({ key: true }),
        }
        let expected: Vec<&str> = path.into_iter().chain([key]).collect();
        assert_eq!(
            unknown_paths(&value),
            [[&["plugins"][..], &expected].concat()],
            "{key}"
        );
    }
    let mut value = serialized.clone();
    value["plugins"] = serde_json::json!({ "key_sound": { "mode": "random" } });
    assert!(serde_json::from_value::<Preferences>(value).is_err());
    let mut value = serialized;
    value["plugins"] = serde_json::json!({ "effect_style": "rainbow" });
    assert!(serde_json::from_value::<Preferences>(value).is_err());
}

#[test]
fn plugin_preferences_are_validated() {
    let invalid: [fn(&mut PluginPreferences); 16] = [
        |plugins| plugins.key_sound.volume = 101,
        |plugins| plugins.effect_intensity = 101,
        |plugins| plugins.music.volume = 255,
        |plugins| plugins.key_sound.pack = "../default".into(),
        |plugins| plugins.melody.pack = "Twinkle".into(),
        |plugins| plugins.music.pack = "rain/..".into(),
        |plugins| plugins.effect_pack = "../neon".into(),
        |plugins| plugins.command_tables = vec!["sig".into(), "sig".into()],
        |plugins| plugins.command_tables = vec![String::new()],
        |plugins| {
            plugins.command_tables = (0..=PluginPreferences::MAX_COMMAND_TABLES)
                .map(|index| format!("table{index}"))
                .collect()
        },
        |plugins| plugins.phrase_tables = vec!["office".into(), "office".into()],
        |plugins| plugins.phrase_tables = vec![String::new()],
        |plugins| plugins.phrase_tables = vec!["../office".into()],
        |plugins| {
            plugins.phrase_tables = (0..=PluginPreferences::MAX_PHRASE_TABLES)
                .map(|index| format!("phrases{index}"))
                .collect()
        },
        |plugins| plugins.helpcode_pack_quanpin = "Radicals".into(),
        |plugins| plugins.helpcode_pack_shuangpin = "../strokes".into(),
    ];
    for (index, change) in invalid.into_iter().enumerate() {
        let mut preferences = Preferences::default();
        change(&mut preferences.plugins);
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidPlugins)
            ),
            "{index}"
        );
    }
    // An empty pack id means none chosen.
    let mut preferences = Preferences::default();
    preferences.plugins.key_sound.pack.clear();
    preferences.plugins.melody.pack.clear();
    preferences.plugins.command_tables = (0..PluginPreferences::MAX_COMMAND_TABLES)
        .map(|index| format!("table{index}"))
        .collect();
    assert!(preferences.validate().is_ok());
}

#[test]
fn game_compatibility_defaults_on_and_stays_out_of_the_document() {
    let defaults = GameCompatibilityPreferences::default();
    assert!(defaults.candidate_overlay);
    assert!(defaults.overlay_processes.is_empty() && defaults.excluded_processes.is_empty());
    assert_eq!(Preferences::default().game_compatibility, defaults);

    // 等于默认值时不写进文档，没有这个键的旧版本照样能读；缺这个键的文档读成默认值，读取时也不改写文件。
    let serialized = serde_json::to_value(Preferences::default()).unwrap();
    assert!(serialized.get("game_compatibility").is_none());
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let legacy = serde_json::to_vec(&PreferencesSnapshot::default()).unwrap();
    fs::write(store.path(), &legacy).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.game_compatibility,
        defaults
    );
    assert_eq!(fs::read(store.path()).unwrap(), legacy);

    // 只写了一部分的段落，其余各项取默认值。
    let mut value = serialized.clone();
    value["game_compatibility"] = serde_json::json!({ "overlay_processes": ["game.exe"] });
    let partial: Preferences = serde_json::from_value(value).unwrap();
    assert!(partial.game_compatibility.candidate_overlay);
    assert_eq!(partial.game_compatibility.overlay_processes, ["game.exe"]);
    assert!(partial.game_compatibility.excluded_processes.is_empty());

    // 关掉开关、写下两张表，保存再读回来原样不变。
    let chosen = Preferences {
        game_compatibility: GameCompatibilityPreferences {
            candidate_overlay: false,
            overlay_processes: vec!["Game.EXE".into(), "my game.exe".into()],
            excluded_processes: vec!["launcher.exe".into()],
        },
        ..Preferences::default()
    };
    let written = serde_json::to_value(&chosen).unwrap();
    assert_eq!(
        written["game_compatibility"],
        serde_json::json!({
            "candidate_overlay": false,
            "overlay_processes": ["Game.EXE", "my game.exe"],
            "excluded_processes": ["launcher.exe"],
        })
    );
    let saved = store.save(0, chosen.clone()).unwrap();
    assert_eq!(
        saved.preferences.game_compatibility,
        chosen.game_compatibility
    );
    assert_eq!(
        store.load().unwrap().preferences.game_compatibility,
        chosen.game_compatibility
    );
    // 恢复默认时回到开启、两张表清空。
    assert_eq!(chosen.restored_to_defaults().game_compatibility, defaults);

    // 段落里多出的键记为未知键，取值不合规仍然读不进来。
    let mut value = serialized.clone();
    value["game_compatibility"] = serde_json::json!({ "auto_rules": true });
    assert_eq!(
        unknown_paths(&value),
        [["game_compatibility", "auto_rules"]]
    );
    let mut value = serialized;
    value["game_compatibility"] = serde_json::json!({ "candidate_overlay": "yes" });
    assert!(serde_json::from_value::<Preferences>(value).is_err());
}

#[test]
fn game_compatibility_process_names_are_validated() {
    let too_many = |game: &mut GameCompatibilityPreferences| {
        game.overlay_processes = (0..GameCompatibilityPreferences::MAX_PROCESSES / 2)
            .map(|index| format!("overlay{index}.exe"))
            .collect();
        game.excluded_processes = (0..=GameCompatibilityPreferences::MAX_PROCESSES / 2)
            .map(|index| format!("excluded{index}.exe"))
            .collect();
    };
    let invalid: [fn(&mut GameCompatibilityPreferences); 18] = [
        |game| game.overlay_processes = vec![String::new()],
        |game| game.overlay_processes = vec!["game".into()],
        |game| game.overlay_processes = vec!["game.exe.bak".into()],
        |game| game.overlay_processes = vec!["game.ex".into()],
        |game| game.overlay_processes = vec![r"C:\Games\game.exe".into()],
        |game| game.overlay_processes = vec!["games/game.exe".into()],
        |game| game.overlay_processes = vec!["c:game.exe".into()],
        |game| game.overlay_processes = vec!["*.exe".into()],
        |game| game.overlay_processes = vec!["game?.exe".into()],
        |game| game.overlay_processes = vec!["\"game\".exe".into()],
        |game| game.excluded_processes = vec!["<game>.exe".into()],
        |game| game.excluded_processes = vec!["a|b.exe".into()],
        |game| game.excluded_processes = vec!["game\n.exe".into()],
        |game| game.excluded_processes = vec!["game\u{7f}.exe".into()],
        |game| game.overlay_processes = vec![format!("{}.exe", "a".repeat(61))],
        |game| game.overlay_processes = vec!["game.exe".into(), "game.exe".into()],
        // 两张表之间只差大小写也算重复：DLL 不分大小写地比较进程名。
        |game| {
            game.overlay_processes = vec!["Game.exe".into()];
            game.excluded_processes = vec!["GAME.EXE".into()];
        },
        too_many,
    ];
    for (index, change) in invalid.into_iter().enumerate() {
        let mut preferences = Preferences::default();
        change(&mut preferences.game_compatibility);
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidGameCompatibility)
            ),
            "{index}"
        );
    }

    // 恰好 64 个字符（按字符而不是字节计）、大写后缀、名字里有空格和非 ASCII 字符、合计恰好 32 条，都是合法的。
    let mut preferences = Preferences::default();
    preferences.game_compatibility.overlay_processes = vec![
        format!("{}.exe", "a".repeat(60)),
        format!("{}.exe", "游".repeat(60)),
        "League of Legends.EXE".into(),
    ];
    preferences.game_compatibility.excluded_processes = (3
        ..GameCompatibilityPreferences::MAX_PROCESSES)
        .map(|index| format!("excluded{index}.exe"))
        .collect();
    assert!(preferences.validate().is_ok());

    // 只差非 ASCII 大小写的两项不算重复：去重按 ASCII 折叠，和 WinUI 设置页的检查一致，否则设置页放行的条目会在保存时被拒。
    let mut preferences = Preferences::default();
    preferences.game_compatibility.overlay_processes = vec!["Ä.exe".into()];
    preferences.game_compatibility.excluded_processes = vec!["ä.exe".into()];
    assert!(preferences.validate().is_ok());

    // 拒绝的文档不会被保存。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut rejected = Preferences::default();
    rejected.game_compatibility.overlay_processes = vec!["game".into()];
    assert!(matches!(
        store.save(0, rejected),
        Err(PreferencesError::InvalidGameCompatibility)
    ));
    assert!(!store.path().exists());
}

#[test]
fn app_input_mode_rules_stay_out_of_the_document_until_written() {
    // 没有规则时不写这个键，没有它的旧版本照样能读；缺这个键的文档读成空表。
    let serialized = serde_json::to_value(Preferences::default()).unwrap();
    assert!(serialized.get("app_input_mode_rules").is_none());
    assert!(Preferences::default().app_input_mode_rules.is_empty());

    // macOS 的 bundle id 和 Windows 的进程基名放在同一张表里，保存再读回原样不变。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut chosen = Preferences::default();
    chosen
        .app_input_mode_rules
        .insert("com.apple.Terminal".into(), AppInputModeRule::English);
    chosen
        .app_input_mode_rules
        .insert("code.exe".into(), AppInputModeRule::English);
    chosen
        .app_input_mode_rules
        .insert("wechat.exe".into(), AppInputModeRule::Chinese);
    let written = serde_json::to_value(&chosen).unwrap();
    assert_eq!(
        written["app_input_mode_rules"],
        serde_json::json!({
            "code.exe": "english",
            "com.apple.Terminal": "english",
            "wechat.exe": "chinese",
        })
    );
    store.save(0, chosen.clone()).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.app_input_mode_rules,
        chosen.app_input_mode_rules
    );
    assert!(chosen
        .restored_to_defaults()
        .app_input_mode_rules
        .is_empty());

    // 模式只认 chinese 和 english。
    let mut value = serialized;
    value["app_input_mode_rules"] = serde_json::json!({ "code.exe": "global" });
    assert!(serde_json::from_value::<Preferences>(value).is_err());
}

#[test]
fn app_input_mode_rule_identifiers_are_validated() {
    let invalid: [fn(&mut std::collections::BTreeMap<String, AppInputModeRule>); 8] = [
        |rules| {
            rules.insert(String::new(), AppInputModeRule::English);
        },
        |rules| {
            rules.insert(" code.exe".into(), AppInputModeRule::English);
        },
        |rules| {
            rules.insert(
                r"C:\Program Files\code.exe".into(),
                AppInputModeRule::English,
            );
        },
        |rules| {
            rules.insert("apps/code".into(), AppInputModeRule::English);
        },
        |rules| {
            rules.insert("code\n.exe".into(), AppInputModeRule::English);
        },
        |rules| {
            rules.insert(
                "a".repeat(MAX_APP_INPUT_MODE_RULE_ID_BYTES + 1),
                AppInputModeRule::English,
            );
        },
        // Windows 不分大小写地比较进程名，只差大小写的两条会让同一个进程拿到两种模式。
        |rules| {
            rules.insert("Code.exe".into(), AppInputModeRule::English);
            rules.insert("code.exe".into(), AppInputModeRule::Chinese);
        },
        |rules| {
            for index in 0..=MAX_APP_INPUT_MODE_RULES {
                rules.insert(format!("app{index}.exe"), AppInputModeRule::English);
            }
        },
    ];
    for (index, change) in invalid.into_iter().enumerate() {
        let mut preferences = Preferences::default();
        change(&mut preferences.app_input_mode_rules);
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidAppInputModeRules)
            ),
            "{index}"
        );
    }

    // 恰好上限的条数和字节数、带空格和非 ASCII 字符的程序名都合法。
    let mut preferences = Preferences::default();
    for index in 0..MAX_APP_INPUT_MODE_RULES - 2 {
        preferences
            .app_input_mode_rules
            .insert(format!("app{index}.exe"), AppInputModeRule::Chinese);
    }
    preferences.app_input_mode_rules.insert(
        "a".repeat(MAX_APP_INPUT_MODE_RULE_ID_BYTES),
        AppInputModeRule::English,
    );
    preferences
        .app_input_mode_rules
        .insert("微信 beta.exe".into(), AppInputModeRule::English);
    assert!(preferences.validate().is_ok());

    // 拒绝的文档不会被保存。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut rejected = Preferences::default();
    rejected
        .app_input_mode_rules
        .insert("apps/code".into(), AppInputModeRule::English);
    assert!(matches!(
        store.save(0, rejected),
        Err(PreferencesError::InvalidAppInputModeRules)
    ));
    assert!(!store.path().exists());
}

#[test]
fn vim_mode_apps_stay_out_of_the_document_until_written() {
    // 没有名单时不写这个键，没有它的旧版本照样能读；缺这个键的文档读成空表。
    let serialized = serde_json::to_value(Preferences::default()).unwrap();
    assert!(serialized.get("vim_mode_apps").is_none());
    assert!(Preferences::default().vim_mode_apps.is_empty());

    // macOS 的 bundle id 和 Windows 的进程基名放在同一份名单里，保存再读回原样不变。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let chosen = Preferences {
        vim_mode_apps: vec![
            "com.apple.Terminal".into(),
            "code.exe".into(),
            "nvim.exe".into(),
        ],
        ..Preferences::default()
    };
    let written = serde_json::to_value(&chosen).unwrap();
    assert_eq!(
        written["vim_mode_apps"],
        serde_json::json!(["com.apple.Terminal", "code.exe", "nvim.exe"])
    );
    store.save(0, chosen.clone()).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.vim_mode_apps,
        chosen.vim_mode_apps
    );
    assert!(chosen.restored_to_defaults().vim_mode_apps.is_empty());

    // 类型非法（不是字符串数组）时整个文档读不出。
    let mut value = serde_json::to_value(Preferences::default()).unwrap();
    value["vim_mode_apps"] = serde_json::json!({ "code.exe": true });
    assert!(serde_json::from_value::<Preferences>(value).is_err());
}

#[test]
fn vim_mode_app_identifiers_are_validated() {
    let invalid: [fn(&mut Vec<String>); 8] = [
        |apps| apps.push(String::new()),
        |apps| apps.push(" code.exe".into()),
        |apps| apps.push(r"C:\Program Files\code.exe".into()),
        |apps| apps.push("apps/code".into()),
        |apps| apps.push("code\n.exe".into()),
        |apps| apps.push("a".repeat(MAX_APP_INPUT_MODE_RULE_ID_BYTES + 1)),
        // 与应用例外同一口径：只差大小写的两条命中同一个应用。
        |apps| {
            apps.push("Code.exe".into());
            apps.push("code.exe".into());
        },
        |apps| {
            for index in 0..=MAX_APP_INPUT_MODE_RULES {
                apps.push(format!("app{index}.exe"));
            }
        },
    ];
    for (index, change) in invalid.into_iter().enumerate() {
        let mut preferences = Preferences::default();
        change(&mut preferences.vim_mode_apps);
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidVimModeApps)
            ),
            "{index}"
        );
    }

    // 恰好上限的条数和字节数、带空格和非 ASCII 字符的名字都合法。
    let mut preferences = Preferences::default();
    for index in 0..MAX_APP_INPUT_MODE_RULES - 2 {
        preferences.vim_mode_apps.push(format!("app{index}.exe"));
    }
    preferences
        .vim_mode_apps
        .push("a".repeat(MAX_APP_INPUT_MODE_RULE_ID_BYTES));
    preferences.vim_mode_apps.push("终端 beta.app".into());
    assert!(preferences.validate().is_ok());

    // 拒绝的文档不会被保存。
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let rejected = Preferences {
        vim_mode_apps: vec!["apps/code".into()],
        ..Preferences::default()
    };
    assert!(matches!(
        store.save(0, rejected),
        Err(PreferencesError::InvalidVimModeApps)
    ));
    assert!(!store.path().exists());
}

#[test]
fn appearance_preferences_absent_defaults_and_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut sparse = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    for key in [
        "theme",
        "settings_theme",
        "toolbar_theme",
        "screen_keyboard_theme",
        "global_theme",
        "custom_theme",
        "candidate_follow_cursor",
        "input_mode_hud",
        "input_mode_focus_hud",
        "input_mode_status_item",
        "expand_candidates",
        "show_candidate_expand_button",
    ] {
        sparse["preferences"].as_object_mut().unwrap().remove(key);
    }
    let bytes = serde_json::to_vec(&sparse).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let loaded = store.load().unwrap();
    // 聚焦提示与菜单栏状态项都缺省关闭，旧文档和新装相同。
    assert!(!Preferences::default().input_mode_focus_hud);
    assert!(!loaded.preferences.input_mode_focus_hud);
    assert!(!Preferences::default().input_mode_status_item);
    assert!(!loaded.preferences.input_mode_status_item);
    // 「展开候选」同样是新加的开关，缺字段的旧文档和新装都读成关闭。
    assert!(!Preferences::default().expand_candidates);
    assert!(!loaded.preferences.expand_candidates);
    assert!(loaded.preferences.show_candidate_expand_button);
    // 缺这两项的旧文档按类型默认值读，保持跟随系统；触屏构建的新安装才用薄荷晨光（#2178），所以这里不能直接比 `Preferences::default()`。
    let mut expected = PreferencesSnapshot::default();
    expected.preferences.global_theme = crate::theme::GlobalTheme::default();
    expected.preferences.custom_theme = CustomTheme::default();
    assert_eq!(loaded, expected);
    assert_eq!(
        loaded.preferences.quanpin_helpcode,
        default_quanpin_helpcode()
    );
    assert_eq!(
        loaded.preferences.shuangpin_helpcode,
        default_shuangpin_helpcode()
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    let preferences = Preferences {
        theme: ThemeMode::Light,
        settings_theme: SettingsTheme::Dark,
        toolbar_theme: SettingsTheme::Light,
        candidate_follow_cursor: false,
        input_mode_hud: false,
        input_mode_focus_hud: true,
        input_mode_status_item: true,
        expand_candidates: true,
        show_candidate_expand_button: false,
        ..Preferences::default()
    };
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert!(store.load().unwrap().preferences.input_mode_focus_hud);
    assert!(
        !store
            .load()
            .unwrap()
            .preferences
            .restored_to_defaults()
            .input_mode_focus_hud
    );
    assert!(store.load().unwrap().preferences.input_mode_status_item);
    assert!(store.load().unwrap().preferences.expand_candidates);
    assert!(
        !store
            .load()
            .unwrap()
            .preferences
            .show_candidate_expand_button
    );
}

#[test]
fn character_width_defaults_for_legacy_files_and_roundtrips() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("character_width");
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.character_width,
        CharacterWidthPreference::Halfwidth
    );

    let saved = store
        .save(
            0,
            Preferences {
                character_width: CharacterWidthPreference::Fullwidth,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.character_width,
        CharacterWidthPreference::Fullwidth
    );
    let document = serde_json::to_value(saved).unwrap();
    assert_eq!(document["preferences"]["character_width"], "fullwidth");

    let mut invalid = document["preferences"].clone();
    invalid["character_width"] = "invalid".into();
    assert!(serde_json::from_value::<Preferences>(invalid).is_err());
}

#[test]
fn toolbar_theme_roundtrips_independently_and_rejects_unknown_values() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    for (revision, toolbar_theme) in [
        SettingsTheme::Dark,
        SettingsTheme::Light,
        SettingsTheme::Follow,
    ]
    .into_iter()
    .enumerate()
    {
        let preferences = Preferences {
            theme: ThemeMode::System,
            settings_theme: SettingsTheme::Light,
            candidate_theme: SettingsTheme::Dark,
            toolbar_theme,
            ..Preferences::default()
        };
        let saved = store.save(revision as u64, preferences).unwrap();
        assert_eq!(store.load().unwrap(), saved);
    }
    let mut invalid = serde_json::to_value(Preferences::default()).unwrap();
    invalid["toolbar_theme"] = "system".into();
    assert!(serde_json::from_value::<Preferences>(invalid).is_err());
}

#[test]
fn screen_keyboard_theme_roundtrips_independently() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    for (revision, screen_keyboard_theme) in [
        SettingsTheme::Dark,
        SettingsTheme::Light,
        SettingsTheme::Follow,
    ]
    .into_iter()
    .enumerate()
    {
        let preferences = Preferences {
            theme: ThemeMode::System,
            toolbar_theme: SettingsTheme::Dark,
            screen_keyboard_theme,
            ..Preferences::default()
        };
        let saved = store.save(revision as u64, preferences).unwrap();
        assert_eq!(store.load().unwrap(), saved);
    }
    let mut invalid = serde_json::to_value(Preferences::default()).unwrap();
    invalid["screen_keyboard_theme"] = "system".into();
    assert!(serde_json::from_value::<Preferences>(invalid).is_err());
}

#[test]
fn global_theme_ids_round_trip_and_reject_unknown_ids() {
    use crate::theme::GlobalTheme;
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    // 新安装在所有构建上都跟随系统。
    assert_eq!(Preferences::default().global_theme, GlobalTheme::System);
    for (revision, global_theme) in GlobalTheme::ALL.into_iter().enumerate() {
        let preferences = Preferences {
            global_theme,
            ..Preferences::default()
        };
        let saved = store.save(revision as u64, preferences).unwrap();
        assert_eq!(saved.preferences.global_theme, global_theme);
        assert_eq!(store.load().unwrap(), saved);
        let value = serde_json::to_value(&saved.preferences).unwrap();
        assert_eq!(value["global_theme"], global_theme.id());
    }
    for retired in ["willow_green", "fluent", "forest", "ocean", "Night", ""] {
        let mut invalid = serde_json::to_value(Preferences::default()).unwrap();
        invalid["global_theme"] = retired.into();
        assert!(
            serde_json::from_value::<Preferences>(invalid).is_err(),
            "{retired}"
        );
    }
    for removed in [
        "candidate_skin",
        "touch_keyboard_skin",
        "custom_touch_keyboard_skin",
        "candidate_text_color",
        "candidate_number_color",
        "candidate_accent_color",
        "candidate_selected_color",
        "candidate_hover_color",
        "candidate_surface_color",
        "candidate_border_color",
    ] {
        // 退役的键不再是设置：读到时和更新版本的键一样记为未知键，不影响任何字段。
        let mut retired = serde_json::to_value(Preferences::default()).unwrap();
        retired[removed] = serde_json::Value::Null;
        assert_eq!(unknown_paths(&retired), [[removed]], "{removed}");
    }
}

#[test]
fn custom_theme_round_trips_every_part() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let default = serde_json::to_value(CustomTheme::default()).unwrap();
    assert_eq!(
        default.as_object().unwrap().keys().collect::<Vec<_>>(),
        Vec::<&String>::new()
    );
    let custom_theme = CustomTheme {
        base: crate::theme::GlobalTheme::Paper,
        candidate_skin: Some("sakura.v2".into()),
        candidate_skin_dark: Some("night-sakura".into()),
        candidate_colors: CustomCandidateColors {
            text: Some("#101010".into()),
            number: Some("#202020".into()),
            accent: Some("#303030".into()),
            selected: Some("#404040".into()),
            hover: Some("#505050".into()),
            surface: Some("#606060".into()),
            border: Some("#707070".into()),
        },
        keyboard: Some(TouchKeyboardSkinDesign {
            background: 0x123456,
            photo: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".into()),
            ..TouchKeyboardSkinDesign::default()
        }),
    };
    let saved = store
        .save(
            0,
            Preferences {
                global_theme: crate::theme::GlobalTheme::Custom,
                custom_theme: custom_theme.clone(),
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(saved.preferences.custom_theme, custom_theme);
    assert_eq!(store.load().unwrap(), saved);
    let value = serde_json::to_value(&saved.preferences.custom_theme).unwrap();
    assert_eq!(value["base"], "paper");
    assert_eq!(value["candidate_skin"], "sakura.v2");
    assert_eq!(value["candidate_colors"]["border"], "#707070");
    assert_eq!(value["keyboard"]["background"], 0x123456);
    assert_eq!(
        serde_json::from_value::<CustomTheme>(value).unwrap(),
        custom_theme
    );

    let mut unknown = serde_json::to_value(&custom_theme).unwrap();
    unknown["candidate_colors"]["selected_text"] = "#000000".into();
    let (read, ignored) = deserialize_keeping_unknown::<CustomTheme>(&unknown, &[]).unwrap();
    assert_eq!(read, custom_theme);
    assert_eq!(ignored.len(), 1);
    for base in ["fluent", "wechat", "dark"] {
        let mut unknown = serde_json::to_value(&custom_theme).unwrap();
        unknown["base"] = base.into();
        assert!(
            serde_json::from_value::<CustomTheme>(unknown).is_err(),
            "{base} is not a theme id"
        );
    }

    // `native` 只在部分宿主上提供，不能当自定义主题的底。
    for base in [
        crate::theme::GlobalTheme::Custom,
        crate::theme::GlobalTheme::Native,
    ] {
        let mut preferences = saved.preferences.clone();
        preferences.custom_theme.base = base;
        assert!(matches!(
            store.save(saved.revision, preferences),
            Err(PreferencesError::InvalidCustomThemeBase)
        ));
    }
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn custom_theme_keyboard_matches_apple_fields_and_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("custom_theme");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.custom_theme.keyboard,
        None
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);

    let design = TouchKeyboardSkinDesign {
        background: 0x151022,
        key_background: 0x291E40,
        key_foreground: 0xFFFFFF,
        accent: 0xD4BBFF,
        action_background: 0x69469B,
        corner_radius: 12.0,
        border_width: 1.5,
        shadow: 0.25,
        pattern: 3,
        monospaced: true,
        key_shape: Some(TouchSkinKeyShape::Pebble),
        key_material: Some(TouchSkinKeyMaterial::Glass),
        key_opacity: Some(0.45),
        gradient_end: Some(0x30224A),
        gradient_horizontal: Some(true),
        pattern_opacity: Some(0.2),
        custom_border_color: Some(0xA987E8),
        photo: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".into()),
        photo_shade: Some(0.8),
        photo_position: Some(1.0),
    };
    let preferences = Preferences {
        global_theme: crate::theme::GlobalTheme::Custom,
        custom_theme: CustomTheme {
            keyboard: Some(design.clone()),
            ..CustomTheme::default()
        },
        ..Preferences::default()
    };
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(saved.preferences.custom_theme.keyboard, Some(design));
    let value = serde_json::to_value(&saved.preferences.custom_theme.keyboard).unwrap();
    assert_eq!(value["keyBackground"], 0x291E40);
    assert_eq!(value["keyShape"], "pebble");
    assert!(value.get("key_background").is_none());

    for invalid in [
        TouchKeyboardSkinDesign {
            background: 0x1000000,
            ..TouchKeyboardSkinDesign::default()
        },
        TouchKeyboardSkinDesign {
            corner_radius: 21.0,
            ..TouchKeyboardSkinDesign::default()
        },
        TouchKeyboardSkinDesign {
            key_opacity: Some(0.24),
            ..TouchKeyboardSkinDesign::default()
        },
        TouchKeyboardSkinDesign {
            photo: Some("not-base64".into()),
            ..TouchKeyboardSkinDesign::default()
        },
    ] {
        let mut preferences = saved.preferences.clone();
        preferences.custom_theme.keyboard = Some(invalid);
        assert!(matches!(
            store.save(saved.revision, preferences),
            Err(PreferencesError::InvalidTouchKeyboardSkinDesign)
        ));
        assert_eq!(store.load().unwrap(), saved);
    }
}

#[test]
fn mixed_input_legacy_roundtrip_and_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("mixed_input");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.mixed_input,
        MixedInputPreferences::default()
    );
    assert_eq!(
        store.load().unwrap().preferences.mixed_input.minimum_prefix,
        5
    );
    assert_eq!(
        store.load().unwrap().preferences.mixed_input.emoji,
        cfg!(any(windows, target_os = "macos"))
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    // 只写前四项的旧对象（`japanese` 之前的文档、各宿主自己拼的对象）照样能读，日文混输按关处理、符号混输按出厂读作开。
    let mut four_fields = legacy.clone();
    four_fields["preferences"]["mixed_input"] = serde_json::json!({
        "english": false,
        "minimum_prefix": 3,
        "emoji": true,
        "kaomoji": true,
    });
    fs::write(store.path(), serde_json::to_vec(&four_fields).unwrap()).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.mixed_input,
        MixedInputPreferences {
            enabled: true,
            english: false,
            minimum_prefix: 3,
            first_page_count: 1,
            emoji: true,
            kaomoji: true,
            japanese: false,
            symbols: true,
            quick_phrase: true,
            quick_phrase_frequency: true,
        }
    );
    fs::write(store.path(), &bytes).unwrap();
    for mask in 0..32 {
        let preferences = Preferences {
            mixed_input: MixedInputPreferences {
                enabled: true,
                english: mask & 1 != 0,
                emoji: mask & 2 != 0,
                kaomoji: mask & 4 != 0,
                japanese: mask & 8 != 0,
                symbols: mask & 16 == 0,
                quick_phrase: true,
                quick_phrase_frequency: true,
                minimum_prefix: mask % 8 + 1,
                first_page_count: 1,
            },
            ..Preferences::default()
        };
        let saved = store.save(u64::from(mask), preferences).unwrap();
        assert_eq!(store.load().unwrap(), saved);
        // 关着时不写这个键，文档和加这一项之前逐字节一样；开着时才写出来。
        let written: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(
            written["preferences"]["mixed_input"].get("japanese"),
            (mask & 8 != 0).then_some(&serde_json::Value::Bool(true))
        );
        // 符号混输开着时不写这个键，关掉才写出；旧版本把缺键读作开。
        assert_eq!(
            written["preferences"]["mixed_input"].get("symbols"),
            (mask & 16 != 0).then_some(&serde_json::Value::Bool(false))
        );
    }
    let saved = store.load().unwrap();
    for value in [0, 9, 255] {
        let mut invalid = saved.preferences.clone();
        invalid.mixed_input.minimum_prefix = value;
        assert!(matches!(
            store.save(saved.revision, invalid),
            Err(PreferencesError::InvalidMixedInput)
        ));
        assert_eq!(store.load().unwrap(), saved);
    }
}

/// 符号混输出厂为开（`symbol_mixed_input` 出厂关，差异已记录）：缺键读作开、开着不写键位，写明的 `false` 原样生效并原样写回；四个必填成员缺一个照旧整份读不进来。
#[test]
fn mixed_input_symbols_default_on_and_are_written_only_when_off() {
    assert!(MixedInputPreferences::default().symbols);
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let mixed = |document: &serde_json::Value| document["preferences"]["mixed_input"].clone();
    assert!(mixed(&document).get("symbols").is_none());

    document["preferences"]["mixed_input"] = serde_json::json!({
        "english": true,
        "minimum_prefix": 5,
        "emoji": false,
        "kaomoji": false,
    });
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let loaded = store.load().unwrap();
    assert!(loaded.preferences.mixed_input.symbols);
    let saved = store.save(loaded.revision, loaded.preferences).unwrap();
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert!(mixed(&written).get("symbols").is_none());

    let mut off = saved.preferences.clone();
    off.mixed_input.symbols = false;
    let saved = store.save(saved.revision, off).unwrap();
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(mixed(&written)["symbols"], serde_json::Value::Bool(false));
    assert!(!store.load().unwrap().preferences.mixed_input.symbols);
    assert_eq!(store.load().unwrap(), saved);

    document["preferences"]["mixed_input"]["symbols"] = serde_json::Value::Bool(true);
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();
    let loaded = store.load().unwrap();
    assert!(loaded.preferences.mixed_input.symbols);

    let mut missing = document.clone();
    missing["preferences"]["mixed_input"]
        .as_object_mut()
        .unwrap()
        .remove("kaomoji");
    fs::write(store.path(), serde_json::to_vec(&missing).unwrap()).unwrap();
    assert!(store.load().is_err());
}

#[test]
fn frequency_legacy_defaults_modes_and_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("frequency");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.frequency,
        FrequencyPreferences::default()
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    for (revision, mode) in [
        FrequencyMode::Disabled,
        FrequencyMode::Pin,
        FrequencyMode::Halve,
        FrequencyMode::Linear,
        FrequencyMode::Promote,
    ]
    .into_iter()
    .enumerate()
    {
        let saved = store
            .save(
                revision as u64,
                Preferences {
                    frequency: FrequencyPreferences {
                        mode,
                        trigger_count: 10,
                        linear_step: 10,
                    },
                    ..Preferences::default()
                },
            )
            .unwrap();
        assert_eq!(store.load().unwrap(), saved);
    }
    let saved = store.load().unwrap();
    for value in [0, 11, 255] {
        for trigger in [true, false] {
            let mut preferences = Preferences::default();
            if trigger {
                preferences.frequency.trigger_count = value;
            } else {
                preferences.frequency.linear_step = value;
            }
            assert!(matches!(
                store.save(saved.revision, preferences),
                Err(PreferencesError::InvalidFrequency)
            ));
            assert_eq!(store.load().unwrap(), saved);
        }
    }
}

#[test]
fn word_character_legacy_roundtrip_and_conflict_protection() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("word_character");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let defaults = store.load().unwrap().preferences.word_character;
    assert_eq!(defaults, WordCharacterPreferences::default());
    assert!(defaults.enabled);
    assert_eq!(defaults.keys, WordCharacterKeys::Brackets);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    for (revision, keys) in [WordCharacterKeys::Brackets, WordCharacterKeys::MinusEqual]
        .into_iter()
        .enumerate()
    {
        let mut preferences = Preferences {
            word_character: WordCharacterPreferences {
                enabled: true,
                keys,
            },
            ..Preferences::default()
        };
        preferences.navigation.brackets = false;
        preferences.navigation.minus_equal = false;
        let saved = store.save(revision as u64, preferences.clone()).unwrap();
        assert_eq!(store.load().unwrap(), saved);
        match keys {
            WordCharacterKeys::Brackets => preferences.navigation.brackets = true,
            WordCharacterKeys::MinusEqual => preferences.navigation.minus_equal = true,
        }
        assert!(matches!(
            store.save(saved.revision, preferences),
            Err(PreferencesError::ConflictingKeyBindings)
        ));
        assert_eq!(store.load().unwrap(), saved);
    }
}

#[test]
fn second_third_candidate_defaults_off_and_stays_out_of_default_documents() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let defaults = Preferences::default();
    assert!(!defaults.second_third_candidate.enabled);
    assert_eq!(
        defaults.second_third_candidate.keys,
        SecondThirdCandidateKeys::SemicolonQuote
    );
    // 关闭时不写进文档：不认识这个键的旧版本（`deny_unknown_fields`）照样能读。
    let document = serde_json::to_value(&defaults).unwrap();
    assert!(document.get("second_third_candidate").is_none());
    let saved = store.save(0, defaults.clone()).unwrap();
    assert_eq!(
        saved.preferences.second_third_candidate,
        defaults.second_third_candidate
    );

    let enabled = Preferences {
        second_third_candidate: SecondThirdCandidatePreferences {
            enabled: true,
            keys: SecondThirdCandidateKeys::SemicolonQuote,
        },
        ..Preferences::default()
    };
    assert_eq!(
        serde_json::to_value(&enabled).unwrap()["second_third_candidate"],
        serde_json::json!({"enabled": true, "keys": "semicolon_quote"})
    );
    let saved = store.save(saved.revision, enabled.clone()).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert!(saved.preferences.second_third_candidate.enabled);

    // 只写了 `enabled` 的对象按默认键位读。
    let partial: SecondThirdCandidatePreferences =
        serde_json::from_value(serde_json::json!({"enabled": true})).unwrap();
    assert!(partial.enabled);
    assert_eq!(partial.keys, SecondThirdCandidateKeys::SemicolonQuote);

    // 未知的键位名拒绝，不会被悄悄读成某个默认键位；多余字段是更新版本的设置，记为未知键。
    assert!(serde_json::from_value::<SecondThirdCandidatePreferences>(
        serde_json::json!({"enabled": true, "keys": "shift"})
    )
    .is_err());
    let (extra, ignored) = deserialize_keeping_unknown::<SecondThirdCandidatePreferences>(
        &serde_json::json!({"enabled": true, "keys": "semicolon_quote", "extra": 1}),
        &[],
    )
    .unwrap();
    assert!(extra.enabled);
    assert_eq!(ignored.len(), 1);
}

#[test]
fn wubi_candidate_selection_defaults_roundtrip_and_rejects_invalid_keys() {
    let root = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(root.path());
    let defaults = Preferences::default();
    assert!(serde_json::to_value(&defaults)
        .unwrap()
        .get("wubi_candidate_selection")
        .is_none());
    let mut settings: WubiCandidateSelectionPreferences =
        serde_json::from_value(serde_json::json!({"enabled": true})).unwrap();
    assert_eq!(settings.slot(b';'), Some(1));
    assert_eq!(settings.slot(b'\''), Some(2));
    settings.second_key = "/".into();
    settings.third_key = "?".into();
    let mut preferences = Preferences {
        wubi_candidate_selection: settings,
        ..defaults
    };
    let saved = store.save(0, preferences.clone()).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert_eq!(
        saved.preferences.wubi_candidate_selection.slot(b'/'),
        Some(1)
    );
    for invalid in ["", "a", "2", " ", "\n", "；", ";'", "?"] {
        preferences.wubi_candidate_selection.second_key = invalid.into();
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidWubiCandidateSelection)
        ));
        assert!(store.save(saved.revision, preferences.clone()).is_err());
        assert_eq!(store.load().unwrap(), saved);
    }
}

#[test]
fn navigation_accepts_windows_candidate_arrow_alias() {
    let value = serde_json::json!({"minus_equal": true, "comma_period": true, "brackets": false, "tab": true, "page_up_down": true, "mouse_wheel": true, "candidate_arrow_navigation": false});
    let parsed: NavigationPreferences = serde_json::from_value(value).unwrap();
    assert!(!parsed.arrows);
    assert!(parsed.mouse_wheel);
    // 旧文档没有这个键时按出厂值读作开（Windows `candidate_horizontal_arrow_navigation` 出厂关，差异已记录）。
    assert!(parsed.left_right);
}

#[test]
fn navigation_left_right_explicit_false_survives_store_reload() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut preferences = Preferences::default();
    preferences.navigation.left_right = false;
    let saved = store.save(0, preferences).unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(document["preferences"]["navigation"]["left_right"], false);
    assert_eq!(PreferencesStore::new(dir.path()).load().unwrap(), saved);
}

#[test]
fn navigation_defaults_and_independent_flags_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("navigation");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.navigation,
        NavigationPreferences::default()
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    let preferences = Preferences {
        word_character: WordCharacterPreferences {
            enabled: false,
            keys: WordCharacterKeys::Brackets,
        },
        navigation: NavigationPreferences {
            minus_equal: false,
            comma_period: false,
            brackets: true,
            tab: false,
            page_up_down: false,
            mouse_wheel: false,
            arrows: false,
            left_right: true,
        },
        ..Preferences::default()
    };
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    let mut invalid = serde_json::to_value(saved).unwrap();
    invalid["preferences"]["navigation"]["tab"] = "invalid".into();
    let bytes = serde_json::to_vec(&invalid).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(store.save(1, Preferences::default()).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
}

#[test]
fn number_row_selection_defaults_on_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("number_row_selection");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(store.load().unwrap().preferences.number_row_selection);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);

    let saved = store
        .save(
            0,
            Preferences {
                number_row_selection: false,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert!(!saved.preferences.number_row_selection);
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn remembered_chinese_scheme_roundtrips_without_changing_legacy_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let legacy = serde_json::to_vec(&PreferencesSnapshot::default()).unwrap();
    fs::write(store.path(), &legacy).unwrap();
    assert_eq!(store.load().unwrap().preferences.last_chinese_scheme, None);
    assert_eq!(fs::read(store.path()).unwrap(), legacy);
    for (revision, scheme) in [
        ChineseScheme::Quanpin,
        ChineseScheme::Shuangpin,
        ChineseScheme::Wubi,
    ]
    .into_iter()
    .enumerate()
    {
        let saved = store
            .save(
                revision as u64,
                Preferences {
                    scheme: InputScheme::Japanese,
                    last_chinese_scheme: Some(scheme),
                    ..Preferences::default()
                },
            )
            .unwrap();
        assert_eq!(store.load().unwrap(), saved);
    }
    // Korean keeps the scheme to return to the same way, and serialises as `korean`.
    let saved = store
        .save(
            3,
            Preferences {
                scheme: InputScheme::Korean,
                last_chinese_scheme: Some(ChineseScheme::Wubi),
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert_eq!(
        serde_json::to_value(&saved.preferences).unwrap()["scheme"],
        "korean"
    );
    // Korean is not a scheme to return to.
    assert!(serde_json::from_value::<ChineseScheme>("korean".into()).is_err());
    let mut invalid = serde_json::to_value(store.load().unwrap()).unwrap();
    invalid["preferences"]["last_chinese_scheme"] = "japanese".into();
    let bytes = serde_json::to_vec(&invalid).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(store.save(3, Preferences::default()).is_err());
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
}

#[test]
fn cantonese_zhuyin_and_vietnamese_schemes_round_trip_under_their_wire_names() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    for (revision, (scheme, name)) in [
        (InputScheme::Cantonese, "cantonese"),
        (InputScheme::Zhuyin, "zhuyin"),
        (InputScheme::Vietnamese, "vietnamese"),
        (InputScheme::Stroke, "stroke"),
    ]
    .into_iter()
    .enumerate()
    {
        let saved = store
            .save(
                revision as u64,
                Preferences {
                    scheme,
                    ..Preferences::default()
                },
            )
            .unwrap();
        assert_eq!(store.load().unwrap(), saved);
        assert_eq!(
            serde_json::to_value(&saved.preferences).unwrap()["scheme"],
            name
        );
    }
    // Cantonese, Zhuyin and Stroke are Chinese schemes to return to; Vietnamese is not.
    for (revision, (scheme, name)) in [
        (ChineseScheme::Cantonese, "cantonese"),
        (ChineseScheme::Zhuyin, "zhuyin"),
        (ChineseScheme::Stroke, "stroke"),
    ]
    .into_iter()
    .enumerate()
    {
        let saved = store
            .save(
                revision as u64 + 4,
                Preferences {
                    scheme: InputScheme::Vietnamese,
                    last_chinese_scheme: Some(scheme),
                    ..Preferences::default()
                },
            )
            .unwrap();
        assert_eq!(store.load().unwrap(), saved);
        assert_eq!(
            serde_json::to_value(&saved.preferences).unwrap()["last_chinese_scheme"],
            name
        );
    }
    assert!(serde_json::from_value::<ChineseScheme>("vietnamese".into()).is_err());
    assert_eq!(
        InputScheme::from(ChineseScheme::Cantonese),
        InputScheme::Cantonese
    );
    assert_eq!(
        InputScheme::from(ChineseScheme::Zhuyin),
        InputScheme::Zhuyin
    );
    assert_eq!(
        InputScheme::from(ChineseScheme::Stroke),
        InputScheme::Stroke
    );
}

// 藏文以 `tibetan` 存盘并原样读回；它不是中文方案，切到藏文时保留记住的中文方案，`last_chinese_scheme` 也不接受它。
#[test]
fn tibetan_scheme_round_trips_and_keeps_the_last_chinese_scheme() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store
        .save(
            0,
            Preferences {
                scheme: InputScheme::Tibetan,
                last_chinese_scheme: Some(ChineseScheme::Wubi),
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(store.load().unwrap(), saved);
    let document = serde_json::to_value(&saved.preferences).unwrap();
    assert_eq!(document["scheme"], "tibetan");
    assert_eq!(document["last_chinese_scheme"], "wubi");
    assert_eq!(
        serde_json::from_value::<InputScheme>("tibetan".into()).unwrap(),
        InputScheme::Tibetan
    );
    assert!(serde_json::from_value::<ChineseScheme>("tibetan".into()).is_err());
}

#[test]
fn vietnamese_preferences_default_when_absent_and_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut defaults = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    assert_eq!(
        defaults["preferences"]["vietnamese"],
        serde_json::json!({ "input_method": "telex", "tone_style": "modern" })
    );
    defaults["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("vietnamese");
    let bytes = serde_json::to_vec(&defaults).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(
        loaded.preferences.vietnamese,
        VietnamesePreferences {
            input_method: VietnameseInputMethod::Telex,
            tone_style: VietnameseToneStyle::Modern,
        }
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    let saved = store.save(0, loaded.preferences).unwrap();
    let preferences = Preferences {
        vietnamese: VietnamesePreferences {
            input_method: VietnameseInputMethod::Vni,
            tone_style: VietnameseToneStyle::Classic,
        },
        ..Preferences::default()
    };
    let saved = store.save(saved.revision, preferences).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert_eq!(
        serde_json::to_value(&saved.preferences).unwrap()["vietnamese"],
        serde_json::json!({ "input_method": "vni", "tone_style": "classic" })
    );
    // One member set leaves the other at its default.
    let partial: VietnamesePreferences =
        serde_json::from_value(serde_json::json!({ "tone_style": "classic" })).unwrap();
    assert_eq!(partial.input_method, VietnameseInputMethod::Telex);
    assert_eq!(partial.tone_style, VietnameseToneStyle::Classic);
}

#[test]
fn korean_layout_loads_legacy_files_and_roundtrips_romaja() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("korean");
    let bytes = serde_json::to_vec(&document).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.preferences.korean.layout, KoreanLayout::Dubeolsik);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    let mut preferences = loaded.preferences;
    preferences.korean.layout = KoreanLayout::Romaja;
    let saved = store.save(loaded.revision, preferences).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert_eq!(
        serde_json::to_value(saved.preferences).unwrap()["korean"]["layout"],
        "romaja"
    );
}

#[test]
fn korean_unknown_disk_layout_falls_back_without_resetting_other_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["preferences"]["korean"] =
        serde_json::json!({ "layout": "future", "future_option": true });
    document["preferences"]["candidate_page_size"] = 4.into();
    let bytes = serde_json::to_vec(&document).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.preferences.korean.layout, KoreanLayout::Dubeolsik);
    assert_eq!(loaded.preferences.candidate_page_size, 4);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    store.save(loaded.revision, loaded.preferences).unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(saved["preferences"]["korean"]["future_option"], true);
    assert!(serde_json::from_value::<Preferences>(document["preferences"].clone()).is_err());
}

#[test]
fn vietnamese_preferences_reject_unknown_members_and_values() {
    // 多出的成员是更新版本的设置，记为未知键；未知的取值仍然拒绝。
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["vietnamese"] = serde_json::json!({ "input_method": "telex", "layout": "us" });
    assert_eq!(unknown_paths(&document), [["vietnamese", "layout"]]);
    for invalid in [
        serde_json::json!({ "input_method": "viqr" }),
        serde_json::json!({ "tone_style": "old" }),
    ] {
        assert!(serde_json::from_value::<VietnamesePreferences>(invalid.clone()).is_err());
        let mut document = serde_json::to_value(Preferences::default()).unwrap();
        document["vietnamese"] = invalid;
        assert!(serde_json::from_value::<Preferences>(document).is_err());
    }
}

#[test]
fn touch_keyboard_layout_defaults_and_roundtrips_without_rewriting_legacy_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("touch_keyboard_layout");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.touch_keyboard_layout,
        TouchKeyboardLayout::TwentySixKey
    );
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    store
        .save(
            0,
            Preferences {
                touch_keyboard_layout: TouchKeyboardLayout::NineKey,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.touch_keyboard_layout,
        TouchKeyboardLayout::NineKey
    );
    store
        .save(
            1,
            Preferences {
                touch_keyboard_layout: TouchKeyboardLayout::Handwriting,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.touch_keyboard_layout,
        TouchKeyboardLayout::Handwriting
    );
    let saved = store
        .save(
            2,
            Preferences {
                touch_keyboard_layout: TouchKeyboardLayout::FourteenKey,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&saved).unwrap()["preferences"]["touch_keyboard_layout"],
        "fourteen_key"
    );
    assert_eq!(
        store.load().unwrap().preferences.touch_keyboard_layout,
        TouchKeyboardLayout::FourteenKey
    );
    let mut invalid = serde_json::to_value(saved).unwrap();
    invalid["preferences"]["touch_keyboard_layout"] = "future_layout".into();
    let bytes = serde_json::to_vec(&invalid).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(store.load().is_err());
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
}

#[test]
fn touch_keyboard_scheme_visibility_matches_apple_order_and_fallback_contract() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    // 以前的版本在列表等于默认值时不写 `touch_keyboard_schemes`。
    let mut document = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    document["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("touch_keyboard_schemes");
    let legacy = serde_json::to_vec(&document).unwrap();
    fs::write(store.path(), &legacy).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(
        loaded.preferences.touch_keyboard_schemes.enabled,
        TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED
            .into_iter()
            .collect()
    );
    assert_eq!(loaded.preferences.touch_keyboard_schemes.selected, None);
    assert_eq!(fs::read(store.path()).unwrap(), legacy);

    let visible = [
        TouchKeyboardScheme::NineKey,
        TouchKeyboardScheme::Handwriting,
    ]
    .into_iter()
    .collect();
    let saved = store
        .save(
            0,
            Preferences {
                touch_keyboard_schemes: TouchKeyboardSchemePreferences {
                    enabled: visible,
                    selected: Some(TouchKeyboardScheme::Handwriting),
                },
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(store.load().unwrap(), saved);

    for value in [
        serde_json::json!({"enabled": [], "selected": null}),
        serde_json::json!({"enabled": ["nine_key"], "selected": "handwriting"}),
    ] {
        let mut invalid = serde_json::to_value(&saved).unwrap();
        invalid["preferences"]["touch_keyboard_schemes"] = value;
        let bytes = serde_json::to_vec(&invalid).unwrap();
        fs::write(store.path(), &bytes).unwrap();
        assert!(matches!(
            store.save(1, Preferences::default()),
            Err(PreferencesError::InvalidTouchKeyboardSchemes)
        ));
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
    }

    let mut unknown = serde_json::to_value(&saved).unwrap();
    unknown["preferences"]["touch_keyboard_schemes"]["enabled"] =
        serde_json::json!(["nine_key", "future_scheme"]);
    let bytes = serde_json::to_vec(&unknown).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(store.load().is_err());
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
}

#[test]
fn retired_thoughtful_reply_touch_scheme_is_migrated_on_read() {
    // 高情商回复已经不是方案，新文档里不会再写出它。
    assert!(TouchKeyboardScheme::ALL
        .into_iter()
        .all(|scheme| serde_json::to_value(scheme).unwrap() != "thoughtful_reply"));
    assert!(serde_json::from_value::<TouchKeyboardScheme>("thoughtful_reply".into()).is_err());

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store.save(0, Preferences::default()).unwrap();
    for (stored, enabled, selected) in [
        // 选中的是高情商回复：从列表里去掉，选中退回按顺序第一个启用的方案，与选中方案被关闭时的处理相同。
        (
            serde_json::json!({"enabled": ["wubi", "thoughtful_reply", "handwriting"], "selected": "thoughtful_reply"}),
            vec![TouchKeyboardScheme::Wubi, TouchKeyboardScheme::Handwriting],
            Some(TouchKeyboardScheme::Wubi),
        ),
        // 只在列表里：去掉它，选中不变。
        (
            serde_json::json!({"enabled": ["nine_key", "thoughtful_reply"], "selected": "nine_key"}),
            vec![TouchKeyboardScheme::NineKey],
            Some(TouchKeyboardScheme::NineKey),
        ),
        // 列表里只有它：按空列表的约定回到全拼 26 键。
        (
            serde_json::json!({"enabled": ["thoughtful_reply"], "selected": "thoughtful_reply"}),
            vec![TouchKeyboardScheme::Quanpin],
            Some(TouchKeyboardScheme::Quanpin),
        ),
        // 没有存列表时用以前的版本的缺省列表（这样的文档只可能是它们写的），选中退回全拼 26 键。
        (
            serde_json::json!({"selected": "thoughtful_reply"}),
            TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED.to_vec(),
            Some(TouchKeyboardScheme::Quanpin),
        ),
    ] {
        let mut legacy = serde_json::to_value(&saved).unwrap();
        legacy["preferences"]["touch_keyboard_schemes"] = stored;
        let bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(store.path(), &bytes).unwrap();
        let loaded = store.load().unwrap();
        let schemes = &loaded.preferences.touch_keyboard_schemes;
        assert_eq!(schemes.enabled, enabled.into_iter().collect());
        assert_eq!(schemes.selected, selected);
        loaded.preferences.validate().unwrap();
        // 读取不改写文件；迁移后的内容与读到的相同，原样保存也不写盘，迁移结果随下一次真正的改动落盘。
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
        let unchanged = store
            .save(loaded.revision, loaded.preferences.clone())
            .unwrap();
        assert_eq!(unchanged, loaded);
        assert_eq!(fs::read(store.path()).unwrap(), bytes);
        let mut changed = loaded.preferences;
        changed.candidate_page_size = if changed.candidate_page_size == 5 {
            6
        } else {
            5
        };
        let resaved = store.save(loaded.revision, changed).unwrap();
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert!(!document["preferences"]["touch_keyboard_schemes"]
            .to_string()
            .contains("thoughtful_reply"));
        assert_eq!(store.load().unwrap(), resaved);
    }
}

#[test]
fn cantonese_zhuyin_and_vietnamese_touch_schemes_are_appended_and_opt_in() {
    // 默认只启用中文方案：日文（9 键和 26 键）和韩文与后面追加的几个方案一样由用户自己打开。
    assert_eq!(
        TouchKeyboardScheme::DEFAULT_ENABLED,
        [
            TouchKeyboardScheme::Quanpin,
            TouchKeyboardScheme::NineKey,
            TouchKeyboardScheme::Xiaohe,
            TouchKeyboardScheme::Ziranma,
            TouchKeyboardScheme::Microsoft,
            TouchKeyboardScheme::Shoudao,
            TouchKeyboardScheme::Wubi,
            TouchKeyboardScheme::Handwriting,
        ]
    );
    for scheme in [
        TouchKeyboardScheme::JapaneseNineKey,
        TouchKeyboardScheme::Japanese,
        TouchKeyboardScheme::Korean,
    ] {
        assert!(!TouchKeyboardSchemePreferences::default()
            .enabled
            .contains(&scheme));
    }
    assert_eq!(
        TouchKeyboardScheme::ALL[11..],
        [
            TouchKeyboardScheme::Cantonese,
            TouchKeyboardScheme::Zhuyin,
            TouchKeyboardScheme::Vietnamese,
            TouchKeyboardScheme::Tibetan,
            TouchKeyboardScheme::Stroke,
            TouchKeyboardScheme::ZhuyinNineKey,
            TouchKeyboardScheme::FourteenKey,
            TouchKeyboardScheme::NineKeyShuangpin,
            TouchKeyboardScheme::EighteenKey,
        ]
    );
    assert_eq!(TouchKeyboardScheme::ALL.len(), 20);
    // 全拼 14 键和九键双拼也不在以前的默认列表里，没存列表的旧文档不会凭空多出它们。
    for scheme in [
        TouchKeyboardScheme::FourteenKey,
        TouchKeyboardScheme::NineKeyShuangpin,
    ] {
        assert!(!TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED.contains(&scheme));
    }
    for (scheme, id) in [
        (TouchKeyboardScheme::Cantonese, "cantonese"),
        (TouchKeyboardScheme::Zhuyin, "zhuyin"),
        (TouchKeyboardScheme::Vietnamese, "vietnamese"),
        (TouchKeyboardScheme::Tibetan, "tibetan"),
        (TouchKeyboardScheme::Stroke, "stroke"),
        (TouchKeyboardScheme::ZhuyinNineKey, "zhuyin_nine_key"),
        (TouchKeyboardScheme::FourteenKey, "fourteen_key"),
        (TouchKeyboardScheme::NineKeyShuangpin, "nine_key_shuangpin"),
    ] {
        assert_eq!(serde_json::to_value(scheme).unwrap(), id);
        assert_eq!(
            serde_json::from_value::<TouchKeyboardScheme>(serde_json::json!(id)).unwrap(),
            scheme
        );
        assert!(!TouchKeyboardSchemePreferences::default()
            .enabled
            .contains(&scheme));
    }
    // The picker grid follows ALL, and a set orders the same way.
    let all: Vec<_> = TouchKeyboardScheme::ALL
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(all, TouchKeyboardScheme::ALL);

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store
        .save(
            0,
            Preferences {
                scheme: InputScheme::Zhuyin,
                touch_keyboard_schemes: TouchKeyboardSchemePreferences {
                    enabled: [TouchKeyboardScheme::Quanpin, TouchKeyboardScheme::Zhuyin]
                        .into_iter()
                        .collect(),
                    selected: Some(TouchKeyboardScheme::Zhuyin),
                },
                ..Preferences::default()
            },
        )
        .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        document["preferences"]["touch_keyboard_schemes"],
        serde_json::json!({"enabled": ["quanpin", "zhuyin"], "selected": "zhuyin"})
    );
    assert_eq!(store.load().unwrap(), saved);
}

/// 以前的版本写的文档在列表等于默认值时没有 `touch_keyboard_schemes`：升级后仍按以前的默认列表读，日文和韩文键盘不会消失；下一次保存把这份列表显式写进文件。
#[test]
fn upgrade_without_a_stored_touch_scheme_list_keeps_the_previous_default() {
    let legacy: std::collections::BTreeSet<_> = TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED
        .into_iter()
        .collect();
    for scheme in [
        TouchKeyboardScheme::JapaneseNineKey,
        TouchKeyboardScheme::Japanese,
        TouchKeyboardScheme::Korean,
    ] {
        assert!(legacy.contains(&scheme));
        assert!(!TouchKeyboardScheme::DEFAULT_ENABLED.contains(&scheme));
    }
    assert!(TouchKeyboardScheme::DEFAULT_ENABLED
        .iter()
        .all(|scheme| legacy.contains(scheme)));

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    // 用户存过别的设置、没动过键盘列表的旧文档：整个对象缺席，或者只有 `selected`。
    for stored in [None, Some(serde_json::json!({"selected": "korean"}))] {
        let mut document = serde_json::to_value(PreferencesSnapshot {
            revision: 3,
            preferences: Preferences {
                candidate_page_size: 7,
                ..Preferences::default()
            },
            ..PreferencesSnapshot::default()
        })
        .unwrap();
        let preferences = document["preferences"].as_object_mut().unwrap();
        match &stored {
            None => {
                preferences.remove("touch_keyboard_schemes");
            }
            Some(value) => {
                preferences.insert("touch_keyboard_schemes".into(), value.clone());
            }
        }
        fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.preferences.touch_keyboard_schemes.enabled, legacy);
        assert_eq!(
            loaded.preferences.touch_keyboard_schemes.selected,
            stored.as_ref().map(|_| TouchKeyboardScheme::Korean)
        );
        // 状态目录已经有文件，准备宿主时不改写它。
        assert_eq!(store.write_first_document().unwrap(), loaded);

        let mut changed = loaded.preferences.clone();
        changed.candidate_page_size = 6;
        store.save(loaded.revision, changed).unwrap();
        let written: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        let enabled: Vec<TouchKeyboardScheme> = serde_json::from_value(
            written["preferences"]["touch_keyboard_schemes"]["enabled"].clone(),
        )
        .unwrap();
        assert_eq!(enabled, TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED);
        assert_eq!(
            store
                .load()
                .unwrap()
                .preferences
                .touch_keyboard_schemes
                .enabled,
            legacy
        );
    }

    // 修复一份损坏、又没有列表的旧文档：其余设置照常保留，列表仍按以前的默认值。
    let mut damaged = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let preferences = damaged["preferences"].as_object_mut().unwrap();
    preferences.remove("touch_keyboard_schemes");
    preferences.insert("candidate_page_size".into(), 7.into());
    preferences.insert("learning".into(), "yes".into());
    fs::write(store.path(), serde_json::to_vec(&damaged).unwrap()).unwrap();
    let RecoveryOutcome::Recovered { snapshot, .. } = store.recover().unwrap() else {
        panic!("a document with a wrong-typed field is recovered");
    };
    assert_eq!(snapshot.preferences.candidate_page_size, 7);
    assert_eq!(snapshot.preferences.touch_keyboard_schemes.enabled, legacy);
}

/// 以前的版本准备过状态目录（建了 `user/dictionaries`）、用户却从没存过偏好：没有偏好文件也按以前的默认列表，准备宿主时把它写成第一份文件。新装还没有这个目录，得到只有中文的默认列表，写下文件后再建目录也不会被当成升级。
#[test]
fn first_document_tells_a_fresh_install_from_an_upgrade() {
    let legacy: std::collections::BTreeSet<_> = TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED
        .into_iter()
        .collect();
    let chinese: std::collections::BTreeSet<_> =
        TouchKeyboardScheme::DEFAULT_ENABLED.into_iter().collect();
    let written_list = |store: &PreferencesStore| -> Vec<TouchKeyboardScheme> {
        let document: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        serde_json::from_value(document["preferences"]["touch_keyboard_schemes"]["enabled"].clone())
            .unwrap()
    };

    // 新装。
    let fresh = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(fresh.path());
    let missing = store.load().unwrap();
    assert_eq!(missing.revision, 0);
    assert_eq!(missing.preferences, Preferences::default());
    assert_eq!(missing.preferences.touch_keyboard_schemes.enabled, chinese);
    let first = store.write_first_document().unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.preferences, Preferences::default());
    // 等于默认值也显式写出列表，否则读回来会被当成以前的版本的文档。
    assert_eq!(written_list(&store), TouchKeyboardScheme::DEFAULT_ENABLED);
    fs::create_dir_all(fresh.path().join("user").join("dictionaries")).unwrap();
    assert_eq!(store.load().unwrap(), first);
    assert_eq!(store.write_first_document().unwrap(), first);

    // 从以前的版本升级，从没存过偏好。
    let upgraded = tempfile::tempdir().unwrap();
    fs::create_dir_all(upgraded.path().join("user").join("dictionaries")).unwrap();
    let store = PreferencesStore::new(upgraded.path());
    let missing = store.load().unwrap();
    assert_eq!(missing.revision, 0);
    assert_eq!(missing.preferences.touch_keyboard_schemes.enabled, legacy);
    assert_eq!(missing.preferences.touch_keyboard_schemes.selected, None);
    // 只有触屏方案不同，其余都是默认值。
    assert_eq!(
        Preferences {
            touch_keyboard_schemes: TouchKeyboardSchemePreferences::default(),
            ..missing.preferences.clone()
        },
        Preferences::default()
    );
    let first = store.write_first_document().unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(first.preferences, missing.preferences);
    assert_eq!(
        written_list(&store),
        TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED
    );
    assert_eq!(store.load().unwrap(), first);

    // 指向别处的 `user/dictionaries` 不算准备过。
    #[cfg(unix)]
    {
        let elsewhere = tempfile::tempdir().unwrap();
        let linked = tempfile::tempdir().unwrap();
        fs::create_dir_all(linked.path().join("user")).unwrap();
        std::os::unix::fs::symlink(
            elsewhere.path(),
            linked.path().join("user").join("dictionaries"),
        )
        .unwrap();
        assert_eq!(
            PreferencesStore::new(linked.path())
                .load()
                .unwrap()
                .preferences
                .touch_keyboard_schemes
                .enabled,
            chinese
        );
    }

    // 不是 full 的版本升级时同样按以前的默认列表，只是收窄到本版本提供的入口：拼音版的日文和韩文入口本来就没有。
    let pinyin = crate::edition::Edition::by_id("pinyin").unwrap();
    let edition_dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(edition_dir.path().join("user").join("dictionaries")).unwrap();
    let store = PreferencesStore::for_edition(edition_dir.path(), pinyin);
    assert_eq!(
        store.load().unwrap().preferences.touch_keyboard_schemes,
        TouchKeyboardSchemePreferences::legacy_for_edition(pinyin)
    );
    assert_eq!(
        TouchKeyboardSchemePreferences::legacy_for_edition(crate::edition::Edition::full()).enabled,
        legacy
    );
}

#[test]
fn touch_spelling_position_defaults_to_the_editor_independently_of_the_candidate_window() {
    // 桌面候选窗默认画拼音行；触屏的拼写位置不能跟着它挪出输入框。
    assert_eq!(
        Preferences::default().candidate_preedit_style,
        CandidatePreeditStyle::Pinyin
    );
    assert_eq!(
        Preferences::default().touch_spelling_position,
        TouchSpellingPosition::Editor
    );

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("touch_spelling_position");
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.touch_spelling_position,
        TouchSpellingPosition::Editor
    );

    let saved = store
        .save(
            0,
            Preferences {
                touch_spelling_position: TouchSpellingPosition::CandidateBar,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(
        store.load().unwrap().preferences.touch_spelling_position,
        TouchSpellingPosition::CandidateBar
    );
    assert_eq!(
        saved.preferences.candidate_preedit_style,
        CandidatePreeditStyle::Pinyin
    );
    assert_eq!(
        serde_json::to_value(TouchSpellingPosition::CandidateBar).unwrap(),
        "candidate_bar"
    );
    assert_eq!(
        serde_json::to_value(TouchSpellingPosition::Editor).unwrap(),
        "editor"
    );
}

#[test]
fn touch_keyboard_spacing_uses_apple_defaults_bounds_and_legacy_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    for key in [
        "touch_key_spacing_tenths",
        "touch_row_spacing_tenths",
        "touch_keyboard_height_adjustment",
        "touch_voice_shortcut",
        "touch_number_keypad_order",
        "touch_twenty_six_key_number_layout",
        "touch_shuangpin_key_hints",
    ] {
        legacy["preferences"].as_object_mut().unwrap().remove(key);
    }
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.preferences.touch_key_spacing_tenths, 60);
    assert_eq!(loaded.preferences.touch_row_spacing_tenths, 70);
    assert_eq!(loaded.preferences.touch_keyboard_height_adjustment, 0);
    assert!(!loaded.preferences.touch_voice_shortcut);
    assert_eq!(
        loaded.preferences.touch_number_keypad_order,
        NumberKeypadOrder::Phone
    );
    assert_eq!(
        loaded.preferences.touch_twenty_six_key_number_layout,
        TwentySixKeyNumberLayout::Row
    );
    // 旧文档没有这个键：新默认关闭双拼键位提示。
    assert!(!loaded.preferences.touch_shuangpin_key_hints);
    assert!(!Preferences::default().touch_shuangpin_key_hints);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);

    let saved = store
        .save(
            0,
            Preferences {
                touch_key_spacing_tenths: 35,
                touch_row_spacing_tenths: 95,
                touch_keyboard_height_adjustment: 24,
                touch_voice_shortcut: true,
                touch_number_keypad_order: NumberKeypadOrder::Calculator,
                touch_twenty_six_key_number_layout: TwentySixKeyNumberLayout::NineKey,
                touch_shuangpin_key_hints: false,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert!(!saved.preferences.touch_shuangpin_key_hints);
    assert!(!store.load().unwrap().preferences.touch_shuangpin_key_hints);
    assert_eq!(
        saved.preferences.touch_number_keypad_order,
        NumberKeypadOrder::Calculator
    );
    assert_eq!(
        serde_json::to_value(NumberKeypadOrder::Calculator).unwrap(),
        "calculator"
    );
    assert_eq!(
        saved.preferences.touch_twenty_six_key_number_layout,
        TwentySixKeyNumberLayout::NineKey
    );
    assert_eq!(
        serde_json::to_value(TwentySixKeyNumberLayout::NineKey).unwrap(),
        "nine_key"
    );
    assert_eq!(saved.preferences.touch_key_spacing_tenths, 35);
    assert_eq!(saved.preferences.touch_row_spacing_tenths, 95);
    assert_eq!(saved.preferences.touch_keyboard_height_adjustment, 24);
    assert!(saved.preferences.touch_voice_shortcut);

    for (key, value) in [
        ("touch_key_spacing_tenths", 29),
        ("touch_key_spacing_tenths", 61),
        ("touch_row_spacing_tenths", 39),
        ("touch_row_spacing_tenths", 101),
    ] {
        let mut invalid = saved.preferences.clone();
        match key {
            "touch_key_spacing_tenths" => invalid.touch_key_spacing_tenths = value,
            _ => invalid.touch_row_spacing_tenths = value,
        }
        assert!(matches!(
            invalid.validate(),
            Err(PreferencesError::InvalidTouchKeyboardSpacing)
        ));
    }

    for value in [-13, 49] {
        let mut invalid = saved.preferences.clone();
        invalid.touch_keyboard_height_adjustment = value;
        assert!(matches!(
            invalid.validate(),
            Err(PreferencesError::InvalidTouchKeyboardSpacing)
        ));
    }
}

#[test]
fn helpcode_legacy_defaults_and_independent_schemes_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    for key in ["quanpin_helpcode", "shuangpin_helpcode"] {
        legacy["preferences"].as_object_mut().unwrap().remove(key);
    }
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert_eq!(store.load().unwrap(), PreferencesSnapshot::default());
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    for (revision, schema) in [
        HelpcodeSchema::Lantian,
        HelpcodeSchema::Ziranma,
        HelpcodeSchema::Shouyou2,
        HelpcodeSchema::Shouyouplus,
        HelpcodeSchema::Wubi86,
        HelpcodeSchema::Xiaohe,
    ]
    .into_iter()
    .enumerate()
    {
        let preferences = Preferences {
            quanpin_helpcode: HelpcodePreferences {
                trigger_key: String::new(),
                enabled: false,
                schema,
                show_in_candidate_window: true,
                modes: ShuangpinHelpcodeModes::default(),
            },
            ..Preferences::default()
        };
        let saved = store.save(revision as u64, preferences).unwrap();
        assert_eq!(store.load().unwrap(), saved);
        assert_eq!(
            saved.preferences.shuangpin_helpcode,
            default_shuangpin_helpcode()
        );
    }
    let unknown = fs::read_to_string(store.path())
        .unwrap()
        .replace("xiaohe", "unknown");
    fs::write(store.path(), &unknown).unwrap();
    assert!(store.save(6, Preferences::default()).is_err());
    assert_eq!(fs::read_to_string(store.path()).unwrap(), unknown);
}

#[test]
fn custom_helpcode_schema_roundtrips_and_rejects_unsafe_ids() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let preferences = Preferences {
        quanpin_helpcode: HelpcodePreferences {
            schema: HelpcodeSchema::Custom("custom/synthetic".into()),
            ..default_quanpin_helpcode()
        },
        ..Preferences::default()
    };
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(
        saved.preferences.quanpin_helpcode.schema.as_str(),
        "custom/synthetic"
    );

    let document = fs::read_to_string(store.path()).unwrap();
    for unsafe_id in ["custom/../escape", "custom/a\\b", "custom/"] {
        let invalid = document.replace("custom/synthetic", unsafe_id);
        fs::write(store.path(), invalid).unwrap();
        assert!(store.load().is_err(), "accepted unsafe schema {unsafe_id}");
    }
}

#[test]
fn shuangpin_helpcode_modes_follow_windows_v0_11_1_defaults() {
    let modes = ShuangpinHelpcodeModes::default();
    assert!(modes.direct);
    assert!(!modes.trigger_backtick && modes.trigger_semicolon && !modes.trigger_uppercase);
    assert!(!modes.mid_sentence);
    assert!(
        modes.mid_sentence_backtick
            && !modes.mid_sentence_semicolon
            && !modes.mid_sentence_uppercase
    );
    assert!(
        !modes.direct_helpcode && modes.direct_helpcode_slash && !modes.direct_helpcode_uppercase
    );
    // 默认值不写进文档：旧文档缺这一段时按默认读，默认的文档也不带这一段。
    let serialized = serde_json::to_value(Preferences::default()).unwrap();
    assert!(serialized["shuangpin_helpcode"].get("modes").is_none());
    assert!(serialized["quanpin_helpcode"].get("modes").is_none());
    let mut legacy = serialized;
    legacy["shuangpin_helpcode"]
        .as_object_mut()
        .unwrap()
        .remove("modes");
    assert_eq!(
        serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .shuangpin_helpcode
            .modes,
        ShuangpinHelpcodeModes::default()
    );
}

#[test]
fn shuangpin_helpcode_modes_roundtrip_and_stay_off_quanpin() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let modes = ShuangpinHelpcodeModes {
        direct: false,
        trigger_backtick: true,
        ..ShuangpinHelpcodeModes::default()
    };
    let saved = store
        .save(
            0,
            Preferences {
                shuangpin_helpcode: HelpcodePreferences {
                    modes,
                    ..default_shuangpin_helpcode()
                },
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(saved.preferences.shuangpin_helpcode.modes, modes);
    assert_eq!(
        store.load().unwrap().preferences.shuangpin_helpcode.modes,
        modes
    );
    // 全拼不写也不读这一段：保存后文档里 `quanpin_helpcode` 仍没有 `modes`。
    let document: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert!(document["preferences"]["quanpin_helpcode"]
        .get("modes")
        .is_none());
    assert_eq!(
        document["preferences"]["shuangpin_helpcode"]["modes"]["trigger_backtick"],
        true
    );
}

#[test]
fn save_normalizes_conflicting_shuangpin_helpcode_modes() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let modes = ShuangpinHelpcodeModes {
        mid_sentence: true,
        mid_sentence_semicolon: true,
        // 与句中同一枚触发键同时开着：归一后只归句中。
        trigger_semicolon: true,
        // 句中没认领的键仍归末尾（`mid_sentence_backtick` 出厂即真，反引号此时只能归句中）。
        trigger_uppercase: true,
        ..ShuangpinHelpcodeModes::default()
    };
    let saved = store
        .save(
            0,
            Preferences {
                shuangpin_helpcode: HelpcodePreferences {
                    modes,
                    ..default_shuangpin_helpcode()
                },
                ..Preferences::default()
            },
        )
        .unwrap();
    let normalized = saved.preferences.shuangpin_helpcode.modes;
    assert!(!normalized.trigger_semicolon && normalized.mid_sentence_semicolon);
    // 句中没认领的键仍归末尾；`mid_sentence_backtick` 出厂即真，反引号此时只能归句中。
    assert!(normalized.trigger_uppercase);
    assert_eq!(store.load().unwrap(), saved);

    // `mid_sentence` 与 `direct_helpcode` 同真时按万象式直接：归一后句中让位。
    let mut conflict = normalized;
    conflict.direct_helpcode = true;
    let saved = store
        .save(
            saved.revision,
            Preferences {
                shuangpin_helpcode: HelpcodePreferences {
                    modes: conflict,
                    ..default_shuangpin_helpcode()
                },
                ..Preferences::default()
            },
        )
        .unwrap();
    let normalized = saved.preferences.shuangpin_helpcode.modes;
    assert!(normalized.direct_helpcode && !normalized.mid_sentence);
}

#[test]
fn quanpin_autocorrect_defaults_on_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let defaults = Preferences::default();
    assert!(defaults.quanpin.autocorrect_transposition);
    assert!(defaults.quanpin.autocorrect_neighbor);

    let explicit = Preferences {
        quanpin: QuanpinPreferences {
            autocorrect_transposition: true,
            autocorrect_neighbor: false,
        },
        ..defaults
    };
    let saved = store.save(0, explicit).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded, saved);
    assert!(loaded.preferences.quanpin.autocorrect_transposition);
    assert!(!loaded.preferences.quanpin.autocorrect_neighbor);
}

#[test]
fn traditional_chinese_output_legacy_default_and_enabled_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("traditional_chinese_output");
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();
    assert!(!store.load().unwrap().preferences.traditional_chinese_output);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);
    let preferences = Preferences {
        traditional_chinese_output: true,
        ..Preferences::default()
    };
    store.save(0, preferences).unwrap();
    assert!(store.load().unwrap().preferences.traditional_chinese_output);
}

// 「中英文之间加空格」默认关，关着时不写进文档：更旧的版本读同一份文档不会因为不认识这个键而整份失败。打开后保存能读回。
#[test]
fn chinese_english_spacing_defaults_off_is_omitted_and_roundtrips() {
    let defaults = Preferences::default();
    assert!(!defaults.chinese_english_spacing);
    let written = serde_json::to_value(&defaults).unwrap();
    assert!(written.get("chinese_english_spacing").is_none());
    assert!(
        !serde_json::from_value::<Preferences>(written)
            .unwrap()
            .chinese_english_spacing
    );

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store
        .save(
            0,
            Preferences {
                chinese_english_spacing: true,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert!(saved.preferences.chinese_english_spacing);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(on_disk["preferences"]["chinese_english_spacing"], true);
    assert_eq!(store.load().unwrap(), saved);
}

#[test]
fn traditional_variant_defaults_to_s2t_is_omitted_and_roundtrips() {
    let defaults = Preferences::default();
    assert_eq!(
        defaults.traditional_chinese_variant,
        TraditionalVariant::Standard
    );
    let written = serde_json::to_value(&defaults).unwrap();
    assert!(written.get("traditional_chinese_variant").is_none());

    for (variant, name) in [
        (TraditionalVariant::Taiwan, "s2tw"),
        (TraditionalVariant::TaiwanPhrases, "s2twp"),
    ] {
        let preferences = Preferences {
            traditional_chinese_output: true,
            traditional_chinese_variant: variant,
            ..Preferences::default()
        };
        let written = serde_json::to_value(&preferences).unwrap();
        assert_eq!(written["traditional_chinese_variant"], name);
        assert_eq!(
            serde_json::from_value::<Preferences>(written).unwrap(),
            preferences
        );
    }

    let mut unknown = serde_json::to_value(&defaults).unwrap();
    unknown["traditional_chinese_variant"] = "s2hk".into();
    assert!(serde_json::from_value::<Preferences>(unknown).is_err());
}

#[test]
fn shuangpin_profiles_preserve_legacy_files_and_reject_unknown_values() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let legacy = r#"{"format_version":1,"revision":7,"preferences":{"scheme":"shuangpin","candidate_page_size":5,"learning":false,"chinese_punctuation":true}}"#;
    fs::write(store.path(), legacy).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.shuangpin_profile,
        ShuangpinProfile::Xiaohe
    );
    assert_eq!(fs::read_to_string(store.path()).unwrap(), legacy);
    let mut revision = 7;
    for profile in [
        ShuangpinProfile::Xiaohe,
        ShuangpinProfile::Ziranma,
        ShuangpinProfile::Shoudao,
        ShuangpinProfile::SmartAbc,
        ShuangpinProfile::Microsoft,
    ] {
        let saved = store
            .save(
                revision,
                Preferences {
                    scheme: InputScheme::Shuangpin,
                    shuangpin_profile: profile,
                    ..Preferences::default()
                },
            )
            .unwrap();
        assert_eq!(store.load().unwrap(), saved);
        revision = saved.revision;
    }
    let unknown = fs::read_to_string(store.path())
        .unwrap()
        .replace("microsoft", "future_profile");
    fs::write(store.path(), &unknown).unwrap();
    assert!(store.load().is_err());
    assert!(store.save(revision, Preferences::default()).is_err());
    assert_eq!(fs::read_to_string(store.path()).unwrap(), unknown);
}

#[test]
fn custom_shuangpin_profile_round_trips_and_stays_out_of_documents_without_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store.save(0, Preferences::default()).unwrap();
    assert!(!fs::read_to_string(store.path())
        .unwrap()
        .contains("shuangpin_custom_profile"));
    let mut preferences = Preferences {
        scheme: InputScheme::Shuangpin,
        shuangpin_profile: ShuangpinProfile::Custom,
        ..Preferences::default()
    };
    let table = &mut preferences.shuangpin_custom_profile;
    table.initials.insert("zh".into(), "a".into());
    table.finals.insert("ing".into(), ";".into());
    table.zero_initials.insert("a".into(), "oa".into());
    let saved = store.save(saved.revision, preferences.clone()).unwrap();
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.path()).unwrap()).unwrap();
    assert_eq!(document["preferences"]["shuangpin_profile"], "custom");
    assert_eq!(
        document["preferences"]["shuangpin_custom_profile"],
        serde_json::json!({
            "initials": { "zh": "a" },
            "finals": { "ing": ";" },
            "zero_initials": { "a": "oa" }
        })
    );
    assert_eq!(store.load().unwrap(), saved);
    // 结构上的限制：单位只能是小写字母，键是可见 ASCII，大小有上限。合不合法是 Engine 的事，这里不判断。
    for (unit, key) in [("Zh", "a"), ("zh", "é"), ("zh", "a b"), ("zhzhzhzhz", "a")] {
        let mut invalid = preferences.clone();
        invalid.shuangpin_custom_profile.initials = [(unit.to_owned(), key.to_owned())].into();
        assert!(
            matches!(
                invalid.validate(),
                Err(PreferencesError::InvalidShuangpinCustomProfile)
            ),
            "{unit:?} {key:?}"
        );
    }
    let mut oversized = preferences.clone();
    oversized.shuangpin_custom_profile.finals = (0..65u8)
        .map(|index| {
            (
                format!(
                    "{}{}",
                    char::from(b'a' + index / 26),
                    char::from(b'a' + index % 26)
                ),
                "q".to_owned(),
            )
        })
        .collect();
    assert!(matches!(
        oversized.validate(),
        Err(PreferencesError::InvalidShuangpinCustomProfile)
    ));
}

#[test]
fn nine_key_shuangpin_preferences_round_trip_and_stay_out_of_documents_without_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store.save(0, Preferences::default()).unwrap();
    let text = fs::read_to_string(store.path()).unwrap();
    // 没用过九键双拼的文档与以前的版本相同，以前的版本照样读得懂。
    assert!(!text.contains("nine_key_shuangpin"));
    assert_eq!(
        Preferences::default().nine_key_shuangpin_profile,
        NineKeyShuangpinProfile::Migala3
    );
    let mut preferences = Preferences {
        touch_keyboard_layout: TouchKeyboardLayout::NineKeyShuangpin,
        nine_key_shuangpin_profile: NineKeyShuangpinProfile::Custom,
        ..Preferences::default()
    };
    let table = &mut preferences.nine_key_shuangpin_custom_profile;
    table.initials.insert("zh".into(), "8".into());
    table.finals.insert("ong".into(), "1".into());
    table.zero_initials.insert("a".into(), "22".into());
    let saved = store.save(saved.revision, preferences.clone()).unwrap();
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store.path()).unwrap()).unwrap();
    assert_eq!(
        document["preferences"]["touch_keyboard_layout"],
        "nine_key_shuangpin"
    );
    assert_eq!(
        document["preferences"]["nine_key_shuangpin_profile"],
        "custom"
    );
    assert_eq!(
        document["preferences"]["nine_key_shuangpin_custom_profile"],
        serde_json::json!({
            "initials": { "zh": "8" },
            "finals": { "ong": "1" },
            "zero_initials": { "a": "22" }
        })
    );
    assert_eq!(store.load().unwrap(), saved);
    for (profile, id) in [
        (NineKeyShuangpinProfile::Migala2, "migala2"),
        (NineKeyShuangpinProfile::Migala3, "migala3"),
        (NineKeyShuangpinProfile::Gushi2Low, "gushi2_low"),
        (NineKeyShuangpinProfile::Custom, "custom"),
    ] {
        assert_eq!(serde_json::to_value(profile).unwrap(), id);
    }
    // 两张自定义表互不影响：九键的表不合规矩时报它自己的错。
    let mut invalid = preferences.clone();
    invalid.nine_key_shuangpin_custom_profile.initials = [("ZH".to_owned(), "8".to_owned())].into();
    assert!(matches!(
        invalid.validate(),
        Err(PreferencesError::InvalidNineKeyShuangpinCustomProfile)
    ));
    assert!(preferences.shuangpin_custom_profile.is_empty());
}

#[test]
fn wubi_profile_defaults_to_86_and_rejects_unknown_values() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let legacy = r#"{"format_version":1,"revision":3,"preferences":{"scheme":"wubi","candidate_page_size":5,"learning":false,"chinese_punctuation":true}}"#;
    fs::write(store.path(), legacy).unwrap();
    assert_eq!(
        store.load().unwrap().preferences.wubi_profile,
        WubiProfile::Wubi86
    );
    let saved = store
        .save(
            3,
            Preferences {
                scheme: InputScheme::Wubi,
                wubi_profile: WubiProfile::Wubi98,
                ..Preferences::default()
            },
        )
        .unwrap();
    let text = fs::read_to_string(store.path()).unwrap();
    assert!(text.contains(r#""wubi_profile": "wubi98""#), "{text}");
    assert_eq!(store.load().unwrap(), saved);
    fs::write(store.path(), text.replace("wubi98", "wubi06")).unwrap();
    assert!(store.load().is_err());
}

#[test]
fn try_load_distinguishes_busy_missing_and_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    assert_eq!(
        store.try_load().unwrap(),
        Some(PreferencesSnapshot::default())
    );
    let lock = store.lock().unwrap();
    assert_eq!(store.try_load().unwrap(), None);
    drop(lock);
    let saved = store.save(0, Preferences::default()).unwrap();
    assert_eq!(store.try_load().unwrap(), Some(saved));
    fs::write(store.path(), "broken").unwrap();
    assert!(store.try_load().is_err());
    assert_eq!(fs::read_to_string(store.path()).unwrap(), "broken");
}

#[test]
fn persists_across_instances_and_rejects_stale_save() {
    let dir = tempfile::tempdir().unwrap();
    let first = PreferencesStore::new(dir.path());
    let second = PreferencesStore::new(dir.path());
    let preferences = Preferences {
        learning: false,
        ..Preferences::default()
    };
    let saved = first.save(0, preferences).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(second.load().unwrap(), saved);
    assert!(matches!(
        second.save(0, Preferences::default()),
        Err(PreferencesError::Conflict)
    ));
    assert_eq!(first.load().unwrap(), saved);
}

#[test]
fn invalid_values_do_not_change_disk() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let initial = store.save(0, Preferences::default()).unwrap();
    for size in [0, 11, 255] {
        assert!(matches!(
            store.save(
                1,
                Preferences {
                    candidate_page_size: size,
                    ..Preferences::default()
                }
            ),
            Err(PreferencesError::InvalidPageSize)
        ));
    }
    assert_eq!(store.load().unwrap(), initial);
}

#[test]
fn ai_assistant_rejects_unknown_provider_and_invalid_candidate_limit() {
    let mut preferences = Preferences::default();
    for provider in AI_PROVIDERS {
        preferences.ai_assistant.provider = provider.into();
        assert!(
            preferences.validate().is_ok(),
            "{provider} should be accepted"
        );
    }
    preferences.ai_assistant.provider = "unknown".into();
    assert!(matches!(
        preferences.validate(),
        Err(PreferencesError::InvalidAiAssistant)
    ));
    preferences.ai_assistant.provider = "openai".into();
    preferences.ai_assistant.candidate_limit = 11;
    assert!(matches!(
        preferences.validate(),
        Err(PreferencesError::InvalidAiAssistant)
    ));
}

#[test]
fn candidate_font_size_bounds_are_strict() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let initial = store.save(0, Preferences::default()).unwrap();
    for size in [11, 33, 255] {
        assert!(matches!(
            store.save(
                1,
                Preferences {
                    candidate_font_size: size,
                    ..Preferences::default()
                }
            ),
            Err(PreferencesError::InvalidCandidateFontSize)
        ));
    }
    let saved = store
        .save(
            1,
            Preferences {
                candidate_font_size: 12,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(saved.preferences.candidate_font_size, 12);
    let other_dir = tempfile::tempdir().unwrap();
    let other = PreferencesStore::new(other_dir.path());
    other.save(0, Preferences::default()).unwrap();
    let saved = other
        .save(
            1,
            Preferences {
                candidate_font_size: 32,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(saved.preferences.candidate_font_size, 32);
    assert_eq!(initial.preferences.candidate_font_size, 18);
    assert_eq!(initial.preferences.candidate_preedit_font_size, 15);
    assert_eq!(initial.preferences.candidate_page_size, 6);
    // 新安装在所有构建上都跟随系统，自定义主题为空。
    assert_eq!(
        initial.preferences.global_theme,
        crate::theme::GlobalTheme::System
    );
    assert_eq!(initial.preferences.custom_theme, CustomTheme::default());
    assert_eq!(initial.preferences.theme, ThemeMode::System);
    assert_eq!(initial.preferences.candidate_font_family, "Noto Sans SC");
    assert_eq!(
        initial.preferences.candidate_fallback_fonts,
        vec!["Noto Sans SC".to_owned(), "Microsoft YaHei".to_owned()]
    );
}

#[test]
fn candidate_appearance_colors_accept_hex_and_reject_unsafe_values() {
    let mut preferences = Preferences::default();
    macro_rules! check {
        ($field:ident, $error:ident) => {{
            preferences.custom_theme.candidate_colors.$field = Some("#12aBcD".to_owned());
            assert!(preferences.validate().is_ok());
            for invalid in ["#12345678", "#123", "red", "12aBcD"] {
                preferences.custom_theme.candidate_colors.$field = Some(invalid.to_owned());
                assert!(matches!(
                    preferences.validate(),
                    Err(PreferencesError::$error)
                ));
            }
            preferences.custom_theme.candidate_colors.$field = None;
        }};
    }
    check!(text, InvalidCandidateTextColor);
    check!(number, InvalidCandidateNumberColor);
    check!(accent, InvalidCandidateAccentColor);
    check!(selected, InvalidCandidateSelectedColor);
    check!(hover, InvalidCandidateHoverColor);
    check!(surface, InvalidCandidateSurfaceColor);
    check!(border, InvalidCandidateBorderColor);
}

#[test]
fn custom_theme_candidate_skin_ids_are_safe_bounded_and_not_theme_ids() {
    let mut preferences = Preferences::default();
    for skin in ["fluent", "willow_green", "external.skin-1"] {
        preferences.custom_theme.candidate_skin = Some(skin.to_owned());
        assert!(preferences.validate().is_ok(), "{skin}");
    }
    for skin in [
        "",
        "-unsafe",
        "Upper",
        "../escape",
        &"a".repeat(65),
        "system",
        "night",
        "custom",
    ] {
        preferences.custom_theme.candidate_skin = Some(skin.to_owned());
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidCandidateSkin)
            ),
            "{skin}"
        );
    }
}

#[test]
fn unicode_font_families_and_ordered_fallbacks_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let fallback_fonts: Vec<String> = (0..32).map(|index| format!("示例字体{index}")).collect();
    let preferences = Preferences {
        candidate_font_family: "示例主字体".to_owned(),
        candidate_fallback_fonts: fallback_fonts.clone(),
        ..Preferences::default()
    };
    let saved = store.save(0, preferences).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.preferences.candidate_font_family, "示例主字体");
    assert_eq!(loaded.preferences.candidate_fallback_fonts, fallback_fonts);
    assert_eq!(loaded.revision, saved.revision);
}

#[test]
fn candidate_fallback_fonts_reject_invalid_lists() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let initial = store.save(0, Preferences::default()).unwrap();
    for fonts in [
        vec!["".to_owned()],
        vec!["a".repeat(129)],
        vec!["字".repeat(43)],
        (0..33).map(|index| format!("Font{index}")).collect(),
    ] {
        assert!(matches!(
            store.save(
                1,
                Preferences {
                    candidate_fallback_fonts: fonts,
                    ..Preferences::default()
                }
            ),
            Err(PreferencesError::InvalidCandidateFontFamily)
        ));
    }
    let valid = vec!["Noto Sans CJK SC".to_owned(); 32];
    let saved = store
        .save(
            1,
            Preferences {
                candidate_fallback_fonts: valid.clone(),
                ..Preferences::default()
            },
        )
        .unwrap();
    assert_eq!(saved.preferences.candidate_fallback_fonts, valid);
    assert_eq!(store.load().unwrap().revision, 2);
    assert_eq!(initial.revision, 1);
}

#[test]
fn unicode_font_names_retain_utf8_byte_budget() {
    let exact_limit = format!("{}ab", "字".repeat(42));
    assert_eq!(exact_limit.len(), 128);
    let mut preferences = Preferences {
        candidate_font_family: exact_limit.clone(),
        candidate_fallback_fonts: vec![exact_limit.clone()],
        ..Preferences::default()
    };
    assert!(preferences.validate().is_ok());
    for invalid in [String::new(), format!("{exact_limit}c"), "字".repeat(43)] {
        preferences.candidate_font_family = invalid;
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidCandidateFontFamily)
        ));
    }
}

#[test]
fn candidate_font_names_reject_control_characters() {
    for invalid in ["Primary\nFont", "Primary\u{7f}Font", "Primary\u{85}Font"] {
        let mut preferences = Preferences {
            candidate_font_family: invalid.to_owned(),
            ..Preferences::default()
        };
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidCandidateFontFamily)
        ));

        preferences.candidate_font_family = default_candidate_font_family();
        preferences.candidate_english_font = Some(invalid.to_owned());
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidCandidateFontFamily)
        ));

        preferences.candidate_english_font = None;
        preferences.candidate_fallback_fonts = vec![invalid.to_owned()];
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidCandidateFontFamily)
        ));
    }
}

#[test]
fn malformed_and_future_format_documents_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    for bytes in [
        "broken".to_owned(),
        serde_json::to_string(&PreferencesSnapshot {
            format_version: 2,
            ..PreferencesSnapshot::default()
        })
        .unwrap(),
    ] {
        fs::write(store.path(), &bytes).unwrap();
        assert!(store.save(0, Preferences::default()).is_err());
        assert_eq!(fs::read_to_string(store.path()).unwrap(), bytes);
    }
    // 只是多了更新版本的键的文档照常保存，那个键留在文档里。
    fs::write(store.path(), r#"{"format_version":1,"revision":0,"preferences":{"scheme":"quanpin","candidate_page_size":5,"learning":true,"chinese_punctuation":true,"future_option":true}}"#).unwrap();
    store.save(0, Preferences::default()).unwrap();
    let written: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(written["preferences"]["future_option"], true);
}

/// `value` 作为偏好对象读取时本版本不认识的键路径。
fn unknown_paths(value: &serde_json::Value) -> Vec<Vec<String>> {
    deserialize_keeping_unknown::<Preferences>(value, &[])
        .unwrap()
        .1
        .into_iter()
        .map(|field| field.path)
        .collect()
}

#[test]
fn concurrent_stores_have_exactly_one_winner() {
    let dir = tempfile::tempdir().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let barrier = barrier.clone();
            let store = PreferencesStore::new(dir.path());
            std::thread::spawn(move || {
                barrier.wait();
                store.save(0, Preferences::default())
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(PreferencesError::Conflict)))
            .count(),
        7
    );
}

#[test]
fn floating_toolbar_component_defaults_and_roundtrip() {
    let defaults = Preferences::default().floating_toolbar;
    assert!(
        defaults.enabled && defaults.english_mode && defaults.fullwidth && defaults.punctuation
    );
    assert!(defaults.character_set && defaults.settings && defaults.input_scheme);
    // The compact toolbar: these four are opt-in, so a new profile gets five buttons and turns on
    // the ones it wants. Emoji is one of the reference's own components and still defaults off here;
    // the Windows installer template keeps it on by setting every component explicitly.
    assert!(
        !defaults.emoji && !defaults.handwriting && !defaults.voice && !defaults.screen_keyboard
    );
    // 自动隐藏按 Windows 旧正式版 v0.11.1 的出厂配置：关、延时 5 秒。macOS 旧行为是常开 10 秒。
    assert!(!defaults.auto_hide);
    assert_eq!(defaults.auto_hide_delay, 5);
    // 阴影出厂为开，与 Windows v0.11.1 一致，也是 macOS 一直写死的行为。
    assert!(defaults.shadow);
    let json = serde_json::to_string(&Preferences::default()).unwrap();
    let restored: Preferences = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.floating_toolbar, defaults);
}

#[test]
fn legacy_preferences_without_toolbar_use_component_defaults() {
    let mut value = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    value["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("floating_toolbar");
    let restored: PreferencesSnapshot = serde_json::from_value(value).unwrap();
    assert!(restored.preferences.floating_toolbar.enabled);
    assert!(restored.preferences.floating_toolbar.english_mode);
    assert!(restored.preferences.floating_toolbar.fullwidth);
    assert!(restored.preferences.floating_toolbar.input_scheme);
    assert!(!restored.preferences.floating_toolbar.screen_keyboard);
    assert!(!restored.preferences.floating_toolbar.auto_hide);
    assert_eq!(restored.preferences.floating_toolbar.auto_hide_delay, 5);
    assert!(restored.preferences.floating_toolbar.shadow);
}

#[test]
fn tencent_translation_defaults_migrate_and_round_trip() {
    let defaults = Preferences::default();
    assert!(defaults.tencent_tmt.enabled);
    assert!(defaults.tencent_tmt.secret_id.is_empty());
    assert!(defaults.tencent_tmt.secret_key.is_empty());
    assert_eq!(defaults.tencent_tmt.region, "ap-guangzhou");
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy.as_object_mut().unwrap().remove("tencent_tmt");
    let restored: Preferences = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.tencent_tmt, defaults.tencent_tmt);
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut configured = defaults;
    configured.tencent_tmt.secret_id = "AKIDsynthetic".into();
    configured.tencent_tmt.secret_key = "synthetic".into();
    configured.tencent_tmt.region = "ap-shanghai".into();
    configured.candidate_page_size = 7;
    store.save(0, configured.clone()).unwrap();
    assert_eq!(store.load().unwrap().preferences, configured);
}

#[test]
fn tencent_translation_rejects_unbounded_or_injected_parameters() {
    for (id, key, region) in [
        ("x".repeat(4097), String::new(), String::new()),
        (String::new(), "x".repeat(4097), String::new()),
        ("bad id".into(), String::new(), String::new()),
        (String::new(), "bad\r\nkey".into(), String::new()),
        (String::new(), String::new(), "x".repeat(65)),
        (String::new(), String::new(), "region\r\nheader".into()),
    ] {
        let mut preferences = Preferences::default();
        preferences.tencent_tmt.secret_id = id;
        preferences.tencent_tmt.secret_key = key;
        preferences.tencent_tmt.region = region;
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidTencentTmt)
        ));
    }
}

#[test]
fn niutrans_translation_defaults_migrate_and_validate() {
    let defaults = Preferences::default();
    assert!(!defaults.niutrans.enabled);
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy.as_object_mut().unwrap().remove("niutrans");
    let restored: Preferences = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.niutrans, defaults.niutrans);
    let mut configured = defaults.clone();
    configured.niutrans.enabled = true;
    configured.niutrans.app_id = "synthetic-app".into();
    configured.niutrans.apikey = "synthetic-key".into();
    assert!(configured.validate().is_ok());
    for (app_id, apikey) in [
        ("x".repeat(4097), String::new()),
        (String::new(), "x".repeat(4097)),
        ("bad\napp".into(), String::new()),
        (String::new(), "bad\rkey".into()),
        ("<YOUR_APP_ID>".into(), String::new()),
        (String::new(), "FAKESECRET_key".into()),
    ] {
        let mut invalid = defaults.clone();
        invalid.niutrans.app_id = app_id;
        invalid.niutrans.apikey = apikey;
        assert!(matches!(
            invalid.validate(),
            Err(PreferencesError::InvalidNiuTrans)
        ));
    }
}

#[test]
fn custom_translation_defaults_and_validation_are_stable() {
    let defaults = Preferences::default();
    assert!(!defaults.custom_translation.enabled);
    assert!(defaults.custom_translation.endpoint.is_empty());
    let json = serde_json::to_string(&defaults).unwrap();
    let restored: Preferences = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.custom_translation, defaults.custom_translation);

    let mut valid = defaults.clone();
    valid.custom_translation.api_key = "masked-test-key".into();
    for endpoint in [
        "https://translate.example/api",
        "http://127.0.0.1:1188/translate",
        "http://[::1]:1188/translate",
    ] {
        valid.custom_translation.endpoint = endpoint.into();
        assert!(valid.validate().is_ok());
    }

    for endpoint in [
        "http://translate.example/api",
        "ftp://translate.example/api",
        "https://translate.example/\napi",
    ] {
        let mut invalid = defaults.clone();
        invalid.custom_translation.endpoint = endpoint.into();
        assert!(matches!(
            invalid.validate(),
            Err(PreferencesError::InvalidCustomTranslation)
        ));
    }
    let mut oversized = defaults;
    oversized.custom_translation.api_key = "x".repeat(4097);
    assert!(matches!(
        oversized.validate(),
        Err(PreferencesError::InvalidCustomTranslation)
    ));
}

// The settings page has five smart-punctuation toggles, and the Windows server
// reads all five out of the saved document. Three of them had no field here,
// and `deny_unknown_fields` means an unknown key does not get dropped - the
// whole save is rejected. Turning any one of them on therefore stopped the
// settings window saving anything at all.
#[test]
fn smart_punctuation_sub_switches_survive_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let keys = [
        "smart_punctuation_space_convert",
        "smart_punctuation_direct_digit",
        "smart_punctuation_direct_letter",
    ];

    // A document that predates them reads them as their defaults, which is what the family's
    // parent says: the two halves of 智能标点 follow it, and the space rewrite does not.
    let following_parent = Preferences::default().smart_punctuation;
    assert!(following_parent);
    let defaults = serde_json::to_value(Preferences::default()).unwrap();
    assert_eq!(
        defaults["smart_punctuation_space_convert"],
        serde_json::Value::Bool(false)
    );
    for key in [
        "smart_punctuation_direct_digit",
        "smart_punctuation_direct_letter",
    ] {
        assert_eq!(
            defaults[key],
            serde_json::Value::Bool(following_parent),
            "{key}"
        );
    }
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    for key in keys {
        legacy["preferences"]
            .as_object_mut()
            .unwrap()
            .remove(key)
            .unwrap();
    }
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert!(!loaded.smart_punctuation_space_convert);
    assert_eq!(loaded.smart_punctuation_direct_digit, following_parent);
    assert_eq!(loaded.smart_punctuation_direct_letter, following_parent);

    // One at a time, from all three off, so a key that was written into the wrong field shows up
    // as its neighbour turning on rather than being hidden by a default that already said true.
    for (revision, key) in keys.iter().enumerate() {
        let mut value = serde_json::to_value(Preferences::default()).unwrap();
        for other in keys {
            value[other] = false.into();
        }
        value[*key] = true.into();
        let saved = store
            .save(revision as u64, serde_json::from_value(value).unwrap())
            .unwrap();
        assert_eq!(
            saved.preferences.smart_punctuation_space_convert,
            *key == keys[0]
        );
        assert_eq!(
            saved.preferences.smart_punctuation_direct_digit,
            *key == keys[1]
        );
        assert_eq!(
            saved.preferences.smart_punctuation_direct_letter,
            *key == keys[2]
        );
        assert_eq!(store.load().unwrap(), saved);
    }
}

// 「大写锁定时使用英文标点」默认关：新装和缺这个键的旧文档都按关读入，打开后保存能读回。
#[test]
fn caps_lock_ascii_punctuation_defaults_off_and_survives_a_save() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    assert!(!Preferences::default().caps_lock_ascii_punctuation);

    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("caps_lock_ascii_punctuation")
        .unwrap();
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(
        !store
            .load()
            .unwrap()
            .preferences
            .caps_lock_ascii_punctuation
    );

    let saved = store
        .save(
            0,
            Preferences {
                caps_lock_ascii_punctuation: true,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert!(saved.preferences.caps_lock_ascii_punctuation);
    assert_eq!(store.load().unwrap(), saved);
}

// 智能标点在所有平台上默认开：新文档、缺键的文档都按开读入，数字后的 `.` 直接出英文。空格转换不跟着开，它改写的是用户已经看着落下去的字符，又没有反悔手势。文档里明写的值照样保留。
#[test]
fn smart_punctuation_defaults_on_everywhere() {
    let defaults = Preferences::default();
    assert!(defaults.smart_punctuation);
    assert!(defaults.smart_punctuation_repeat);
    assert!(defaults.smart_punctuation_direct_digit);
    assert!(defaults.smart_punctuation_direct_letter);
    assert!(!defaults.smart_punctuation_space_convert);
    // 新文档从一开始就带着迁移标记，所以之后关掉的值不会被当成旧默认改回去。
    assert!(defaults.smart_punctuation_migrated);

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let preferences = legacy["preferences"].as_object_mut().unwrap();
    preferences.remove("smart_punctuation").unwrap();
    preferences.remove("smart_punctuation_repeat").unwrap();
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert!(loaded.smart_punctuation);
    assert!(loaded.smart_punctuation_repeat);

    // A document that states the opposite keeps stating it.
    let mut stored = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    stored["preferences"]["smart_punctuation"] = false.into();
    stored["preferences"]["smart_punctuation_repeat"] = false.into();
    fs::write(store.path(), serde_json::to_vec(&stored).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert!(!loaded.smart_punctuation);
    assert!(!loaded.smart_punctuation_repeat);
}

/// 默认改变之前写下的文档：智能标点这一族存着 `false`，没有迁移标记。
fn smart_punctuation_legacy_document() -> serde_json::Value {
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let preferences = legacy["preferences"].as_object_mut().unwrap();
    for key in [
        "smart_punctuation",
        "smart_punctuation_repeat",
        "smart_punctuation_direct_digit",
        "smart_punctuation_direct_letter",
        "smart_punctuation_space_convert",
    ] {
        preferences.insert(key.into(), false.into());
    }
    assert_eq!(
        preferences.remove("smart_punctuation_migrated"),
        Some(true.into())
    );
    legacy
}

// 迁移本身与平台无关地验证：以前默认关的平台把四个开关打开，空格转换不动；其余平台旧文档里的 `false` 是用户自己关的，原样保留。两种都标上标记，第二次不再改。
#[test]
fn smart_punctuation_settle_flips_only_where_the_old_default_was_off() {
    let legacy: Preferences =
        serde_json::from_value(smart_punctuation_legacy_document()["preferences"].clone()).unwrap();
    assert!(!legacy.smart_punctuation_migrated);

    let mut desktop = legacy.clone();
    desktop.settle_smart_punctuation_default(true);
    assert!(desktop.smart_punctuation);
    assert!(desktop.smart_punctuation_repeat);
    assert!(desktop.smart_punctuation_direct_digit);
    assert!(desktop.smart_punctuation_direct_letter);
    assert!(!desktop.smart_punctuation_space_convert);
    assert!(desktop.smart_punctuation_migrated);
    // 用户之后关掉，再走一遍迁移也不会打开。
    desktop.smart_punctuation = false;
    desktop.settle_smart_punctuation_default(true);
    assert!(!desktop.smart_punctuation);

    let mut other = legacy;
    other.settle_smart_punctuation_default(false);
    assert!(!other.smart_punctuation);
    assert!(!other.smart_punctuation_repeat);
    assert!(!other.smart_punctuation_direct_digit);
    assert!(!other.smart_punctuation_direct_letter);
    assert!(other.smart_punctuation_migrated);
}

// 经过存储的完整路径：旧文档读入时迁移（macOS、Windows 打开，其余平台不动），文件在下一次真正的保存之前保持原样；用户随后关掉智能标点，保存下来的文档带着标记，再读还是关。
#[test]
fn smart_punctuation_migration_runs_once_and_a_later_off_sticks() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = smart_punctuation_legacy_document();
    legacy["revision"] = 3.into();
    let bytes = serde_json::to_vec(&legacy).unwrap();
    fs::write(store.path(), &bytes).unwrap();

    let flipped = SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT;
    let loaded = store.load().unwrap();
    assert_eq!(loaded.revision, 3);
    assert_eq!(loaded.preferences.smart_punctuation, flipped);
    assert_eq!(loaded.preferences.smart_punctuation_repeat, flipped);
    assert_eq!(loaded.preferences.smart_punctuation_direct_digit, flipped);
    assert_eq!(loaded.preferences.smart_punctuation_direct_letter, flipped);
    assert!(!loaded.preferences.smart_punctuation_space_convert);
    assert!(loaded.preferences.smart_punctuation_migrated);
    // 只读不写：两次读得到同样的结果，文件没有变。
    assert_eq!(store.load().unwrap(), loaded);
    assert_eq!(fs::read(store.path()).unwrap(), bytes);

    // 写入方的偏好里没有标记（比如照旧形状拼出来的 JSON），保存时也带上，否则下一次读会把刚关掉的又打开。
    let mut off = serde_json::to_value(&loaded.preferences).unwrap();
    off["smart_punctuation"] = false.into();
    // Android 等平台的旧值本来就是关；同时改变子开关，确保走真实保存而非无变化快速返回。
    off["smart_punctuation_repeat"] = (!flipped).into();
    off.as_object_mut()
        .unwrap()
        .remove("smart_punctuation_migrated")
        .unwrap();
    let off: Preferences = serde_json::from_value(off).unwrap();
    assert!(!off.smart_punctuation_migrated);
    let saved = store.save(3, off).unwrap();
    assert!(saved.preferences.smart_punctuation_migrated);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        on_disk["preferences"]["smart_punctuation_migrated"],
        serde_json::Value::Bool(true)
    );
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded, saved);
    assert!(!reloaded.preferences.smart_punctuation);
    assert_eq!(reloaded.preferences.smart_punctuation_repeat, !flipped);
    assert_eq!(reloaded.preferences.smart_punctuation_direct_digit, flipped);

    // 恢复默认设置回到默认开，标记仍在。
    let restored = store.restore_defaults(saved.revision).unwrap();
    assert!(restored.preferences.smart_punctuation);
    assert!(restored.preferences.smart_punctuation_migrated);
}

// 新装：第一份文档带着标记，用户关掉之后不会因为「没有标记」被当成旧文档打开。
#[test]
fn smart_punctuation_turned_off_on_a_fresh_profile_stays_off() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let first = store.write_first_document().unwrap();
    assert!(first.preferences.smart_punctuation);
    assert!(first.preferences.smart_punctuation_migrated);
    let saved = store
        .save(
            first.revision,
            Preferences {
                smart_punctuation: false,
                smart_punctuation_repeat: false,
                ..first.preferences
            },
        )
        .unwrap();
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded, saved);
    assert!(!reloaded.preferences.smart_punctuation);
    assert!(!reloaded.preferences.smart_punctuation_repeat);
}

// 修复损坏文档时，没有标记的旧文档照样迁移一次；带着标记的文档里关掉的值原样保留。
#[test]
fn smart_punctuation_recovery_migrates_only_a_legacy_document() {
    let flipped = SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT;
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = smart_punctuation_legacy_document();
    legacy["preferences"]["learning"] = serde_json::json!("yes");
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(store.load().is_err());
    let (snapshot, _, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(salvaged);
    assert_eq!(snapshot.preferences.smart_punctuation, flipped);
    assert_eq!(
        snapshot.preferences.smart_punctuation_direct_letter,
        flipped
    );
    assert!(snapshot.preferences.smart_punctuation_migrated);
    assert_eq!(store.load().unwrap(), snapshot);

    let mut current = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    current["preferences"]["smart_punctuation"] = false.into();
    current["preferences"]["learning"] = serde_json::json!("yes");
    fs::write(store.path(), serde_json::to_vec(&current).unwrap()).unwrap();
    let (snapshot, _, _) = expect_recovered(store.recover().unwrap());
    assert!(!snapshot.preferences.smart_punctuation);
}

// The source ships muting, DDC and text polishing on; `config.default.toml` says so for Windows, and macOS follows the desktop product it ports. Each is a plain voice switch, so no platform reason keeps them off there. The other hosts keep what they have shipped, and a stored value is never reinterpreted.
#[test]
fn voice_first_run_follows_the_source_on_desktop_ports() {
    let expected = cfg!(any(windows, target_os = "macos"));
    let voice = Preferences::default().voice_input;
    assert_eq!(voice.mute_system_audio, expected);
    assert_eq!(voice.doubao_enable_ddc, expected);
    assert_eq!(voice.polish_text, expected);
    // Polishing the text is not the same switch as the separate polish pass; that one stays off.
    assert!(!voice.polish_enabled);

    let keys = ["mute_system_audio", "doubao_enable_ddc", "polish_text"];
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    let section = legacy["preferences"]["voice_input"]
        .as_object_mut()
        .unwrap();
    for key in keys {
        section.remove(key).unwrap();
    }
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences.voice_input;
    assert_eq!(loaded.mute_system_audio, expected);
    assert_eq!(loaded.doubao_enable_ddc, expected);
    assert_eq!(loaded.polish_text, expected);

    let mut stored = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    for key in keys {
        stored["preferences"]["voice_input"][key] = (!expected).into();
    }
    fs::write(store.path(), serde_json::to_vec(&stored).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences.voice_input;
    assert_eq!(loaded.mute_system_audio, !expected);
    assert_eq!(loaded.doubao_enable_ddc, !expected);
    assert_eq!(loaded.polish_text, !expected);
}

// The source template ships voice polishing through DeepSeek (`deepseek-v4-flash`) and the AI assistant on against the same endpoint and model; the Windows installer template says the same, and macOS follows the desktop product it ports. The other hosts keep SiliconFlow/Qwen and the assistant off, and a stored value is never reinterpreted.
#[test]
fn polish_and_ai_first_run_follow_the_source_on_desktop_ports() {
    let desktop = cfg!(any(windows, target_os = "macos"));
    let defaults = Preferences::default();
    defaults.validate().expect("default preferences validate");
    let voice = &defaults.voice_input;
    let ai = &defaults.ai_assistant;
    if desktop {
        assert_eq!(voice.polish_provider, "deepseek");
        assert_eq!(
            voice.polish_endpoint,
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(voice.polish_model, "deepseek-v4-flash");
        assert!(ai.enabled);
        assert_eq!(ai.endpoint, "https://api.deepseek.com/chat/completions");
        assert_eq!(ai.model, "deepseek-v4-flash");
    } else {
        assert_eq!(voice.polish_provider, "siliconflow");
        assert_eq!(
            voice.polish_endpoint,
            "https://api.siliconflow.cn/v1/chat/completions"
        );
        assert_eq!(voice.polish_model, "Qwen/Qwen3-8B");
        assert!(!ai.enabled);
        assert!(ai.endpoint.is_empty());
        assert!(ai.model.is_empty());
    }
    assert_eq!(ai.provider, "deepseek");
    // No token ships, so turning the assistant on sends nothing until the user adds one.
    assert!(ai.token.is_empty() && ai.tokens.is_empty());
    assert!(voice.polish_token.is_empty() && voice.polish_tokens.is_empty());

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]["ai_assistant"]
        .as_object_mut()
        .unwrap()
        .remove("enabled")
        .unwrap();
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert_eq!(loaded.ai_assistant.enabled, desktop);

    let mut stored = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    stored["preferences"]["ai_assistant"]["enabled"] = false.into();
    stored["preferences"]["voice_input"]["polish_provider"] = "siliconflow".into();
    stored["preferences"]["voice_input"]["polish_endpoint"] =
        "https://api.siliconflow.cn/v1/chat/completions".into();
    stored["preferences"]["voice_input"]["polish_model"] = "Qwen/Qwen3-8B".into();
    fs::write(store.path(), serde_json::to_vec(&stored).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert!(!loaded.ai_assistant.enabled);
    assert_eq!(loaded.voice_input.polish_provider, "siliconflow");
    assert_eq!(
        loaded.voice_input.polish_endpoint,
        "https://api.siliconflow.cn/v1/chat/completions"
    );
    assert_eq!(loaded.voice_input.polish_model, "Qwen/Qwen3-8B");
}

#[test]
fn mixed_emoji_first_run_follows_the_source_on_desktop_ports() {
    let expected = cfg!(any(windows, target_os = "macos"));
    let mixed = Preferences::default().mixed_input;
    assert_eq!(mixed.emoji, expected);
    // The source ships kaomoji mixed-input off on every platform.
    assert!(!mixed.kaomoji);

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut stored = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    stored["preferences"]["mixed_input"]["emoji"] = (!expected).into();
    fs::write(store.path(), serde_json::to_vec(&stored).unwrap()).unwrap();
    let loaded = store.load().unwrap().preferences.mixed_input;
    assert_eq!(loaded.emoji, !expected);
    assert!(!loaded.kaomoji);
}

/// The HarmonyOS host has no way to match on this type.
///
/// Its settings page reaches this store through the C ABI, whose error channel is a single string
/// shared by every entry point, so it recovers the code the shared UI decodes by matching the text
/// itself in `PreferencesErrorCode` (`platforms/harmony/entry/src/main/ets/keyboard/settings/`).
/// Rewording a variant there silently costs that failure its own sentence, so the wordings that
/// mapping names are pinned here, where the rewording would happen.
#[test]
fn error_wordings_the_harmony_host_matches_on() {
    assert_eq!(
        PreferencesError::Conflict.to_string(),
        "preferences changed; reload before saving"
    );
    assert_eq!(
        PreferencesError::InvalidPageSize.to_string(),
        "candidate page size must be between 1 and 10"
    );
    assert_eq!(
        PreferencesError::InvalidFrequency.to_string(),
        "frequency trigger count and linear step must be between 1 and 10"
    );
    assert_eq!(
        PreferencesError::InvalidMixedInput.to_string(),
        "mixed English minimum prefix must be between 1 and 8"
    );
    assert_eq!(
        PreferencesError::InvalidFloatingToolbar.to_string(),
        "floating toolbar settings are invalid"
    );
    assert_eq!(
        PreferencesError::ConflictingKeyBindings.to_string(),
        "word-to-character and paging cannot use the same keys"
    );
    assert_eq!(
        PreferencesError::UnsupportedFormat.to_string(),
        "unsupported preferences format"
    );
    // Matched by prefix, because the cause it carries varies.
    assert!(
        PreferencesError::Json(serde_json::from_str::<u8>("x").unwrap_err())
            .to_string()
            .starts_with("invalid preferences document:")
    );
}

/// A restore puts every setting back and costs the user no secret.
///
/// The secrets are asserted by walking the serialized document rather than by listing the fields:
/// a credential added later would otherwise be reset by `restored_to_defaults` with nothing to
/// notice it, and the whole point of the method is that it never silently throws one away.
#[test]
fn restoring_defaults_keeps_what_cannot_be_retyped() {
    let mut edited = Preferences {
        candidate_page_size: 7,
        learning: !Preferences::default().learning,
        scheme: InputScheme::Wubi,
        ..Preferences::default()
    };
    edited.keybindings.switch_language_shift = false;
    edited.fuzzy_pinyin.seeded = true;
    edited.fuzzy_pinyin.enabled = true;
    edited.voice_input.asr_provider = "doubao".into();
    edited.voice_input.asr_token = "fixture-asr-token".into();
    edited.voice_input.asr_endpoint = "https://asr.example.test/v1".into();
    edited.voice_input.asr_model_path = "/Users/fixture/voice-models/sense-voice-small".into();
    edited.voice_input.asr_model_mirror = "https://mirror.example.test/".into();
    edited.voice_input.polish_token = "fixture-polish-token".into();
    edited.voice_input.polish_enabled = true;
    edited.ai_assistant.token = "fixture-ai-token".into();
    edited.ai_assistant.enabled = !Preferences::default().ai_assistant.enabled;
    edited.custom_translation.api_key = "fixture-custom-key".into();
    edited.tencent_tmt.secret_id = "fixture-tencent-id".into();
    edited.tencent_tmt.secret_key = "fixture-tencent-key".into();
    edited.niutrans.app_id = "fixture-niutrans-app".into();
    edited.niutrans.apikey = "fixture-niutrans-key".into();
    edited.cloud_clipboard_relay.url = "https://relay.example.test/msime".into();
    edited.cloud_clipboard_relay.token = "fixture-relay-token".into();

    let restored = edited.restored_to_defaults();
    let defaults = Preferences::default();

    // The settings are back.
    assert_eq!(restored.candidate_page_size, defaults.candidate_page_size);
    assert_eq!(restored.learning, defaults.learning);
    assert_eq!(restored.scheme, defaults.scheme);
    assert_eq!(
        restored.keybindings.switch_language_shift,
        defaults.keybindings.switch_language_shift
    );
    assert_eq!(restored.fuzzy_pinyin.enabled, defaults.fuzzy_pinyin.enabled);
    assert!(!restored.voice_input.polish_enabled);
    assert_eq!(restored.ai_assistant.enabled, defaults.ai_assistant.enabled);

    // The fixture itself has to be a document the store would accept, or this proves nothing.
    edited.validate().expect("edited fixture validates");

    // Nothing that reads as a secret was dropped, whoever adds one next.
    let document = serde_json::to_string(&restored).expect("restored preferences serialize");
    for fixture in [
        "fixture-asr-token",
        "fixture-polish-token",
        "fixture-ai-token",
        "fixture-custom-key",
        "fixture-tencent-id",
        "fixture-tencent-key",
        "fixture-niutrans-key",
        "fixture-relay-token",
    ] {
        assert!(
            document.contains(fixture),
            "{fixture} did not survive a restore"
        );
    }
    // What addresses the same service travels with it.
    assert_eq!(restored.voice_input.asr_provider, "doubao");
    assert_eq!(
        restored.voice_input.asr_endpoint,
        "https://asr.example.test/v1"
    );
    assert_eq!(
        restored.voice_input.asr_model_path,
        "/Users/fixture/voice-models/sense-voice-small"
    );
    assert_eq!(
        restored.voice_input.asr_model_mirror,
        "https://mirror.example.test/"
    );
    assert_eq!(
        restored.cloud_clipboard_relay.url,
        "https://relay.example.test/msime"
    );
    // The seeding marker is not a setting: clearing it would re-seed rules the user turned off.
    assert!(restored.fuzzy_pinyin.seeded);

    // A restore is idempotent and produces a document the store will accept.
    restored.validate().expect("restored preferences validate");
    assert_eq!(restored.restored_to_defaults(), restored);
}

// Staged writes nobody is going to finish.
//
// `NamedTempFile` removes itself when dropped, but a process killed between creating the file and
// renaming it drops nothing, and the input method is stopped exactly that way every time it is
// reinstalled. Two such files were sitting in the data directory of a machine running this client,
// one holding a copy of the preferences and one of the typing statistics; nothing would ever have
// removed them.
#[test]
fn saving_clears_staged_writes_that_were_abandoned() {
    use std::time::{Duration, SystemTime};

    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    store.save(0, Preferences::default()).unwrap();

    let age = |path: &std::path::Path, seconds: u64| {
        let when = SystemTime::now() - Duration::from_secs(seconds);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(when))
            .unwrap();
    };

    let abandoned = directory.path().join(".tmpAbandoned");
    std::fs::write(&abandoned, b"{}").unwrap();
    age(&abandoned, 48 * 60 * 60);

    // A staged write from a moment ago may belong to something still running - the statistics
    // document stages into this same directory under a lock of its own - so it is left alone.
    let in_flight = directory.path().join(".tmpInFlight");
    std::fs::write(&in_flight, b"{}").unwrap();

    // Age alone is not the rule: a file that is not a staged write keeps its place however old it
    // is, and so does a directory that happens to be named like one.
    let unrelated = directory.path().join("old-export.json");
    std::fs::write(&unrelated, b"{}").unwrap();
    age(&unrelated, 48 * 60 * 60);
    let directory_named_like_a_temporary = directory.path().join(".tmpDirectory");
    std::fs::create_dir(&directory_named_like_a_temporary).unwrap();

    store
        .save(
            1,
            Preferences {
                candidate_font_size: 20,
                ..Preferences::default()
            },
        )
        .unwrap();

    assert!(!abandoned.exists(), "the abandoned staged write is removed");
    assert!(in_flight.exists(), "a staged write from a moment ago stays");
    assert!(unrelated.exists());
    assert!(directory_named_like_a_temporary.is_dir());
    // And the save itself did what it was asked.
    assert_eq!(store.load().unwrap().preferences.candidate_font_size, 20);
}

fn corrupt_backups(directory: &Path) -> Vec<PathBuf> {
    let mut backups: Vec<PathBuf> = fs::read_dir(directory)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("preferences.json.corrupt-"))
        })
        .collect();
    backups.sort();
    backups
}

fn expect_recovered(outcome: RecoveryOutcome) -> (PreferencesSnapshot, PathBuf, bool) {
    match outcome {
        RecoveryOutcome::Recovered {
            snapshot,
            backup_path,
            salvaged,
        } => (snapshot, backup_path, salvaged),
        RecoveryOutcome::NotNeeded(_) => panic!("expected a recovery"),
    }
}

#[test]
fn recover_backs_up_truncated_json_and_writes_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let damaged = br#"{"format_version":1,"revision":4,"preferences":{"scheme":"#;
    fs::write(directory.path().join("preferences.json"), damaged).unwrap();
    assert!(matches!(store.load(), Err(PreferencesError::Json(_))));

    let (snapshot, backup, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(!salvaged);
    assert_eq!(fs::read(&backup).unwrap(), damaged);
    assert_eq!(backup.parent().unwrap(), directory.path());
    let name = backup.file_name().unwrap().to_str().unwrap();
    let stamp = name.strip_prefix("preferences.json.corrupt-").unwrap();
    assert_eq!(stamp.len(), 15);
    assert_eq!(stamp.as_bytes()[8], b'-');
    assert!(stamp
        .bytes()
        .enumerate()
        .all(|(index, byte)| index == 8 || byte.is_ascii_digit()));
    assert_eq!(snapshot.preferences, Preferences::default());
    // The revision could not be read, so it restarts above anything a host counted to.
    assert!(snapshot.revision > 1_000_000_000);
    assert_eq!(store.load().unwrap(), snapshot);
    // The repaired document saves like any other.
    store
        .save(snapshot.revision, Preferences::default())
        .unwrap();
}

#[test]
fn recover_treats_an_empty_document_as_damaged() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    fs::write(directory.path().join("preferences.json"), b"").unwrap();
    let (snapshot, backup, salvaged) = expect_recovered(store.recover_malformed().unwrap());
    assert!(!salvaged);
    assert!(fs::read(backup).unwrap().is_empty());
    assert_eq!(store.load().unwrap(), snapshot);
}

#[test]
fn recover_drops_only_the_unknown_and_ill_typed_fields() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut preferences = Preferences {
        scheme: InputScheme::Wubi,
        candidate_page_size: 7,
        candidate_font_size: 22,
        clipboard_history: true,
        ..Preferences::default()
    };
    preferences.ai_assistant.token = "synthetic-ai-token".into();
    let saved = store.save(0, preferences.clone()).unwrap();
    let mut document = serde_json::to_value(&saved).unwrap();
    document["preferences"]["from_a_newer_build"] = serde_json::json!(true);
    document["preferences"]["learning"] = serde_json::json!("yes");
    fs::write(
        directory.path().join("preferences.json"),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();
    assert!(store.load().is_err());

    let (snapshot, _, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(salvaged);
    assert_eq!(snapshot.revision, saved.revision + 1);
    let expected = Preferences {
        learning: Preferences::default().learning,
        ..preferences
    };
    assert_eq!(snapshot.preferences, expected);
    assert_eq!(store.load().unwrap(), snapshot);
}

#[test]
fn recover_keeps_a_credential_beside_a_bad_sibling() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut preferences = Preferences::default();
    preferences.custom_translation.endpoint = "https://translate.example/api".into();
    preferences.custom_translation.api_key = "synthetic-translation-key".into();
    preferences.tencent_tmt.secret_id = "SyntheticId".into();
    let saved = store.save(0, preferences).unwrap();
    let mut document = serde_json::to_value(&saved).unwrap();
    document["preferences"]["custom_translation"]["enabled"] = serde_json::json!("sometimes");
    document["preferences"]["tencent_tmt"]["region"] = serde_json::json!("not a region!");
    fs::write(
        directory.path().join("preferences.json"),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();

    let (snapshot, _, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(salvaged);
    let translation = &snapshot.preferences.custom_translation;
    assert_eq!(translation.api_key, "synthetic-translation-key");
    assert_eq!(translation.endpoint, "https://translate.example/api");
    assert_eq!(
        translation.enabled,
        CustomTranslationPreferences::default().enabled
    );
    assert_eq!(snapshot.preferences.tencent_tmt.secret_id, "SyntheticId");
    assert_eq!(
        snapshot.preferences.tencent_tmt.region,
        TencentTmtPreferences::default().region
    );
}

#[test]
fn recover_handles_a_future_format_but_malformed_scope_leaves_it() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let saved = store
        .save(
            0,
            Preferences {
                candidate_page_size: 6,
                ..Preferences::default()
            },
        )
        .unwrap();
    let mut document = serde_json::to_value(&saved).unwrap();
    document["format_version"] = serde_json::json!(2);
    let bytes = serde_json::to_vec(&document).unwrap();
    let path = directory.path().join("preferences.json");
    fs::write(&path, &bytes).unwrap();

    // The automatic path refuses a well-formed document from a newer build.
    assert!(matches!(
        store.recover_malformed(),
        Err(PreferencesError::UnsupportedFormat)
    ));
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert!(corrupt_backups(directory.path()).is_empty());

    let (snapshot, backup, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(salvaged);
    assert_eq!(fs::read(backup).unwrap(), bytes);
    assert_eq!(snapshot.format_version, 1);
    assert_eq!(snapshot.revision, saved.revision + 1);
    assert_eq!(snapshot.preferences.candidate_page_size, 6);
}

#[test]
fn recover_is_not_needed_for_valid_missing_or_repaired_documents() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    assert_eq!(
        store.recover().unwrap(),
        RecoveryOutcome::NotNeeded(PreferencesSnapshot::default())
    );
    assert!(!directory.path().join("preferences.json").exists());

    let saved = store.save(0, Preferences::default()).unwrap();
    let before = fs::read(directory.path().join("preferences.json")).unwrap();
    assert_eq!(
        store.recover().unwrap(),
        RecoveryOutcome::NotNeeded(saved.clone())
    );
    assert_eq!(
        fs::read(directory.path().join("preferences.json")).unwrap(),
        before
    );
    assert!(corrupt_backups(directory.path()).is_empty());

    fs::write(directory.path().join("preferences.json"), b"{").unwrap();
    let (repaired, _, _) = expect_recovered(store.recover().unwrap());
    assert_eq!(
        store.recover().unwrap(),
        RecoveryOutcome::NotNeeded(repaired)
    );
    assert_eq!(corrupt_backups(directory.path()).len(), 1);
}

#[test]
fn recover_suffixes_a_backup_name_that_is_taken() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    // 固定备份时间，确保重名分支不依赖设备能否在一秒内完成两次同步写入。
    let stamp = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let first = store.write_backup(b"first", stamp).unwrap();
    let second = store.write_backup(b"second", stamp).unwrap();
    let third = store.write_backup(b"third", stamp).unwrap();
    assert_eq!(fs::read(&first).unwrap(), b"first");
    assert_eq!(fs::read(&second).unwrap(), b"second");
    assert_eq!(fs::read(&third).unwrap(), b"third");
    let first = first.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        second.file_name().unwrap().to_str().unwrap(),
        format!("{first}-1")
    );
    assert_eq!(
        third.file_name().unwrap().to_str().unwrap(),
        format!("{first}-2")
    );

    // 完整恢复保留既有备份和本次损坏内容。
    let path = directory.path().join("preferences.json");
    fs::write(&path, b"damaged").unwrap();
    let (_, backup, _) = expect_recovered(store.recover().unwrap());
    assert_eq!(fs::read(backup).unwrap(), b"damaged");
    assert_eq!(corrupt_backups(directory.path()).len(), 4);
}

#[cfg(unix)]
#[test]
fn recover_leaves_the_original_when_the_backup_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let path = directory.path().join("preferences.json");
    fs::write(&path, b"{\"format_version\":").unwrap();
    // Create the lock file while the directory is still writable.
    assert!(store.load().is_err());
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o555)).unwrap();
    let probe = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.path().join("probe"));
    if probe.is_ok() {
        // Running with privileges that ignore directory permissions; nothing to observe.
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let result = store.recover();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(result, Err(PreferencesError::Io(_))));
    assert_eq!(fs::read(&path).unwrap(), b"{\"format_version\":");
    assert!(corrupt_backups(directory.path()).is_empty());
}

#[test]
fn touch_toolbar_keeps_the_original_buttons_by_default_and_round_trips_partial_documents() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("touch_toolbar");
    fs::write(store.path(), serde_json::to_vec(&legacy).unwrap()).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(
        loaded.preferences.touch_toolbar,
        TouchToolbarPreferences::default()
    );
    let defaults = loaded.preferences.touch_toolbar;
    assert!(defaults.layout && defaults.emoji && defaults.skin);
    assert!(!defaults.undo && !defaults.redo);
    assert!(!defaults.clipboard && !defaults.ai && !defaults.character_set);

    // 缺少新开关的旧文档按共享默认值补全，不能让升级后的工具栏自动增加按钮。
    let partial: TouchToolbarPreferences =
        serde_json::from_value(serde_json::json!({"emoji": false, "clipboard": true})).unwrap();
    assert!(!partial.emoji && partial.clipboard && partial.layout && !partial.ai);
    assert!(!partial.undo && !partial.redo);

    let saved = store
        .save(
            0,
            Preferences {
                touch_toolbar: TouchToolbarPreferences {
                    skin: false,
                    punctuation: true,
                    undo: true,
                    redo: true,
                    ..TouchToolbarPreferences::default()
                },
                ..Preferences::default()
            },
        )
        .unwrap();
    assert!(!saved.preferences.touch_toolbar.skin);
    assert!(saved.preferences.touch_toolbar.punctuation);
    let reloaded = store.load().unwrap().preferences.touch_toolbar;
    assert!(!reloaded.skin);
    assert!(reloaded.undo && reloaded.redo);
    assert!(serde_json::from_value::<TouchToolbarPreferences>(
        serde_json::json!({"undo": "true", "redo": false})
    )
    .is_err());
}

#[test]
fn harmony_toolbar_defaults_to_every_tool_shown_and_stays_out_of_default_documents() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let defaults = HarmonyToolbarPreferences::default();
    // 默认就是没有这组设置时的鸿蒙键盘：五个工具全在，整行照常显示。
    assert!(defaults.emoji && defaults.phrase && defaults.clipboard);
    assert!(defaults.skin && defaults.scheme && !defaults.hidden);
    let document = serde_json::to_value(Preferences::default()).unwrap();
    assert!(document.get("harmony_toolbar").is_none());

    // 只写了几项的对象，其余按默认读。
    let partial: HarmonyToolbarPreferences =
        serde_json::from_value(serde_json::json!({"clipboard": false, "hidden": true})).unwrap();
    assert!(!partial.clipboard && partial.hidden);
    assert!(partial.emoji && partial.phrase && partial.skin && partial.scheme);

    let edited = Preferences {
        harmony_toolbar: HarmonyToolbarPreferences {
            phrase: false,
            hidden: true,
            ..HarmonyToolbarPreferences::default()
        },
        ..Preferences::default()
    };
    assert_eq!(
        serde_json::to_value(&edited).unwrap()["harmony_toolbar"],
        serde_json::json!({
            "emoji": true, "phrase": false, "clipboard": true,
            "skin": true, "scheme": true, "hidden": true
        })
    );
    let saved = store.save(0, edited).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert!(!saved.preferences.harmony_toolbar.phrase);
    assert!(saved.preferences.harmony_toolbar.hidden);
}

#[test]
fn saving_unchanged_preferences_keeps_the_revision_and_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    // The first save creates the document even though it holds only defaults.
    let created = store.save(0, Preferences::default()).unwrap();
    assert_eq!(created.revision, 1);
    let path = directory.path().join("preferences.json");
    let written = std::fs::metadata(&path).unwrap().modified().unwrap();

    let again = store.save(1, Preferences::default()).unwrap();
    assert_eq!(again, created);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        written
    );
    // A stale revision is still a conflict, unchanged content or not.
    assert!(matches!(
        store.save(0, Preferences::default()),
        Err(PreferencesError::Conflict)
    ));

    let mut changed = Preferences::default();
    changed.usage_reporting = !changed.usage_reporting;
    assert_eq!(store.save(1, changed).unwrap().revision, 2);
}

#[test]
fn mint_morning_is_the_community_design_and_a_valid_custom_theme() {
    let design = TouchKeyboardSkinDesign::mint_morning();
    assert!(design.validate());
    // The 水杉精选 design of the same name as the skin community serves it (camelCase, as the Apple editor writes it).
    let expected = serde_json::json!({
        "accent": 0x245A43, "shadow": 0.08, "pattern": 0, "background": 0xD8F0E4,
        "monospaced": false, "borderWidth": 0.5, "gradientEnd": 0xEEF6DD, "cornerRadius": 14.0,
        "keyBackground": 0xFAFFF9, "keyForeground": 0x173D30, "actionBackground": 0x245A43,
        "customBorderColor": 0xB6D8C5,
    });
    let written = serde_json::to_value(&design).unwrap();
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(&written[key], value, "{key}");
    }
    let preferences = Preferences {
        global_theme: crate::theme::GlobalTheme::Custom,
        custom_theme: CustomTheme {
            keyboard: Some(design),
            ..CustomTheme::default()
        },
        ..Preferences::default()
    };
    assert!(preferences.validate().is_ok());
    // 薄荷晨光不再是触屏构建的默认值：新安装在桌面和触屏构建（Android、iOS、鸿蒙）上都跟随系统，自定义主题为空。client-core 的单测也在 Android 模拟器上整套运行（`platforms/android/tests/device/run-core-test.sh`），两种构建都要守住这一点。
    assert_eq!(
        Preferences::default().global_theme,
        crate::theme::GlobalTheme::System
    );
    assert_eq!(Preferences::default().custom_theme, CustomTheme::default());
}

#[test]
fn candidate_window_style_defaults_round_trip() {
    let preferences = Preferences::default();
    assert_eq!(preferences.candidate_scale_percent, 100);
    assert_eq!(preferences.candidate_opacity_percent, 100);
    assert_eq!(preferences.candidate_corner_radius, None);

    let value = serde_json::to_value(&preferences).unwrap();
    assert_eq!(value["candidate_scale_percent"], 100);
    assert_eq!(value["candidate_opacity_percent"], 100);
    assert!(value.get("candidate_corner_radius").is_none());
    let decoded: Preferences = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, preferences);
}

#[test]
fn candidate_window_style_round_trips_through_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let styled = Preferences {
        candidate_scale_percent: 125,
        candidate_opacity_percent: 80,
        candidate_corner_radius: Some(0),
        ..Preferences::default()
    };
    let value = serde_json::to_value(&styled).unwrap();
    assert_eq!(value["candidate_scale_percent"], 125);
    assert_eq!(value["candidate_opacity_percent"], 80);
    // Zero is a real choice, square corners, and must not collapse into "follow the skin".
    assert_eq!(value["candidate_corner_radius"], 0);

    store.save(0, styled.clone()).unwrap();
    let loaded = store.load().unwrap().preferences;
    assert_eq!(loaded, styled);
}

#[test]
fn candidate_window_style_rejects_out_of_range_values() {
    for (scale, opacity, radius) in [
        (49, 100, None),
        (201, 100, None),
        (100, 49, None),
        (100, 101, None),
        (100, 100, Some(33)),
    ] {
        let preferences = Preferences {
            candidate_scale_percent: scale,
            candidate_opacity_percent: opacity,
            candidate_corner_radius: radius,
            ..Preferences::default()
        };
        assert!(
            matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidCandidateWindowStyle)
            ),
            "{scale} {opacity} {radius:?} was accepted"
        );
    }
    for (scale, opacity, radius) in [(50, 50, Some(0)), (200, 100, Some(32)), (75, 95, None)] {
        let preferences = Preferences {
            candidate_scale_percent: scale,
            candidate_opacity_percent: opacity,
            candidate_corner_radius: radius,
            ..Preferences::default()
        };
        assert!(preferences.validate().is_ok());
    }

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    assert!(matches!(
        store.save(
            0,
            Preferences {
                candidate_opacity_percent: 20,
                ..Preferences::default()
            }
        ),
        Err(PreferencesError::InvalidCandidateWindowStyle)
    ));
    assert_eq!(
        PreferencesError::InvalidCandidateWindowStyle.to_string(),
        "candidate window scale must be 50-200%, opacity 50-100%, corner radius 0-32 and padding 0-16"
    );
}

/// 卡片内边距缺省时不写进文档，旧版设置应用读得了；0 是真的选择，不能和「跟随皮肤」混成一样；超过 16 拒收（#7316）。
#[test]
fn candidate_padding_is_absent_by_default_and_bounded() {
    let preferences = Preferences::default();
    assert_eq!(preferences.candidate_padding, None);
    let value = serde_json::to_value(&preferences).unwrap();
    assert!(value.get("candidate_padding").is_none());

    for padding in [0, 2, 16] {
        let padded = Preferences {
            candidate_padding: Some(padding),
            ..Preferences::default()
        };
        assert!(padded.validate().is_ok(), "{padding} was refused");
        let value = serde_json::to_value(&padded).unwrap();
        assert_eq!(value["candidate_padding"], padding);
        let decoded: Preferences = serde_json::from_value(value).unwrap();
        assert_eq!(decoded, padded);
    }

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let padded = Preferences {
        candidate_padding: Some(3),
        ..Preferences::default()
    };
    store.save(0, padded.clone()).unwrap();
    assert_eq!(store.load().unwrap().preferences, padded);
    assert!(matches!(
        store.save(
            1,
            Preferences {
                candidate_padding: Some(17),
                ..Preferences::default()
            }
        ),
        Err(PreferencesError::InvalidCandidateWindowStyle)
    ));
}

#[test]
fn candidate_item_geometry_preserves_absence_zero_bounds_and_store_roundtrip() {
    let base = serde_json::to_value(Preferences::default()).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut generation = 0;
    for (key, maximum) in [
        ("candidate_padding_horizontal", 16),
        ("candidate_padding_vertical", 16),
        ("candidate_item_padding_horizontal", 16),
        ("candidate_item_padding_vertical", 16),
        ("candidate_item_corner_radius", 32),
        ("candidate_item_spacing", 16),
    ] {
        assert!(base.get(key).is_none());
        for number in [0, maximum] {
            let mut value = base.clone();
            value[key] = number.into();
            value["candidate_padding"] = 7.into();
            let preferences: Preferences = serde_json::from_value(value).unwrap();
            assert!(preferences.validate().is_ok());
            assert_eq!(serde_json::to_value(&preferences).unwrap()[key], number);
            store.save(generation, preferences.clone()).unwrap();
            generation += 1;
            assert_eq!(store.load().unwrap().preferences, preferences);
        }
        for invalid in [
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!(true),
        ] {
            let mut value = base.clone();
            value[key] = invalid;
            assert!(serde_json::from_value::<Preferences>(value).is_err());
        }
        let mut value = base.clone();
        value[key] = (maximum + 1).into();
        let invalid: Preferences = serde_json::from_value(value).unwrap();
        assert!(matches!(
            invalid.validate(),
            Err(PreferencesError::InvalidCandidateWindowStyle)
        ));
        let mut value = base.clone();
        value[key] = serde_json::Value::Null;
        let cleared: Preferences = serde_json::from_value(value).unwrap();
        assert!(serde_json::to_value(cleared).unwrap().get(key).is_none());
    }
}

fn wubi_edition() -> &'static crate::edition::Edition {
    crate::edition::Edition::by_id("wubi").unwrap()
}

/// full 的版本默认值就是 `Default`，所以 full 的偏好、首启快照和触屏方案都与引入版本之前相同。
#[test]
fn full_edition_defaults_are_the_defaults() {
    let full = crate::edition::Edition::full();
    assert_eq!(Preferences::for_edition(full), Preferences::default());
    assert_eq!(
        PreferencesSnapshot::for_edition(full),
        PreferencesSnapshot::default()
    );
    assert_eq!(
        TouchKeyboardSchemePreferences::for_edition(full),
        TouchKeyboardSchemePreferences::default()
    );
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::for_edition(directory.path(), full);
    assert!(store.edition().is_full());
    assert_eq!(store.load().unwrap(), PreferencesSnapshot::default());
    assert!(PreferencesStore::new(directory.path()).edition().is_full());
}

/// 五笔版第一次运行、还没有偏好文件时：方案是五笔，切回中文也回到五笔，混拼默认打开，触屏键盘只有五笔和手写。
#[test]
fn wubi_edition_first_run_defaults_to_wubi_with_mixed_pinyin_on() {
    let wubi = wubi_edition();
    let defaults = Preferences::for_edition(wubi);
    assert_eq!(defaults.scheme, InputScheme::Wubi);
    assert_eq!(defaults.last_chinese_scheme, Some(ChineseScheme::Wubi));
    assert!(defaults.wubi_mixed_pinyin);
    assert_eq!(
        defaults.touch_keyboard_schemes.enabled,
        [TouchKeyboardScheme::Wubi, TouchKeyboardScheme::Handwriting]
            .into_iter()
            .collect()
    );
    assert_eq!(
        defaults.touch_keyboard_schemes.first_enabled(),
        TouchKeyboardScheme::Wubi
    );
    defaults.validate().unwrap();
    // 其余设置与 full 相同。
    assert_eq!(
        Preferences {
            scheme: InputScheme::Quanpin,
            last_chinese_scheme: None,
            wubi_mixed_pinyin: false,
            touch_keyboard_schemes: TouchKeyboardSchemePreferences::default(),
            ..defaults.clone()
        },
        Preferences::default()
    );

    // 没有偏好文件这条路：读到的是五笔版的默认值，修订号 0。
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::for_edition(directory.path(), wubi);
    assert_eq!(store.edition().id, "wubi");
    let first = store.load().unwrap();
    assert_eq!(first.revision, 0);
    assert_eq!(first.preferences, defaults);
    assert!(
        store
            .try_load()
            .unwrap()
            .unwrap()
            .preferences
            .wubi_mixed_pinyin
    );

    // 收窄后的触屏方案不等于 full 的缺省，所以会被写进文件，读回来不变。
    let saved = store.save(0, first.preferences.clone()).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(
        PreferencesStore::new(directory.path()).load().unwrap(),
        saved
    );

    // 混拼只是默认打开，用户可以关掉，关掉后保持关闭。
    let mut off = saved.preferences.clone();
    off.wubi_mixed_pinyin = false;
    store.save(saved.revision, off).unwrap();
    assert!(!store.load().unwrap().preferences.wubi_mixed_pinyin);
}

/// 拼音版只有全拼和双拼：默认方案仍是全拼，触屏键盘去掉五笔、日文和韩文入口，保留手写。
#[test]
fn pinyin_edition_narrows_the_touch_keyboard() {
    let pinyin = crate::edition::Edition::by_id("pinyin").unwrap();
    let defaults = Preferences::for_edition(pinyin);
    assert_eq!(defaults.scheme, InputScheme::Quanpin);
    assert_eq!(defaults.last_chinese_scheme, None);
    assert!(!defaults.wubi_mixed_pinyin);
    assert_eq!(
        defaults.touch_keyboard_schemes.enabled,
        [
            TouchKeyboardScheme::Quanpin,
            TouchKeyboardScheme::NineKey,
            TouchKeyboardScheme::Xiaohe,
            TouchKeyboardScheme::Ziranma,
            TouchKeyboardScheme::Microsoft,
            TouchKeyboardScheme::Shoudao,
            TouchKeyboardScheme::Handwriting,
        ]
        .into_iter()
        .collect()
    );
}

/// 五笔版恢复默认设置和修复损坏的偏好文件时，回到的都是五笔版的默认值。
#[test]
fn wubi_edition_restores_and_recovers_to_its_own_defaults() {
    let wubi = wubi_edition();
    let mut edited = Preferences::for_edition(wubi);
    edited.wubi_mixed_pinyin = false;
    edited.candidate_page_size = 7;
    let restored = edited.restored_to_defaults_for(wubi);
    assert_eq!(restored, Preferences::for_edition(wubi));
    assert_eq!(edited.restored_to_defaults(), Preferences::default());

    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::for_edition(directory.path(), wubi);
    fs::write(
        directory.path().join("preferences.json"),
        br#"{"format_version":1,"revision":4,"preferences":{"scheme":"#,
    )
    .unwrap();
    let (snapshot, _, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(!salvaged);
    assert_eq!(snapshot.preferences, Preferences::for_edition(wubi));

    // 能挽救的设置照常保留，没有的设置取五笔版的默认值。
    fs::write(
        directory.path().join("preferences.json"),
        br#"{"format_version":1,"revision":9,"preferences":{"candidate_page_size":6,"unknown_field":1}}"#,
    )
    .unwrap();
    let (snapshot, _, salvaged) = expect_recovered(store.recover().unwrap());
    assert!(salvaged);
    assert_eq!(snapshot.preferences.candidate_page_size, 6);
    assert_eq!(snapshot.preferences.scheme, InputScheme::Wubi);
    assert!(snapshot.preferences.wubi_mixed_pinyin);
}

/// 只拿到目录的存储（各平台的 C ABI、设置应用）按目录里的版本记录取默认值；没有记录、记录认不出时是 full，行为与引入版本之前相同。
#[test]
fn a_store_built_from_a_directory_follows_the_recorded_edition() {
    let wubi = wubi_edition();
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    assert!(store.edition().is_full());
    assert_eq!(store.load().unwrap(), PreferencesSnapshot::default());

    wubi.record_in(directory.path()).unwrap();
    assert_eq!(
        fs::read_to_string(
            directory
                .path()
                .join(crate::edition::Edition::STATE_RECORD_FILE)
        )
        .unwrap(),
        "wubi\n"
    );
    assert_eq!(store.edition().id, "wubi");
    assert_eq!(
        store.load().unwrap(),
        PreferencesSnapshot::for_edition(wubi)
    );
    fs::write(
        directory.path().join("preferences.json"),
        br#"{"format_version":1,"revision":4,"preferences":{"scheme":"#,
    )
    .unwrap();
    let (snapshot, _, _) = expect_recovered(store.recover().unwrap());
    assert_eq!(snapshot.preferences, Preferences::for_edition(wubi));
    // 构造时指定的版本优先于记录。
    assert!(
        PreferencesStore::for_edition(directory.path(), crate::edition::Edition::full())
            .edition()
            .is_full()
    );

    // full 准备同一个目录时删掉记录。
    crate::edition::Edition::full()
        .record_in(directory.path())
        .unwrap();
    assert!(!directory
        .path()
        .join(crate::edition::Edition::STATE_RECORD_FILE)
        .exists());
    assert!(store.edition().is_full());
    crate::edition::Edition::full()
        .record_in(directory.path())
        .unwrap();

    // 认不出的记录按 full 处理。
    fs::write(
        directory
            .path()
            .join(crate::edition::Edition::STATE_RECORD_FILE),
        "future\n",
    )
    .unwrap();
    assert!(store.edition().is_full());
}

/// 日文、越南文和藏文版第一次运行时：方案就是本版本唯一的方案，没有可回到的中文方案，其余偏好与 full 相同；触屏键盘只有本方案的入口（越南文、藏文在 full 里默认停用的入口也启用），没有手写（手写识别器只认汉字），选中留空，第一次打开键盘就是本方案。
#[test]
fn language_editions_first_run_defaults_to_their_own_scheme() {
    for (id, scheme, touch) in [
        (
            "japanese",
            InputScheme::Japanese,
            vec![
                TouchKeyboardScheme::JapaneseNineKey,
                TouchKeyboardScheme::Japanese,
            ],
        ),
        (
            "vietnamese",
            InputScheme::Vietnamese,
            vec![TouchKeyboardScheme::Vietnamese],
        ),
        (
            "tibetan",
            InputScheme::Tibetan,
            vec![TouchKeyboardScheme::Tibetan],
        ),
    ] {
        let edition = crate::edition::Edition::by_id(id).unwrap();
        let defaults = Preferences::for_edition(edition);
        assert_eq!(defaults.scheme, scheme, "{id}");
        assert_eq!(defaults.last_chinese_scheme, None, "{id}");
        assert!(!defaults.wubi_mixed_pinyin, "{id}");
        let expected: std::collections::BTreeSet<_> = touch.into_iter().collect();
        assert_eq!(defaults.touch_keyboard_schemes.enabled, expected, "{id}");
        assert_eq!(defaults.touch_keyboard_schemes.selected, None, "{id}");
        defaults.validate().unwrap();
        assert_eq!(
            Preferences {
                scheme: InputScheme::Quanpin,
                touch_keyboard_schemes: TouchKeyboardSchemePreferences::default(),
                ..defaults.clone()
            },
            Preferences::default(),
            "{id}"
        );

        // 没有偏好文件时读到的就是这份默认值；写进文件再读回来不变。
        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::for_edition(directory.path(), edition);
        assert_eq!(store.edition().id, id);
        let first = store.load().unwrap();
        assert_eq!(first.revision, 0);
        assert_eq!(first.preferences, defaults, "{id}");
        let saved = store.save(0, defaults.clone()).unwrap();
        assert_eq!(saved.preferences, defaults, "{id}");
        assert_eq!(store.load().unwrap().preferences, defaults, "{id}");
    }
}

#[test]
fn restoring_defaults_in_the_store_is_one_compare_and_swap() {
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut edited = Preferences {
        candidate_page_size: 7,
        ..Preferences::default()
    };
    edited.voice_input.asr_token = "fixture-asr-token".into();
    edited.fuzzy_pinyin.seeded = true;
    let saved = store.save(0, edited).unwrap();
    assert!(matches!(
        store.restore_defaults(saved.revision + 1),
        Err(PreferencesError::Conflict)
    ));
    assert_eq!(store.load().unwrap(), saved);
    let restored = store.restore_defaults(saved.revision).unwrap();
    assert_eq!(restored.revision, saved.revision + 1);
    assert_eq!(restored.preferences.candidate_page_size, 6);
    assert_eq!(
        restored.preferences.voice_input.asr_token,
        "fixture-asr-token"
    );
    assert!(restored.preferences.fuzzy_pinyin.seeded);
    assert_eq!(store.load().unwrap(), restored);
}

#[test]
fn android_portrait_split_defaults_and_validates_without_changing_legacy_documents() {
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document
        .as_object_mut()
        .unwrap()
        .remove("touch_keyboard_split_portrait");
    let legacy = document.to_string();
    let loaded: Preferences = serde_json::from_str(&legacy).unwrap();
    assert_eq!(
        serde_json::to_value(loaded).unwrap()["touch_keyboard_split_portrait"],
        false
    );
    for enabled in [false, true] {
        document["touch_keyboard_split_portrait"] = enabled.into();
        let loaded: Preferences = serde_json::from_value(document.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(loaded).unwrap()["touch_keyboard_split_portrait"],
            enabled
        );
    }
    for invalid in [
        serde_json::json!("true"),
        serde_json::json!(1),
        serde_json::Value::Null,
    ] {
        document["touch_keyboard_split_portrait"] = invalid;
        assert!(serde_json::from_value::<Preferences>(document.clone()).is_err());
    }
}

/// 偏好里键名像凭据、但确实不是凭据的字段。新字段名含 token|key|secret|password 时，要么进 `Preferences::credential_slots`，要么在这里说明它不是凭据。
const NOT_CREDENTIALS: &[&str] = &[
    "custom_theme.keyboard",
    "custom_theme.keyboard.keyBackground",
    "custom_theme.keyboard.keyForeground",
    "custom_theme.keyboard.keyMaterial",
    "custom_theme.keyboard.keyOpacity",
    "custom_theme.keyboard.keyShape",
    "floating_toolbar.screen_keyboard",
    "keybindings",
    // 辅助码按键是普通键位偏好，不是服务凭据。
    "quanpin_helpcode.trigger_key",
    "shuangpin_helpcode.trigger_key",
    "screen_keyboard_theme",
    "second_third_candidate.keys",
    "sentence_association.neural_keyboard",
    "touch_key_spacing_tenths",
    "touch_keyboard_height_adjustment",
    "touch_keyboard_layout",
    "touch_keyboard_schemes",
    "touch_keyboard_split_portrait",
    "touch_number_keypad_order",
    "touch_shuangpin_key_hints",
    "touch_twenty_six_key_number_layout",
    "voice_input.hotkey_ctrl_f9",
    "voice_input.hotkey_ctrl_win",
    "voice_input.hotkey_hold_space_lock",
    "voice_input.hotkey_ralt",
    "voice_input.hotkey_rctrl_ralt",
    "word_character.keys",
];

fn credential_like(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["token", "key", "secret", "password"]
        .iter()
        .any(|pattern| name.contains(pattern))
}

fn credential_like_paths(value: &serde_json::Value, prefix: &str, into: &mut Vec<String>) {
    if let serde_json::Value::Object(object) = value {
        for (key, child) in object {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            if credential_like(key) {
                into.push(path.clone());
            }
            credential_like_paths(child, &path, into);
        }
    }
}

fn with_every_credential(mut preferences: Preferences) -> Preferences {
    for (_, slot) in preferences.credential_slots() {
        match slot {
            CredentialSlot::Text(text) => *text = "secret-value".into(),
            CredentialSlot::Map(map) => {
                map.insert("provider".into(), "secret-value".into());
            }
        }
    }
    preferences
}

#[test]
fn every_credential_like_preference_field_is_listed() {
    let mut preferences = Preferences::default();
    let credentials: Vec<&str> = preferences
        .credential_slots()
        .iter()
        .map(|(path, _)| *path)
        .collect();
    // 默认值里没有键盘皮肤（`None`），填上一份，扫描才覆盖得到这一段字段。
    let mut scanned = Preferences::default();
    scanned.custom_theme.keyboard = Some(TouchKeyboardSkinDesign::mint_morning());
    // 二三候选键位关着时不写进文档，打开它，扫描才看得到 `keys`。
    scanned.second_third_candidate.enabled = true;
    let document = serde_json::to_value(with_every_credential(scanned)).unwrap();
    let mut paths = Vec::new();
    credential_like_paths(&document, "", &mut paths);
    let unlisted: Vec<_> = paths
        .iter()
        .filter(|path| {
            !credentials.contains(&path.as_str())
                && !NOT_CREDENTIALS.contains(&path.as_str())
                && !credentials
                    .iter()
                    .any(|credential| path.starts_with(&format!("{credential}.")))
        })
        .collect();
    assert!(
        unlisted.is_empty(),
        "credential-like preference fields neither in credential_slots nor NOT_CREDENTIALS: {unlisted:?}"
    );
    for path in &credentials {
        let mut cursor = &document;
        for segment in path.split('.') {
            cursor = &cursor[segment];
        }
        assert!(!cursor.is_null(), "{path} is not a preference field");
    }
}

#[test]
fn diagnostic_snapshot_redacts_every_credential() {
    let preferences = with_every_credential(Preferences::default());
    let redacted = preferences.redacted_for_diagnostics();
    let text = redacted.to_string();
    assert!(!text.contains("secret-value"), "{text}");
    let mut copy = preferences.clone();
    for (path, _) in copy.credential_slots() {
        let mut cursor = &redacted;
        for segment in path.split('.') {
            cursor = &cursor[segment];
        }
        assert_eq!(cursor, REDACTED, "{path}");
    }
    fn check(value: &serde_json::Value) {
        if let serde_json::Value::Object(object) = value {
            for (key, child) in object {
                if is_sensitive_key(key) {
                    assert_eq!(child, REDACTED, "{key}");
                } else {
                    check(child);
                }
            }
        }
    }
    check(&redacted);
    // 非凭据的设置照常保留，快照才有用。
    assert_eq!(
        redacted["candidate_page_size"],
        serde_json::json!(preferences.candidate_page_size)
    );
}

#[test]
fn diagnostic_snapshot_strips_secrets_from_endpoints() {
    let mut preferences = Preferences::default();
    preferences.voice_input.asr_endpoint =
        "wss://user:pass@asr.example.com:8443/v1/stream?token=secret-value#frag".into();
    preferences.voice_input.polish_endpoint =
        "https://polish.example.com/chat?key=secret-value".into();
    preferences.ai_assistant.endpoint = "https://secret-value@ai.example.com/v1".into();
    preferences.custom_translation.endpoint = "not a url secret-value".into();
    preferences.cloud_clipboard_relay.url =
        "https://relay.example.com/msime?token=secret-value".into();
    let redacted = preferences.redacted_for_diagnostics();
    assert!(!redacted.to_string().contains("secret-value"), "{redacted}");
    assert_eq!(
        redacted["voice_input"]["asr_endpoint"],
        "wss://asr.example.com:8443/v1/stream"
    );
    assert_eq!(
        redacted["voice_input"]["polish_endpoint"],
        "https://polish.example.com/chat"
    );
    assert_eq!(
        redacted["ai_assistant"]["endpoint"],
        "https://ai.example.com/v1"
    );
    assert_eq!(redacted["custom_translation"]["endpoint"], REDACTED);
    assert_eq!(
        redacted["cloud_clipboard_relay"]["url"],
        "https://relay.example.com/msime"
    );

    // 没填的地址照样是空的。
    preferences.custom_translation.endpoint.clear();
    assert_eq!(
        preferences.redacted_for_diagnostics()["custom_translation"]["endpoint"],
        ""
    );
}

#[test]
fn restoring_defaults_keeps_exactly_the_listed_credentials() {
    let preferences = with_every_credential(Preferences::default());
    let mut restored = preferences.restored_to_defaults();
    let mut original = preferences.clone();
    for ((path, before), (_, after)) in original
        .credential_slots()
        .into_iter()
        .zip(restored.credential_slots())
    {
        match (before, after) {
            (CredentialSlot::Text(before), CredentialSlot::Text(after)) => {
                assert_eq!(before, after, "{path}")
            }
            (CredentialSlot::Map(before), CredentialSlot::Map(after)) => {
                assert_eq!(before, after, "{path}")
            }
            _ => panic!("{path} changed type"),
        }
    }
}

/// 中继的地址和令牌要么都空，要么都合规；没配置时不写进文档，旧版本读得了。
#[test]
fn cloud_clipboard_relay_is_both_or_nothing_and_left_out_until_configured() {
    let defaults = Preferences::default();
    let document = serde_json::to_value(&defaults).unwrap();
    assert!(document.get("cloud_clipboard_relay").is_none());
    assert!(!defaults.cloud_clipboard_relay.is_configured());

    let configured = |url: &str, token: &str| {
        let mut preferences = Preferences::default();
        preferences.cloud_clipboard_relay.url = url.into();
        preferences.cloud_clipboard_relay.token = token.into();
        preferences
    };
    let token = "synthetic-relay-token";
    for (url, token, valid) in [
        ("https://relay.example.test", token, true),
        ("https://relay.example.test/msime/", token, true),
        ("http://127.0.0.1:8787", token, true),
        ("http://localhost:8787/msime", token, true),
        ("http://relay.example.test", token, false),
        ("https://relay.example.test/?q=1", token, false),
        ("https://user:secret@relay.example.test", token, false),
        ("https://relay.example.test#frag", token, false),
        ("ftp://relay.example.test", token, false),
        ("https://relay.example.test", "", false),
        ("https://relay.example.test", "too-short", false),
        ("https://relay.example.test", "has a space in it", false),
        ("", token, false),
    ] {
        let preferences = configured(url, token);
        assert_eq!(
            preferences.validate().is_ok(),
            valid,
            "{url} {token}: {:?}",
            preferences.validate()
        );
        if !valid {
            assert!(matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidCloudClipboardRelay)
            ));
        }
    }

    let preferences = configured("https://relay.example.test", token);
    let text = serde_json::to_string(&preferences).unwrap();
    let back: Preferences = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back.cloud_clipboard_relay,
        preferences.cloud_clipboard_relay
    );
    assert!(back.cloud_clipboard_relay.is_configured());
}

// 引号样式与方头括号样式缺省是弯引号、实心，等于缺省时不写进文档；三种引号样式和两种括号样式都能保存读回，不认识的取值让整份文档解析失败。
#[test]
fn punctuation_styles_default_omitted_roundtrip_and_reject_unknown_values() {
    let defaults = Preferences::default();
    assert_eq!(defaults.quote_style, QuoteStyle::Curly);
    assert_eq!(defaults.bracket_style, BracketStyle::Solid);
    let written = serde_json::to_value(&defaults).unwrap();
    assert!(written.get("quote_style").is_none());
    assert!(written.get("bracket_style").is_none());
    let read = serde_json::from_value::<Preferences>(written.clone()).unwrap();
    assert_eq!(read.quote_style, QuoteStyle::Curly);
    assert_eq!(read.bracket_style, BracketStyle::Solid);

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let mut revision = 0;
    for (quote_style, quote_value) in [
        (QuoteStyle::Corner, "corner"),
        (QuoteStyle::CornerWhenTraditional, "corner_when_traditional"),
    ] {
        let saved = store
            .save(
                revision,
                Preferences {
                    quote_style,
                    bracket_style: BracketStyle::Hollow,
                    ..Preferences::default()
                },
            )
            .unwrap();
        revision = saved.revision;
        let on_disk: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(on_disk["preferences"]["quote_style"], quote_value);
        assert_eq!(on_disk["preferences"]["bracket_style"], "hollow");
        assert_eq!(store.load().unwrap(), saved);
    }

    for (key, value) in [("quote_style", "semantic"), ("bracket_style", "round")] {
        let mut document = written.clone();
        document[key] = serde_json::Value::String(value.into());
        assert!(
            serde_json::from_value::<Preferences>(document).is_err(),
            "{key}={value}"
        );
    }
}

// 顿号自定义（Windows v0.11.1 `input.enumeration_comma_*`）：`\` 默认开、`/` 默认关，等于缺省时不写进文档，没有这个键的旧文档按缺省读；两个键各自的取值都能保存读回。
#[test]
fn enumeration_comma_keys_default_omitted_and_roundtrip() {
    let defaults = Preferences::default();
    assert!(defaults.enumeration_comma_backslash);
    assert!(!defaults.enumeration_comma_slash);
    let written = serde_json::to_value(&defaults).unwrap();
    assert!(written.get("enumeration_comma_backslash").is_none());
    assert!(written.get("enumeration_comma_slash").is_none());
    let read = serde_json::from_value::<Preferences>(written).unwrap();
    assert!(read.enumeration_comma_backslash);
    assert!(!read.enumeration_comma_slash);

    // 「两个都关」的文档照样读得进来（deny_unknown_fields 不拦），归一在 `punctuation_marks`。
    let mut document = serde_json::to_value(&defaults).unwrap();
    document["enumeration_comma_backslash"] = serde_json::Value::Bool(false);
    document["enumeration_comma_slash"] = serde_json::Value::Bool(false);
    let parsed: Preferences = serde_json::from_value(document).unwrap();
    assert!(!parsed.enumeration_comma_backslash);
    assert!(!parsed.enumeration_comma_slash);

    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store
        .save(
            0,
            Preferences {
                enumeration_comma_backslash: false,
                enumeration_comma_slash: true,
                ..Preferences::default()
            },
        )
        .unwrap();
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(on_disk["preferences"]["enumeration_comma_backslash"], false);
    assert_eq!(on_disk["preferences"]["enumeration_comma_slash"], true);
    assert_eq!(store.load().unwrap(), saved);
}

/// `value` 里每一个对象（数组元素里的除外）的键路径。
fn object_paths(value: &serde_json::Value, path: &mut Vec<String>, found: &mut Vec<Vec<String>>) {
    let serde_json::Value::Object(object) = value else {
        return;
    };
    found.push(path.clone());
    for (key, child) in object {
        path.push(key.clone());
        object_paths(child, path, found);
        path.pop();
    }
}

fn at<'a>(value: &'a serde_json::Value, path: &[String]) -> Option<&'a serde_json::Value> {
    path.iter()
        .try_fold(value, |node, key| node.get(key.as_str()))
}

fn at_mut<'a>(value: &'a mut serde_json::Value, path: &[String]) -> &'a mut serde_json::Value {
    path.iter().fold(value, |node, key| &mut node[key.as_str()])
}

/// macOS 的输入法由 Sparkle 单独更新，设置应用和 `msime-mcp` 不跟着变：更新版本写进偏好文档的键，旧版本要读得了，保存时也不能抹掉。旧版本曾因 `deny_unknown_fields` 整份拒读，设置页改什么都存不下来。
#[test]
fn settings_from_a_newer_build_are_read_and_kept_through_a_save() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let saved = store.save(0, Preferences::default()).unwrap();
    let mut document = serde_json::to_value(&saved).unwrap();

    // 默认文档里的每个对象逐个试一遍：结构要把新键当作未知键收下；以用户数据为键的 map（凭据、应用例外、键位表）把它当成一条数据或因值的类型不符拒绝，跳过。任何一个结构说 unknown field，就是它还在拒绝更新版本的文档。
    let mut objects = Vec::new();
    object_paths(&document["preferences"], &mut Vec::new(), &mut objects);
    let mut planted = Vec::new();
    for object in objects {
        let mut trial = document["preferences"].clone();
        let mut key = object.clone();
        key.push("from_a_newer_build".into());
        at_mut(&mut trial, &object)["from_a_newer_build"] = "kept".into();
        match deserialize_keeping_unknown::<Preferences>(&trial, &[]) {
            Ok((_, unknown)) if unknown.iter().any(|field| field.path == key) => planted.push(key),
            Ok(_) => {}
            Err(error) => assert!(
                !error.to_string().contains("unknown field"),
                "{object:?}: {error}"
            ),
        }
    }
    assert!(planted.len() > 20, "{planted:?}");
    for (index, key) in planted.iter().enumerate() {
        let (field, parent) = key.split_last().unwrap();
        at_mut(&mut document["preferences"], parent)[field.as_str()] =
            serde_json::json!({"kept": index});
    }
    fs::write(store.path(), serde_json::to_vec(&document).unwrap()).unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded.preferences, saved.preferences);

    let mut changed = loaded.preferences.clone();
    changed.candidate_page_size = 9;
    let written = store.save(loaded.revision, changed).unwrap();
    assert_eq!(written.preferences.candidate_page_size, 9);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(on_disk["preferences"]["candidate_page_size"], 9);
    for key in &planted {
        assert_eq!(
            at(&on_disk["preferences"], key),
            at(&document["preferences"], key),
            "{key:?}"
        );
    }
    assert_eq!(store.load().unwrap(), written);

    // 恢复默认设置是整份回到默认，更新版本写下的键一并清掉。
    let restored = store.restore_defaults(written.revision).unwrap();
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
    assert_eq!(
        on_disk["preferences"],
        serde_json::to_value(&restored.preferences).unwrap()
    );
}

/// 未知键的父对象这次因为取默认值没有写出时不重建它：只凭一个未知键写出半个已知对象，读它的版本会缺字段。父对象写出时照常放回，本版本已有同名键时以本版本为准。
#[test]
fn an_unknown_key_goes_back_only_into_a_written_section() {
    let unknown = vec![UnknownPreferenceField {
        path: vec!["section".into(), "from_a_newer_build".into()],
        value: true.into(),
    }];
    let mut preferences = serde_json::json!({});
    restore_unknown_fields(&mut preferences, &unknown);
    assert_eq!(preferences, serde_json::json!({}));
    let mut preferences = serde_json::json!({"section": {"from_a_newer_build": false}});
    restore_unknown_fields(&mut preferences, &unknown);
    assert_eq!(preferences["section"]["from_a_newer_build"], false);
    let mut preferences = serde_json::json!({"section": {}});
    restore_unknown_fields(&mut preferences, &unknown);
    assert_eq!(preferences["section"]["from_a_newer_build"], true);
}

#[cfg(unix)]
#[test]
fn finished_scope_releases_inherited_lock() {
    let root = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(root.path());
    let held = store.lock().unwrap();
    let inherited = crate::file_lock::testing::duplicate(&held);
    crate::file_lock::testing::assert_finished_scope_releases_lock(
        held,
        inherited,
        &root.path().join("preferences.lock"),
    );
}

#[test]
fn english_online_is_an_explicit_local_opt_in() {
    let defaults = Preferences::default();
    assert!(!defaults.english_online_candidates);
    let mut legacy = serde_json::to_value(&defaults).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("english_online_candidates");
    assert!(
        !serde_json::from_value::<Preferences>(legacy)
            .unwrap()
            .english_online_candidates
    );
    let enabled = Preferences {
        english_online_candidates: true,
        ..defaults
    };
    assert!(
        serde_json::from_value::<Preferences>(serde_json::to_value(&enabled).unwrap())
            .unwrap()
            .english_online_candidates
    );
    let exported = crate::settings_document::export_document(&enabled).unwrap();
    assert!(!exported.contains("english_online_candidates"));
}

/// 网络代理设置：出厂值与 Windows 正式版相同，等于默认值时不写进文档，旧文档缺这一节时也能读。
#[test]
fn network_proxy_defaults_match_the_windows_release_and_stay_out_of_the_document() {
    let defaults = Preferences::default();
    assert_eq!(
        defaults.network.proxy_mode,
        crate::network::ProxyMode::System
    );
    assert!(defaults.network.proxy_server.is_empty());
    let document = serde_json::to_value(&defaults).unwrap();
    assert!(document.get("network").is_none());

    // 没有这个键的旧文档按 system 读出来。
    let restored: Preferences = serde_json::from_value(document).unwrap();
    assert_eq!(
        restored.network,
        crate::network::NetworkPreferences::default()
    );

    // custom 模式里留空服务器时文档里写 `custom`，生效模式回退 system。
    let mut empty_custom = Preferences::default();
    empty_custom.network.proxy_mode = crate::network::ProxyMode::Custom;
    let document = serde_json::to_value(&empty_custom).unwrap();
    assert_eq!(document["network"]["proxy_mode"], "custom");
    assert_eq!(
        empty_custom.network.effective_mode(),
        crate::network::ProxyMode::System
    );

    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let saved = store.save(0, empty_custom.clone()).unwrap();
    assert_eq!(store.load().unwrap(), saved);
    assert_eq!(saved.preferences.network, empty_custom.network);
}

#[test]
fn network_proxy_server_is_validated_and_restored_with_the_services() {
    let valid = |server: &str| {
        let mut preferences = Preferences::default();
        preferences.network.proxy_mode = crate::network::ProxyMode::Custom;
        preferences.network.proxy_server = server.into();
        preferences.validate()
    };
    assert!(valid("127.0.0.1:7890").is_ok());
    assert!(valid("http://127.0.0.1:7890").is_ok());
    assert!(valid("").is_ok());
    assert!(matches!(
        valid("127.0.0.1"),
        Err(PreferencesError::InvalidNetwork)
    ));
    assert!(matches!(
        valid("http://user:secret@127.0.0.1:7890"),
        Err(PreferencesError::InvalidNetwork)
    ));

    // 恢复默认设置留下代理地址：服务凭据还在，出口跟着一起留下，否则全部断掉。
    let mut configured = Preferences::default();
    configured.network.proxy_mode = crate::network::ProxyMode::Custom;
    configured.network.proxy_server = "127.0.0.1:7890".into();
    assert_eq!(
        configured.restored_to_defaults().network,
        configured.network
    );
}

#[test]
fn helpcode_trigger_keys_roundtrip_independently_and_validate() {
    let old = serde_json::json!({"enabled": true, "schema": "ziranma"});
    let old: HelpcodePreferences = serde_json::from_value(old).unwrap();
    assert!(old.trigger_key.is_empty());
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    for (revision, key) in ["", ";", "/", "\\", "`", "="].into_iter().enumerate() {
        let mut preferences = Preferences::default();
        preferences.quanpin_helpcode.trigger_key = key.into();
        preferences.shuangpin_helpcode.trigger_key = "/".into();
        preferences.validate().unwrap();
        store.save(revision as u64, preferences).unwrap();
        let loaded = PreferencesStore::new(directory.path())
            .load()
            .unwrap()
            .preferences;
        assert_eq!(loaded.quanpin_helpcode.trigger_key, key);
        assert_eq!(loaded.shuangpin_helpcode.trigger_key, "/");
    }
    for key in ["a", "1", " ", "'", "Shift", "Ctrl+;", ";;", "；"] {
        for shuangpin in [false, true] {
            let mut preferences = Preferences::default();
            let family = if shuangpin {
                &mut preferences.shuangpin_helpcode
            } else {
                &mut preferences.quanpin_helpcode
            };
            family.trigger_key = key.into();
            assert!(matches!(
                preferences.validate(),
                Err(PreferencesError::InvalidHelpcodeTriggerKey)
            ));
        }
    }
}

#[test]
fn sentence_collocation_tuning_defaults_match_windows_factory() {
    let association = Preferences::default().sentence_association;
    assert_eq!(association.sentence_collocation_weight, 1.0);
    assert_eq!(association.sentence_collocation_max_length, 6);
    assert_eq!(association.sentence_collocation_min_length, 2);
    assert_eq!(association.sentence_collocation_penalty, -14.0);
    assert_eq!(association.sentence_collocation_non_penalty, -6.0);
    assert_eq!(association.sentence_collocation_weak_penalty, -100.0);
    assert_eq!(association.sentence_collocation_rear_penalty, -18.0);
}

#[test]
fn sentence_collocation_tuning_serializes_only_when_not_factory() {
    // 全出厂值：七个键一个都不写进文档（与 quote_style 的约定一致）。
    let defaults = serde_json::to_value(Preferences::default()).unwrap();
    let association = defaults["sentence_association"].as_object().unwrap();
    for key in [
        "sentence_collocation_weight",
        "sentence_collocation_max_length",
        "sentence_collocation_min_length",
        "sentence_collocation_penalty",
        "sentence_collocation_non_penalty",
        "sentence_collocation_weak_penalty",
        "sentence_collocation_rear_penalty",
    ] {
        assert!(
            !association.contains_key(key),
            "{key} 等于出厂值不该写进文档"
        );
    }

    // 改过才写，且写出的值能原样读回。
    let mut tuned = Preferences::default();
    tuned.sentence_association.sentence_collocation_weight = 0.0;
    tuned.sentence_association.sentence_collocation_penalty = -30.0;
    tuned.sentence_association.sentence_collocation_max_length = 4;
    let value = serde_json::to_value(&tuned).unwrap();
    let association = &value["sentence_association"];
    assert_eq!(
        association["sentence_collocation_weight"],
        serde_json::json!(0.0)
    );
    assert_eq!(
        association["sentence_collocation_penalty"],
        serde_json::json!(-30.0)
    );
    assert_eq!(
        association["sentence_collocation_max_length"],
        serde_json::json!(4)
    );
    assert!(!association
        .as_object()
        .unwrap()
        .contains_key("sentence_collocation_min_length"));
    let restored: Preferences = serde_json::from_value(value).unwrap();
    assert_eq!(restored.sentence_association, tuned.sentence_association);
}

/// 与 Windows `value_or` 同口径的逐键回退：类型不符、i32 放不下的整数、非有限值
/// 各自回出厂值，相邻键和文档其余部分照常生效——单个手误不让整份文档不可读。
#[test]
fn sentence_collocation_tuning_falls_back_per_key_on_bad_values() {
    let mut document = serde_json::to_value(Preferences::default()).unwrap();
    document["sentence_association"] = serde_json::json!({
        "word_lattice": false,
        "sentence_collocation_weight": "high",
        "sentence_collocation_max_length": 5_000_000_000i64,
        "sentence_collocation_min_length": true,
        "sentence_collocation_penalty": -20.0,
        "sentence_collocation_non_penalty": {"bad": "object"},
        "sentence_collocation_weak_penalty": [1, 2],
        "sentence_collocation_rear_penalty": null
    });
    let parsed: Preferences = serde_json::from_value(document).unwrap();
    let association = parsed.sentence_association;
    assert!(!association.word_lattice, "相邻的合法键照常生效");
    assert_eq!(association.sentence_collocation_weight, 1.0);
    assert_eq!(association.sentence_collocation_max_length, 6);
    assert_eq!(association.sentence_collocation_min_length, 2);
    assert_eq!(association.sentence_collocation_penalty, -20.0);
    assert_eq!(association.sentence_collocation_non_penalty, -6.0);
    assert_eq!(association.sentence_collocation_weak_penalty, -100.0);
    assert_eq!(association.sentence_collocation_rear_penalty, -18.0);
    parsed.validate().unwrap();
}

/// `1e400` 这种合法 JSON 数能进到 f64 的非有限区；读成偏好必须回出厂值，
/// 不能把 inf/NaN 带进词格打分。
#[test]
fn sentence_collocation_tuning_rejects_non_finite_values() {
    for (key, raw) in [
        ("sentence_collocation_weight", "1e400"),
        ("sentence_collocation_penalty", "-1e400"),
    ] {
        let mut document = serde_json::to_value(Preferences::default()).unwrap();
        document["sentence_association"][key] =
            serde_json::from_str(raw).unwrap_or(serde_json::Value::Null);
        let parsed: Preferences = serde_json::from_value(document).unwrap();
        assert_eq!(
            parsed.sentence_association.sentence_collocation_weight, 1.0,
            "{key}"
        );
        assert_eq!(
            parsed.sentence_association.sentence_collocation_penalty, -14.0,
            "{key}"
        );
    }
}

#[test]
fn cross_commit_spacing_is_independent_and_validates_excluded_apps() {
    let mut preferences = Preferences::default();
    let defaults = serde_json::to_value(&preferences).unwrap();
    assert!(!preferences.cross_commit_spacing);
    assert!(defaults.get("cross_commit_spacing").is_none());
    assert!(defaults.get("cross_commit_spacing_excluded_apps").is_none());
    preferences.cross_commit_spacing = true;
    preferences.cross_commit_spacing_excluded_apps = vec!["app.example.Editor".into()];
    assert!(preferences.validate().is_ok());
    assert!(!preferences.chinese_english_spacing);
    let written = serde_json::to_value(&preferences).unwrap();
    assert_eq!(
        serde_json::from_value::<Preferences>(written).unwrap(),
        preferences
    );
    for invalid in [
        vec!["".into()],
        vec![" app.example.Editor".into()],
        vec!["app/example/Editor".into()],
        vec!["app.example.Editor".into(), "APP.EXAMPLE.EDITOR".into()],
        vec!["x".repeat(65)],
        (0..33)
            .map(|index| format!("app.example.Editor{index}"))
            .collect(),
    ] {
        preferences.cross_commit_spacing_excluded_apps = invalid;
        assert!(matches!(
            preferences.validate(),
            Err(PreferencesError::InvalidCrossCommitSpacingApps)
        ));
    }
}

#[test]
fn text_edit_button_order_round_trips_and_restores_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    let mut preferences = store.load().unwrap().preferences;
    assert_eq!(
        preferences.touch_text_edit_button_order,
        default_text_edit_button_order()
    );
    preferences.touch_text_edit_button_order.reverse();
    let expected = preferences.touch_text_edit_button_order.clone();
    let saved = store.save(0, preferences).unwrap();
    assert_eq!(
        store
            .load()
            .unwrap()
            .preferences
            .touch_text_edit_button_order,
        expected
    );
    assert_eq!(
        store
            .restore_defaults(saved.revision)
            .unwrap()
            .preferences
            .touch_text_edit_button_order,
        default_text_edit_button_order()
    );
    let mut old_document = serde_json::to_value(Preferences::default()).unwrap();
    old_document
        .as_object_mut()
        .unwrap()
        .remove("touch_text_edit_button_order");
    let old: Preferences = serde_json::from_value(old_document).unwrap();
    assert_eq!(
        old.touch_text_edit_button_order,
        default_text_edit_button_order()
    );
}

#[test]
fn text_edit_button_order_rejects_missing_duplicate_and_unknown_actions() {
    let directory = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(directory.path());
    for order in [
        vec![],
        vec![TextEditButton::Copy; 7],
        default_text_edit_button_order()[..6].to_vec(),
    ] {
        let preferences = Preferences {
            touch_text_edit_button_order: order,
            ..Preferences::default()
        };
        assert!(matches!(
            store.save(0, preferences),
            Err(PreferencesError::InvalidTextEditButtonOrder)
        ));
        assert_eq!(store.load().unwrap().revision, 0);
    }
    let mut unknown = serde_json::to_value(Preferences::default()).unwrap();
    unknown["touch_text_edit_button_order"][6] = serde_json::json!("return");
    let error = serde_json::from_value::<Preferences>(unknown).unwrap_err();
    assert!(error.to_string().contains("unknown variant `return`"));
}
