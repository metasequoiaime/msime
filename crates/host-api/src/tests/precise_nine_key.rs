use super::*;

#[test]
fn precise_nine_key_preference_reaches_new_and_live_sessions() {
    let fixture = tempfile::tempdir().unwrap();
    let mut preferences = Preferences {
        touch_keyboard_layout: TouchKeyboardLayout::NineKey,
        touch_nine_key_precise: true,
        ..chinese_preferences()
    };
    let handle = test_host_with_pinyin_fixture(fixture.path(), preferences.clone());
    read(msime_client_focus(handle, true));
    let typed = read(msime_client_character(handle, b'2', false));
    assert_eq!(
        typed["value"]["view"]["nine_key_spellings"],
        json!(["A", "B", "C"])
    );
    read(msime_client_command(handle, 3));

    preferences.touch_nine_key_precise = false;
    assert_eq!(update(handle, 1, &preferences)["ok"], true);
    let typed = read(msime_client_character(handle, b'2', false));
    assert!(typed["value"]["view"]["nine_key_spellings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|spelling| spelling == "cong"));
    read(msime_client_destroy(handle));
}
