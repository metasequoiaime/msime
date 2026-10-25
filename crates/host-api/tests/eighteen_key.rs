// 用合成词库验证十八键的 C ABI、方案限制、拼写锁定和实体键盘边界。
#![allow(unsafe_code)]

use msime_client_core::preferences::{DefaultImeMode, InputScheme, Preferences};
use msime_host_api::{
    msime_client_character, msime_client_choose_nine_key_spelling, msime_client_command,
    msime_client_create, msime_client_destroy, msime_client_focus, msime_client_grid_key,
    msime_client_set_key_grid, msime_client_string_free, msime_client_view,
};
use serde_json::{json, Value};
use std::{
    ffi::{c_char, CStr},
    path::Path,
};

fn read(raw: *mut c_char) -> Value {
    assert!(!raw.is_null());
    // 安全：字符串来自同一 C ABI，同步读取后只释放一次。
    unsafe {
        let result = serde_json::from_slice(CStr::from_ptr(raw).to_bytes());
        msime_client_string_free(raw);
        result.unwrap()
    }
}
fn create(root: &Path, scheme: InputScheme) -> u64 {
    std::fs::create_dir_all(root).unwrap();
    rusqlite::Connection::open(root.join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_1_n VALUES('ni','n','你',100);
         CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_1_b VALUES('bo','b','波',90);
         CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_1_h VALUES('hao','h','好',100);
         CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',200);
         CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
         INSERT INTO tbl_2_b VALUES('bo''hao','bh','波好',150);",
        )
        .unwrap();
    let options = serde_json::to_vec(&json!({"api_version":1,"resources":root,"user_data":root,
    "cache":root,"dictionaries":root,"preferences":Preferences {
        default_ime_mode:DefaultImeMode::Chinese, scheme, learning:false, ..Preferences::default()
    }}))
    .unwrap();
    // 安全：选项缓冲区在同步调用期间有效。
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    handle
}
#[test]
fn eighteen_key_ffi_keeps_the_native_session_and_exact_key_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let handle = create(directory.path(), InputScheme::Quanpin);
    let enabled = read(msime_client_set_key_grid(handle, 4));
    assert_eq!(enabled["value"]["key_grid"], "eighteen_key", "{enabled}");
    assert_eq!(enabled["value"]["nine_key"], false);
    let physical = read(msime_client_character(handle, b'n', false));
    assert_eq!(physical["value"]["view"]["preedit"], "n");
    assert_eq!(
        read(msime_client_grid_key(handle, b'n'))["value"]["handled"],
        false
    );
    read(msime_client_command(handle, 3));
    let mut typed = Value::Null;
    for letter in b"nihao" {
        typed = read(msime_client_grid_key(handle, *letter));
        assert_eq!(typed["value"]["handled"], true, "{typed}");
    }
    let view = &typed["value"]["view"];
    let texts: Vec<_> = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["text"].as_str().unwrap())
        .collect();
    assert!(
        texts.contains(&"你好") && texts.contains(&"波好"),
        "{texts:?}"
    );
    let generation = view["generation"].as_u64().unwrap();
    let spelling = view["nine_key_spellings"]
        .as_array()
        .unwrap()
        .iter()
        .position(|item| item == "ni")
        .unwrap();
    assert_eq!(
        read(msime_client_choose_nine_key_spelling(
            handle, generation, spelling
        ))["value"]["handled"],
        true
    );
    assert_eq!(read(msime_client_set_key_grid(handle, 0))["ok"], false);
    let expected = read(msime_client_view(handle))["value"]["candidates"][0]["text"].clone();
    let committed = read(msime_client_command(handle, 1));
    assert_eq!(committed["value"]["commit"], expected);
    assert_eq!(
        read(msime_client_set_key_grid(handle, 0))["value"]["key_grid"],
        "none"
    );
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
#[test]
fn eighteen_key_ffi_rejects_other_schemes_without_changing_the_view() {
    let directory = tempfile::tempdir().unwrap();
    let handle = create(directory.path(), InputScheme::Shuangpin);
    let before = read(msime_client_view(handle));
    assert_eq!(read(msime_client_set_key_grid(handle, 4))["ok"], false);
    assert_eq!(read(msime_client_view(handle)), before);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
