//! 从原生宿主使用的 JSON 入口验证来源分批契约与旧宿主兼容性。
#![allow(unsafe_code)]

use msime_host_api::{msime_client_string_free, msime_client_typing_statistics};
use serde_json::{json, Value};
use std::ffi::CStr;

fn call(directory: &std::path::Path, action: Value) -> Value {
    let request = serde_json::to_vec(&json!({"directory": directory, "action": action})).unwrap();
    // 安全：同步调用期间请求有效，返回字符串只读取、释放一次。
    unsafe {
        let raw = msime_client_typing_statistics(request.as_ptr(), request.len());
        assert!(!raw.is_null());
        let response = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        msime_client_string_free(raw);
        response.unwrap()
    }
}

#[test]
fn source_batches_preserve_totals_and_legacy_callers() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path();
    assert_eq!(
        call(path, json!({"operation": "set_enabled", "enabled": true}))["ok"],
        true
    );
    let action = json!({"operation": "record_keys", "day": "2026-11-17",
        "keys": {"KeyA": 3, "Tab": 2},
        "sources": {"quanpin": {"KeyA": 1, "Tab": 1}, "english": {"KeyA": 2, "Tab": 1}}
    });
    let response = call(path, action.clone());
    assert_eq!(response["value"]["recorded"], 5, "{response}");
    assert_eq!(
        call(
            path,
            json!({"operation": "record_keys", "day": "2026-11-17", "keys": {"KeyA": 1}})
        )["value"]["recorded"],
        1
    );
    let loaded = call(path, json!({"operation": "load"}));
    let groups = &loaded["value"]["dailyKeyLanguages"]["2026-11-17"];
    assert_eq!(groups["chinese"]["KeyA"], 1);
    assert_eq!(groups["english"]["KeyA"], 2);
    assert_eq!(groups["unknown"]["KeyA"], 1);
    assert_eq!(groups["common"]["Tab"], 2);
    let mut invalid = action;
    invalid["sources"]["english"]["KeyA"] = json!(1);
    assert_eq!(call(path, invalid)["ok"], false);
    assert_eq!(call(path, json!({"operation": "load"})), loaded);
}
