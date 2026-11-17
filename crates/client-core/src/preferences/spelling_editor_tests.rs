use super::*;

#[test]
fn spelling_editor_preserves_legacy_default_and_explicit_choice() {
    assert!(Preferences::default().touch_spelling_editor);
    let mut legacy = serde_json::to_value(PreferencesSnapshot::default()).unwrap();
    legacy["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("touch_spelling_editor");
    let decoded: PreferencesSnapshot = serde_json::from_value(legacy.clone()).unwrap();
    assert!(decoded.preferences.touch_spelling_editor);
    let dir = tempfile::tempdir().unwrap();
    let store = PreferencesStore::new(dir.path());
    let saved = store
        .save(
            0,
            Preferences {
                touch_spelling_editor: false,
                ..Preferences::default()
            },
        )
        .unwrap();
    assert!(!store.load().unwrap().preferences.touch_spelling_editor);
    assert_eq!(
        saved.preferences.touch_spelling_position,
        TouchSpellingPosition::Editor
    );
    legacy["preferences"]["touch_spelling_editor"] = serde_json::json!("false");
    assert!(serde_json::from_value::<PreferencesSnapshot>(legacy).is_err());
}
