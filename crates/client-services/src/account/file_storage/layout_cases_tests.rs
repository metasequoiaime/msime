//! 逐条跑各平台共用的 `shared/contracts/account-session-layouts/cases.json`（#8201 迁移计划第 6 步）：Native、Apple（Swift 写的和 Rust 写的）、鸿蒙扁平（含已过期的）和 Android v2 存储里的真实字节，Rust 都按同一个规则读出；Rust 自己写出的 Native 和 Apple 布局与用例逐字节相同。Swift 的 `BackendSessionLayoutCaseTests` 和 Android 的 `SessionStoreDeviceSmoke` 读同一份用例。

use super::*;
use std::collections::BTreeSet;

/// 用例文件里的全部用例。
pub(crate) fn layout_cases() -> Vec<serde_json::Value> {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../shared/contracts/account-session-layouts/cases.json"
    ))
    .unwrap();
    contract["cases"].as_array().unwrap().clone()
}

/// 名为 `name` 的那一条用例。
pub(crate) fn layout_case(name: &str) -> serde_json::Value {
    layout_cases()
        .into_iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("no layout case named {name}"))
}

/// 用例 `document` 的原样字节。
pub(crate) fn case_document(case: &serde_json::Value) -> Vec<u8> {
    case["document"].as_str().unwrap().as_bytes().to_vec()
}

/// 用例的 `session` 按 [`SavedAccountSession`] 序列化后的样子：`session_id` 为 null 时序列化里没有这个键。
pub(crate) fn expected_session(case: &serde_json::Value) -> serde_json::Value {
    let mut session = case["session"].clone();
    let object = session.as_object_mut().unwrap();
    if object
        .get("session_id")
        .is_some_and(serde_json::Value::is_null)
    {
        object.remove("session_id");
    }
    session
}

#[test]
fn every_platform_layout_reads_the_same_and_rust_writes_its_own_byte_for_byte() {
    let cases = layout_cases();
    let mut layouts = BTreeSet::new();
    let mut written = BTreeSet::new();
    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let layout = case["layout"].as_str().unwrap();
        layouts.insert(layout.to_owned());
        let now = case["now_unix_ms"].as_u64().unwrap();
        let decoded = decode_at(&case_document(case), now);
        if let Some(code) = case["error"].as_str() {
            assert_eq!(
                decoded.err().map(|error| error.code()),
                Some(code),
                "{name}"
            );
            continue;
        }
        let saved = decoded.unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert_eq!(
            serde_json::to_value(&saved).unwrap(),
            expected_session(case),
            "{name}"
        );
        // 读出来的令牌和用户本身也是合法的，`BackendAccountSession` 才会用它。
        super::super::validate::validate_tokens(&saved.tokens)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        for read in case["expires_in_at"].as_array().into_iter().flatten() {
            let later =
                decode_at(&case_document(case), read["now_unix_ms"].as_u64().unwrap()).unwrap();
            assert_eq!(
                later.tokens.expires_in,
                read["expires_in"].as_u64().unwrap(),
                "{name} at {}",
                read["now_unix_ms"]
            );
        }
        if case["rust_writes"] == true {
            let file_layout = match layout {
                "native" => AccountSessionFileLayout::Native,
                "apple" => AccountSessionFileLayout::Apple,
                other => panic!("{name}: Rust does not write the {other} layout"),
            };
            let bytes = encode_session(file_layout, &saved).unwrap();
            assert_eq!(
                String::from_utf8(bytes).unwrap(),
                case["document"].as_str().unwrap(),
                "{name}"
            );
            written.insert(layout.to_owned());
        }
    }
    // 四种布局都有用例，两种可写的布局都核对过 Rust 写出的字节。
    assert_eq!(
        layouts,
        BTreeSet::from(["android_v2", "apple", "harmony_flat", "native"].map(String::from))
    );
    assert_eq!(
        written,
        BTreeSet::from(["apple", "native"].map(String::from))
    );
}

#[test]
fn the_file_store_reads_every_case_from_disk_as_decode_does() {
    // 经真的会话文件（仅本人可读写）读，与内存里解码的结果相同；鸿蒙扁平布局的 expires_in 取决于读的时刻，只核对它至少是 1。
    for case in layout_cases() {
        let name = case["name"].as_str().unwrap();
        let root = tempfile::tempdir().unwrap();
        let storage = FileAccountSessionStorage::new(root.path(), AccountSessionFileLayout::Native);
        storage
            .write_private(ACCOUNT_SESSION_FILE, &case_document(&case))
            .unwrap();
        let loaded = storage.load();
        if let Some(code) = case["error"].as_str() {
            assert_eq!(loaded.err().map(|error| error.code()), Some(code), "{name}");
            continue;
        }
        let mut loaded = serde_json::to_value(loaded.unwrap().unwrap()).unwrap();
        let mut expected = expected_session(&case);
        if case["layout"] == "harmony_flat" {
            assert!(
                loaded["tokens"]["expires_in"].as_u64().unwrap() >= 1,
                "{name}"
            );
            loaded["tokens"]["expires_in"] = 0.into();
            expected["tokens"]["expires_in"] = 0.into();
        }
        assert_eq!(loaded, expected, "{name}");
    }
}
