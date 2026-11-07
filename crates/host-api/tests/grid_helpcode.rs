// 用合成词库和辅助码表验证十四／十八键网格辅助码的偏好接线、C ABI 与视图字段。
#![allow(unsafe_code)]

use msime_client_core::preferences::{DefaultImeMode, HelpcodePreferences, Preferences};
use msime_host_api::{
    msime_client_choose_nine_key_spelling, msime_client_create, msime_client_destroy,
    msime_client_focus, msime_client_grid_helpcode, msime_client_grid_key,
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

/// 打好 14 键 `nihao` 的会话；`grid` 是全拼辅助码偏好里的网格辅助码开关。
fn typed(root: &Path, grid: bool) -> u64 {
    std::fs::create_dir_all(root.join("helpcodes")).unwrap();
    std::fs::write(
        root.join("helpcodes/zrm_helpcode_big_unique.txt"),
        "你=ab\n拟=cd\n逆=ea\n好=ef\n",
    )
    .unwrap();
    rusqlite::Connection::open(root.join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',800),('ni''hao','nh','逆好',600);",
        )
        .unwrap();
    let preferences = Preferences {
        default_ime_mode: DefaultImeMode::Chinese,
        learning: false,
        quanpin_helpcode: HelpcodePreferences {
            grid,
            ..Preferences::default().quanpin_helpcode
        },
        ..Preferences::default()
    };
    let options = serde_json::to_vec(&json!({"api_version":1,"resources":root,"user_data":root,
    "cache":root,"dictionaries":root,"preferences":preferences}))
    .unwrap();
    // 安全：选项缓冲区在同步调用期间有效。
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    assert_eq!(
        read(msime_client_set_key_grid(handle, 2))["value"]["key_grid"],
        "fourteen_key"
    );
    for letter in *b"bugao" {
        let typed = read(msime_client_grid_key(handle, letter));
        assert_eq!(typed["value"]["handled"], true, "{typed}");
    }
    handle
}

fn words(view: &Value) -> Vec<String> {
    view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["text"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_grid_preference_reaches_the_session_and_the_abi_drives_it() {
    let directory = tempfile::tempdir().unwrap();
    let handle = typed(directory.path(), true);
    let plain = read(msime_client_view(handle))["value"].clone();
    assert_eq!(plain["grid_helpcode_available"], true, "{plain}");
    assert!(plain.get("grid_helpcode_display").is_none());

    let begun = read(msime_client_grid_helpcode(handle, 0, 0));
    assert_eq!(begun["value"]["handled"], true, "{begun}");
    assert_eq!(begun["value"]["view"]["grid_helpcode_display"], "|");
    let refused = read(msime_client_grid_key(handle, b'a'));
    assert_eq!(refused["value"]["handled"], false, "{refused}");
    read(msime_client_grid_helpcode(handle, 3, b'a'));
    let filtered = read(msime_client_grid_helpcode(handle, 3, b'e'))["value"]["view"].clone();
    assert_eq!(filtered["grid_helpcode_display"], "|[AS][ER]");
    assert_eq!(
        filtered["nine_key_spellings"],
        json!(["AE", "AR", "SE", "SR"])
    );
    assert_eq!(words(&filtered), ["你好"]);
    assert!(!words(&filtered).iter().any(|word| word == "|"));
    assert!(!filtered["preedit"].as_str().unwrap().contains('|'));

    let generation = filtered["generation"].as_u64().unwrap();
    let chosen = read(msime_client_choose_nine_key_spelling(handle, generation, 0));
    assert_eq!(chosen["value"]["handled"], true, "{chosen}");
    assert_eq!(chosen["value"]["view"]["grid_helpcode_display"], "|AE");

    let invalid = read(msime_client_grid_helpcode(handle, 9, 0));
    assert_eq!(invalid["ok"], false, "{invalid}");
    let cleared = read(msime_client_grid_helpcode(handle, 2, 0));
    assert!(cleared["value"]["view"]
        .get("grid_helpcode_display")
        .is_none());
    assert_eq!(words(&cleared["value"]["view"]), words(&plain));
    read(msime_client_destroy(handle));
}

#[test]
fn the_grid_preference_is_off_by_default() {
    let directory = tempfile::tempdir().unwrap();
    let handle = typed(directory.path(), false);
    let view = read(msime_client_view(handle))["value"].clone();
    assert!(view.get("grid_helpcode_available").is_none(), "{view}");
    let refused = read(msime_client_grid_helpcode(handle, 0, 0));
    assert_eq!(refused["value"]["handled"], false, "{refused}");
    let after = &refused["value"]["view"];
    assert!(after.get("grid_helpcode_display").is_none());
    assert_eq!(words(after), words(&view));
    assert_eq!(after["nine_key_spellings"], view["nine_key_spellings"]);
    read(msime_client_destroy(handle));
}
