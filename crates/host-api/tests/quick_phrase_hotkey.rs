// 使用合成快捷短语验证完整 C ABI、运行时与引擎，不读取用户数据。
#![allow(unsafe_code)]
use msime_client_core::preferences::{DefaultImeMode, Preferences};
use msime_host_api::{
    msime_client_character, msime_client_command, msime_client_create, msime_client_destroy,
    msime_client_focus, msime_client_set_english_mode, msime_client_string_free, msime_client_view,
};
use serde_json::{json, Value};
use std::ffi::{c_char, CStr};

fn read(raw: *mut c_char) -> Value {
    assert!(!raw.is_null());
    // 安全：同一 ABI 的返回值同步读取，并在读取后释放一次。
    unsafe {
        let result = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        msime_client_string_free(raw);
        result.unwrap()
    }
}
fn create(root: &std::path::Path, enabled: bool, shift_entry: bool) -> u64 {
    rusqlite::Connection::open(root.join("msime-pinyin.db")).unwrap().execute_batch(
        "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER); INSERT INTO quick_parases VALUES('ab','合成短语',100);"
    ).unwrap();
    let mut preferences = Preferences {
        default_ime_mode: DefaultImeMode::Chinese,
        learning: false,
        ..Preferences::default()
    };
    preferences.local_modes.quick_phrase = enabled;
    preferences.local_modes.shift_letter_entry = shift_entry;
    let request = serde_json::to_vec(&json!({"api_version":1, "resources":root, "user_data":root, "cache":root, "dictionaries":root, "preferences":preferences})).unwrap();
    // 安全：选项字节在同步创建期间有效。
    let result = read(unsafe { msime_client_create(request.as_ptr(), request.len()) });
    assert_eq!(result["ok"], true, "{result}");
    let session = result["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(session, true))["ok"], true);
    session
}
#[test]
fn explicit_quick_phrase_entry_does_not_depend_on_shift_entry_and_selects_a_real_phrase() {
    for shift_entry in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let session = create(directory.path(), true, shift_entry);
        let opened = read(msime_client_command(session, 21));
        assert_eq!(opened["value"]["handled"], true, "{opened}");
        assert_eq!(opened["value"]["view"]["local_mode"], "quick_phrase");
        read(msime_client_character(session, b'a', false));
        let typed = read(msime_client_character(session, b'b', false));
        assert_eq!(
            typed["value"]["view"]["candidates"][0]["text"], "合成短语",
            "{typed}"
        );
        let committed = read(msime_client_command(session, 1));
        assert_eq!(committed["value"]["commit"], "合成短语");
        assert_eq!(committed["value"]["view"]["local_mode"], "none");
        if shift_entry {
            assert_eq!(
                read(msime_client_character(session, b'K', true))["value"]["view"]["local_mode"],
                "quick_phrase"
            );
            read(msime_client_command(session, 3));
        }
        read(msime_client_destroy(session));
    }
}
#[test]
fn explicit_entry_preserves_composition_and_respects_disabled_mode_and_dedicated_english() {
    let directory = tempfile::tempdir().unwrap();
    let session = create(directory.path(), false, true);
    assert_eq!(
        read(msime_client_command(session, 21))["value"]["handled"],
        false
    );
    read(msime_client_destroy(session));
    let directory = tempfile::tempdir().unwrap();
    let session = create(directory.path(), true, true);
    read(msime_client_character(session, b'n', false));
    let before = read(msime_client_view(session));
    assert_eq!(
        read(msime_client_command(session, 21))["value"]["handled"],
        false
    );
    assert_eq!(
        read(msime_client_view(session))["value"]["editing_text"],
        before["value"]["editing_text"]
    );
    read(msime_client_command(session, 3));
    read(msime_client_set_english_mode(session, true));
    assert_eq!(
        read(msime_client_command(session, 21))["value"]["handled"],
        false
    );
    assert_eq!(
        read(msime_client_view(session))["value"]["dedicated_english"],
        true
    );
    read(msime_client_destroy(session));
}
