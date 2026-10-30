use super::*;

#[test]
fn quick_phrase_cursor_survives_host_formatting_and_empty_end() {
    for (phrase, full_width, spaced, text, suffix) in [
        ("title:\"${cursor}\"", false, false, "title:\"\"", "\""),
        ("title:\"${cursor}\"", true, false, "ｔｉｔｌｅ：＂＂", "＂"),
        ("用${cursor}Rust写", false, true, "用 Rust 写", "Rust 写"),
        ("用Rust${cursor}写", false, true, "用 Rust 写", "写"),
        ("末尾${cursor}", false, false, "末尾", ""),
        ("${cursor}", false, false, "", ""),
        (
            "前${cursor}𠀀👩‍💻e\u{301}",
            false,
            false,
            "前𠀀👩‍💻e\u{301}",
            "𠀀👩‍💻e\u{301}",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        install_phrase_table(dir.path(), "cursor", &[("ab", phrase)]);
        let mut preferences = chinese_preferences();
        preferences.plugins.phrase_tables = vec!["cursor".into()];
        preferences.chinese_english_spacing = spaced;
        let handle = plugin_host(dir.path(), preferences);
        assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
        if full_width {
            SESSIONS.with(|sessions| {
                sessions
                    .borrow_mut()
                    .get_mut(&handle)
                    .unwrap()
                    .runtime
                    .set_character_width(CharacterWidth::Fullwidth);
            });
        }
        for key in b"Kab" {
            assert_eq!(
                read(msime_client_character(
                    handle,
                    *key,
                    key.is_ascii_uppercase()
                ))["ok"],
                true
            );
        }
        let committed = read(msime_client_command(handle, 1));
        assert_eq!(committed["ok"], true);
        assert_eq!(committed["value"]["commit"], text, "{phrase}");
        assert_eq!(
            committed["value"]["commit_cursor_suffix"], suffix,
            "{phrase}"
        );
        assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    }
}

#[test]
fn quick_phrase_cursor_drops_an_invalid_suffix_before_formatting() {
    let root = tempfile::tempdir().unwrap();
    let handle = test_host(root.path());
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions.get_mut(&handle).unwrap();
        let completed = session.complete_transition(Transition {
            handled: true,
            commit: Some("title:\"\"".into()),
            commit_cursor_suffix: Some("stale".into()),
            commit_context: None,
            diagnostic: None,
            view: session.runtime.view(),
        });
        assert_eq!(completed.commit.as_deref(), Some("title:\"\""));
        assert!(completed.commit_cursor_suffix.is_none());
    });
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
