use crate::tests::{host_options_with_main_dictionary, read};
use crate::{msime_client_create, msime_client_destroy, msime_client_dictionary_collections};
use serde_json::json;

fn call_android_data(
    function: unsafe extern "C" fn(*const u8, usize) -> *mut std::ffi::c_char,
    request: serde_json::Value,
) -> serde_json::Value {
    let bytes = serde_json::to_vec(&request).unwrap();
    // 请求缓冲区在同步调用返回前保持有效。
    read(unsafe { function(bytes.as_ptr(), bytes.len()) })
}

#[test]
fn empty_collection_settings_sync_does_not_interrupt_a_live_input_session() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let created = call_android_data(msime_client_create, options.clone());
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    let result = call_android_data(
        msime_client_dictionary_collections,
        json!({"options":options,"action":{"operation":"synchronize_settings","budget_millis":100}}),
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["value"]["pending_count"], 0);
    assert!(!root.path().join("user/.msime-dictionary-quiesce").exists());
    assert_eq!(read(crate::msime_client_focus(handle, true))["ok"], true);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}

#[test]
fn collection_settings_sync_checks_the_budget_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    for budget in [0, 2001] {
        let response = call_android_data(
            msime_client_dictionary_collections,
            json!({"options":options,"action":{"operation":"synchronize_settings","budget_millis":budget}}),
        );
        assert_eq!(response["ok"], false);
        assert_eq!(response["error"], "collections_invalid");
    }
    assert!(!root.path().join("DictionaryCollections").exists());
}

#[test]
fn collection_settings_sync_applies_queued_words_and_batch_removal() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let call = |action| {
        call_android_data(
            msime_client_dictionary_collections,
            json!({"options": options, "action": action}),
        )
    };
    let imported = call(
        json!({"operation": "import", "name": "合成分组", "kind": "pinyin",
        "format": "standard", "text": "拟好\tni'hao\t100\n"}),
    );
    assert_eq!(imported["ok"], true, "{imported}");
    let id = imported["value"]["collections"][0]["id"].clone();
    for (action, expected) in [
        (
            json!({"operation": "synchronize_settings", "budget_millis": 2000}),
            1,
        ),
        (json!({"operation": "delete_many", "ids": [id]}), 1),
        (
            json!({"operation": "synchronize_settings", "budget_millis": 2000}),
            0,
        ),
    ] {
        let result = call(action);
        assert_eq!(result["ok"], true, "{result}");
        let count = call_android_data(
            crate::msime_client_dictionary,
            json!({"options": options, "action": {"operation": "count", "user_only": true}}),
        );
        assert_eq!(count["ok"], true, "{count}");
        assert_eq!(count["value"]["count"], expected, "{count}");
    }
}
