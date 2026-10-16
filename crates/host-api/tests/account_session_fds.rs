//! 按账号目录缓存的会话不能各自留着一份 HTTP 客户端：reqwest 的阻塞客户端各带一个后台线程和它的事件循环（macOS 上占三个文件描述符），按目录缓存的会话一直不放，每多一个目录就多攒一份，长期运行的进程和每个用例建一个临时目录的测试会把默认 256 的打开文件上限占满。这个测试单独一个进程，数到的描述符只有它自己的。
// Integration tests are their own crates, so the exemption the library root carries does not reach them: this target drives the C ABI directly.
#![allow(unsafe_code)]
#![cfg(unix)]

use msime_host_api::account::msime_client_account;
use msime_host_api::msime_client_string_free;
use std::ffi::CStr;
use std::path::Path;

fn open_descriptors() -> usize {
    std::fs::read_dir("/dev/fd").unwrap().count()
}

fn status(directory: &Path) -> serde_json::Value {
    let request = serde_json::json!({
        "directory": directory,
        "action": { "operation": "status" },
    })
    .to_string();
    // SAFETY: 缓冲区在调用期间有效；返回的字符串由本库分配，读完以后交还本库释放。
    unsafe {
        let response = msime_client_account(request.as_ptr(), request.len());
        assert!(!response.is_null());
        let value = serde_json::from_slice(CStr::from_ptr(response).to_bytes()).unwrap();
        msime_client_string_free(response);
        value
    }
}

#[test]
fn sessions_for_many_account_directories_share_one_http_client() {
    let root = tempfile::tempdir().unwrap();
    // 第一个目录建出共享的客户端。
    assert_eq!(status(&root.path().join("first"))["ok"], true);
    let before = open_descriptors();
    for index in 0..32 {
        let value = status(&root.path().join(format!("account-{index}")));
        assert_eq!(value["ok"], true, "{value}");
    }
    let after = open_descriptors();
    assert!(
        after <= before + 4,
        "32 account directories opened {} descriptors",
        after.saturating_sub(before)
    );
}
