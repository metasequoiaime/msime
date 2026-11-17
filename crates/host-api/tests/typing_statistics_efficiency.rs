//! 正式库经候选选择记录效率，不依赖 `cfg(test)` 中的同步写入路径。
#![allow(unsafe_code)]

use msime_client_core::preferences::{DefaultImeMode, Preferences};
use msime_client_services::typing_statistics::TypingStatisticsStore;
use msime_host_api::{
    msime_client_character, msime_client_create, msime_client_destroy, msime_client_focus,
    msime_client_select, msime_client_string_free,
};
use serde_json::{json, Value};
use std::ffi::{c_char, CStr};
use std::time::{Duration, Instant};

fn read(raw: *mut c_char) -> Value {
    assert!(!raw.is_null());
    // 安全：返回字符串来自同一 ABI，同步读取后只释放一次。
    unsafe {
        let response = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        msime_client_string_free(raw);
        response.unwrap()
    }
}

#[test]
fn native_library_records_candidate_efficiency_when_statistics_is_enabled() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    rusqlite::Connection::open(root.join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_1_n VALUES('ni','n','你',100);",
        )
        .unwrap();
    let store = TypingStatisticsStore::new(root);
    store.set_enabled(true).unwrap();
    let request = serde_json::to_vec(&json!({
        "api_version": 1, "resources": root, "user_data": root,
        "cache": root, "dictionaries": root, "preferences_directory": root,
        "preferences": Preferences {
            default_ime_mode: DefaultImeMode::Chinese,
            learning: false,
            ..Preferences::default()
        }
    }))
    .unwrap();
    // 安全：请求缓冲区在同步调用期间有效。
    let created = read(unsafe { msime_client_create(request.as_ptr(), request.len()) });
    assert_eq!(created["ok"], true, "{created}");
    let session = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(session, true))["ok"], true);
    read(msime_client_character(session, b'n', false));
    let typed = read(msime_client_character(session, b'i', false));
    let view = &typed["value"]["view"];
    let index = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .position(|candidate| candidate["text"] == "你")
        .unwrap();
    let committed = read(msime_client_select(
        session,
        view["generation"].as_u64().unwrap(),
        index,
    ));
    assert_eq!(committed["value"]["commit"], "你");
    assert_eq!(read(msime_client_focus(session, false))["ok"], true);
    read(msime_client_destroy(session));

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let value = store.load().unwrap();
        if value.efficiency.commits > 0 {
            assert_eq!(value.efficiency.commits, 1);
            assert_eq!(value.efficiency.typed_keys, 2);
            assert_eq!(value.selections.total(), 1);
            break;
        }
        assert!(Instant::now() < deadline, "正式共享库没有记录候选上屏效率");
        std::thread::sleep(Duration::from_millis(10));
    }
}
