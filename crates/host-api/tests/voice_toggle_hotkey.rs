// 集成测试独立成 crate；直接验证 C ABI，缓冲区与返回值的所有权在调用处逐一说明。
#![allow(unsafe_code)]

use std::ffi::CStr;

use msime_host_api::{
    msime_client_check_toggle_hotkey, msime_client_string_free, msime_client_toggle_hotkey_chords,
};
use serde_json::{json, Value};

fn decode(request: Value) -> Value {
    let bytes = serde_json::to_vec(&request).unwrap();
    // SAFETY: 缓冲区在调用期间有效；返回字符串只读取一次，随后交回同一接口释放。
    unsafe {
        let raw = msime_client_toggle_hotkey_chords(bytes.as_ptr(), bytes.len());
        assert!(!raw.is_null());
        let response = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        msime_client_string_free(raw);
        response.unwrap()
    }
}

#[test]
fn voice_toggle_chord_ffi_preserves_defaults_and_decodes_modifier_aliases() {
    let default = decode(json!({}));
    assert_eq!(default["ok"], true);
    assert_eq!(default["value"]["voice_input"], Value::Null);
    let configured = decode(json!({"keybindings": {
        "toggle_voice_input": " option + control + f8 "
    }}));
    assert_eq!(configured["ok"], true);
    assert_eq!(
        configured["value"]["voice_input"],
        json!({
            "key": "F8", "ctrl": true, "alt": true, "shift": false, "meta": false
        })
    );
    for key in ["english_candidates", "full_width", "punctuation"] {
        assert_eq!(configured["value"][key], default["value"][key]);
    }
    for unbound in ["", "   ", "Ctrl+Banana"] {
        let value = decode(json!({"keybindings": {"toggle_voice_input": unbound}}));
        assert_eq!(value["ok"], true);
        assert_eq!(value["value"]["voice_input"], Value::Null);
    }
}

#[test]
fn voice_toggle_check_ffi_normalizes_and_reports_conflicts_without_saving() {
    fn check(chord: &str, fixed_enabled: bool) -> Value {
        let bytes = serde_json::to_vec(&json!({
            "field": "toggle_voice_input", "chord": chord,
            "keybindings": {}, "voice_ctrl_f9": fixed_enabled
        }))
        .unwrap();
        // SAFETY: 请求在调用期间有效，返回结果由同一 ABI 释放。
        unsafe {
            let raw = msime_client_check_toggle_hotkey(bytes.as_ptr(), bytes.len());
            assert!(!raw.is_null());
            let result: Value = serde_json::from_slice(CStr::from_ptr(raw).to_bytes()).unwrap();
            msime_client_string_free(raw);
            assert_eq!(result["ok"], true);
            result["value"].clone()
        }
    }
    assert_eq!(
        check(" option + control + f8 ", true),
        json!({"ok": true, "chord": "Ctrl+Alt+F8"})
    );
    assert_eq!(check("", true), json!({"ok": true, "chord": ""}));
    assert_eq!(
        check("Ctrl+Banana", true),
        json!({"ok": false, "kind": "invalid"})
    );
    assert_eq!(
        check("Ctrl+F9", true),
        json!({"ok": false, "kind": "conflict", "owner": "voice_hotkey"})
    );
    assert_eq!(
        check("Ctrl+F9", false),
        json!({"ok": true, "chord": "Ctrl+F9"})
    );
    assert_eq!(
        check("Ctrl+Period", true),
        json!({"ok": false, "kind": "conflict", "owner": "toggle_punctuation"})
    );
}
