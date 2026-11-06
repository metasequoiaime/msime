//! 移动端清理必须按存储中的当前开关执行，不能相信异步回调里的旧关闭值。

use super::*;

#[test]
fn delayed_disable_preserves_reenabled_history_and_explicit_clear_still_works() {
    let root = tempfile::tempdir().unwrap();
    let preferences_root = root.path().join("preferences");
    let store = PreferencesStore::new(&preferences_root);
    let original = store.load().unwrap();
    assert!(!original.preferences.clipboard_history);
    let mut enabled = original.preferences;
    enabled.clipboard_history = true;
    enabled.clipboard_history_limit = 100;
    let saved = store.save(original.revision, enabled).unwrap();
    let call = |action: Value| {
        let request = serde_json::to_vec(&json!({
            "directory": root.path(), "preferences_directory": preferences_root, "action": action,
        }))
        .unwrap();
        read(unsafe { msime_client_mobile_clipboard_history(request.as_ptr(), request.len()) })
    };
    for index in 0..60 {
        assert_eq!(
            call(json!({"operation": "capture", "text": format!("synthetic-{index:03}")}))["value"]
                ["captured"],
            true
        );
        if index == 0 {
            assert_eq!(
                call(json!({"operation": "clear_if_disabled"}))["value"]["cleared"],
                false
            );
        }
    }
    assert_eq!(
        call(json!({"operation": "load"}))["value"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        60
    );
    let mut disabled = saved.preferences;
    disabled.clipboard_history = false;
    store.save(saved.revision, disabled).unwrap();
    assert_eq!(
        call(json!({"operation": "clear_if_disabled"}))["value"]["cleared"],
        true
    );
    assert_eq!(
        call(json!({"operation": "load"}))["value"]["entries"],
        json!([])
    );
    let current = store.load().unwrap();
    let mut enabled = current.preferences;
    enabled.clipboard_history = true;
    store.save(current.revision, enabled).unwrap();
    assert_eq!(
        call(json!({"operation": "capture", "text": "synthetic-explicit"}))["value"]["captured"],
        true
    );
    assert_eq!(
        call(json!({"operation": "clear"}))["value"]["cleared"],
        true
    );
    assert_eq!(
        call(json!({"operation": "load"}))["value"]["entries"],
        json!([])
    );
    assert_eq!(
        call(json!({"operation": "capture", "text": "synthetic-unreadable"}))["value"]["captured"],
        true
    );
    std::fs::write(
        preferences_root.join("preferences.json"),
        b"invalid synthetic document",
    )
    .unwrap();
    assert_eq!(call(json!({"operation": "clear_if_disabled"}))["ok"], false);
    assert!(root.path().join("MSIME/clipboard_history.json").exists());
}
