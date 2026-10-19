use msime_client_services::account::name::present_name;

#[test]
fn account_name_trims_the_existing_ui_whitespace_set() {
    let whitespace = "\t\n\u{b}\u{c}\r \u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
    let name = present_name(&format!("{whitespace}水杉 小鹿{whitespace}"), "abcdef12");
    assert_eq!(name.normalized, "水杉 小鹿");
    assert_eq!(name.preferred, "水杉 小鹿");
    assert!(name.valid);
    assert_eq!(name.maximum_chars, 64);
}

#[test]
fn account_name_counts_unicode_scalars_and_keeps_the_draft() {
    for character in ["名", "🦌"] {
        assert!(present_name(&character.repeat(64), "").valid);
        let name = present_name(&character.repeat(64), "");
        assert!(name.normalized.encode_utf16().count() <= name.maximum_utf16_units);
        assert_eq!(name.maximum_utf16_units, 128);
        let too_long = character.repeat(65);
        let name = present_name(&too_long, "");
        assert!(!name.valid);
        assert_eq!(name.normalized, too_long);
    }
}

#[test]
fn account_name_uses_the_backend_control_character_rules() {
    for control in ['\0', '\n', '\r', '\t', '\u{7f}', '\u{85}', '\u{9f}'] {
        let draft = format!("小{control}鹿");
        let name = present_name(&draft, "");
        assert!(!name.valid);
        assert_eq!(name.normalized, draft);
    }
    let name = present_name("\u{85}小鹿\u{85}", "");
    assert_eq!(name.normalized, "\u{85}小鹿\u{85}");
    assert!(!name.valid);
}

#[test]
fn account_name_falls_back_to_the_existing_generated_name() {
    for draft in ["", " \t\u{feff}"] {
        let name = present_name(draft, "abcdef12-3456");
        assert!(!name.valid);
        assert_eq!(name.normalized, "");
        assert_eq!(name.preferred, "水杉小鹿·ABCDEF");
    }
    assert_eq!(present_name("", "ab").preferred, "水杉小鹿·AB");
}
