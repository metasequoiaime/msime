use msime_client_core::preferences::Preferences;
use msime_host_api::{msime_client_create, msime_client_string_free};
use serde_json::json;
use std::ffi::CStr;

// 只在真实 C ABI 的指针读取与配对释放中局部允许 unsafe。
#[allow(unsafe_code)]
fn call(document: &serde_json::Value) -> serde_json::Value {
    let bytes = serde_json::to_vec(document).unwrap();
    // 字节在调用期间有效，返回值只由本次 ABI 配对释放。
    unsafe {
        let raw = msime_client_create(bytes.as_ptr(), bytes.len());
        assert!(!raw.is_null());
        let reply = CStr::from_ptr(raw).to_bytes().to_vec();
        msime_client_string_free(raw);
        serde_json::from_slice(&reply).unwrap()
    }
}

#[test]
fn prepared_options_fit_a_256_kib_thread_stack() {
    const CHILD: &str = "MSIME_HOST_OPTIONS_SMALL_STACK_CHILD";
    const COMPLETE: &str = "HOST_OPTIONS_SMALL_STACK_OK";
    if std::env::var_os(CHILD).is_none() {
        // 栈溢出会终止进程；隔离在子进程里，父测试仍能报告真实失败。
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "prepared_options_fit_a_256_kib_thread_stack",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "small-stack child failed: {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(COMPLETE));
        return;
    }

    // 记事本只有 512 KiB 主线程栈；给系统和 TSF 调用链留一半，完整偏好仍须可解析。
    let defaults = serde_json::to_value(Preferences::default()).unwrap();
    let document = json!({
        "api_version": 999,
        "resources": "synthetic-resources",
        "user_data": "synthetic-user",
        "cache": "synthetic-cache",
        "dictionaries": "synthetic-dictionaries",
        "preferences": defaults,
    });
    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            // API 版本在完整配置解析后检查；用拒绝路径避免访问真实词库和用户目录。
            assert_eq!(
                call(&document),
                json!({"ok": false, "error": "unsupported host API version"})
            );
            let mut unknown_edition = document;
            unknown_edition["edition"] = json!("synthetic-unknown-edition");
            assert_eq!(
                call(&unknown_edition),
                json!({"ok": false, "error": "invalid options document"})
            );
        })
        .unwrap()
        .join()
        .unwrap();
    println!("{COMPLETE}");
}
