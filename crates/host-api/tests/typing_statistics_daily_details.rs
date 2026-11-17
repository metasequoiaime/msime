//! 用合成统计文件验证原生宿主读取的明细契约，不加载任何真实输入数据。
#![allow(unsafe_code)]

use msime_host_api::{msime_client_string_free, msime_client_typing_statistics};
use serde_json::{json, Value};
use std::ffi::CStr;

#[test]
fn daily_details_cross_the_existing_summary_boundary() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("typing-statistics.json"),
        serde_json::to_vec(&json!({
            "enabled": false,
            "total": 12,
            "days": {"2026-11-16": 5, "2026-11-17": 7},
            "dailyDetails": {"2026-11-17": {"characters": {"han": 4, "latin": 2}}},
            "dailyActiveMs": {"2026-11-17": 30_000}
        }))
        .unwrap(),
    )
    .unwrap();
    let request = serde_json::to_vec(&json!({
        "directory": directory.path(),
        "action": {"operation": "summary", "day": "2026-11-17"}
    }))
    .unwrap();
    // 安全：请求在同步调用期间有效，返回字符串只读取并释放一次。
    let response: Value = unsafe {
        let raw = msime_client_typing_statistics(request.as_ptr(), request.len());
        assert!(!raw.is_null());
        let response = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        msime_client_string_free(raw);
        response.unwrap()
    };
    assert_eq!(response["ok"], true, "{response}");
    let rows = response["value"]["daily_details"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["day"], "2026-11-17");
    assert_eq!(rows[0]["other"], 1);
    assert_eq!(rows[0]["active_ms"], 30_000);
    assert_eq!(rows[0]["speed"], 12.0);
    assert_eq!(rows[1]["other"], 5);
    assert!(rows[1]["active_ms"].is_null());
    assert!(rows[1]["speed"].is_null());
}
