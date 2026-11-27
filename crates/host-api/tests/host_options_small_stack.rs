use msime_client_core::preferences::Preferences;
use msime_host_api::{msime_client_create, msime_client_destroy, msime_client_string_free};
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

// 销毁也必须在创建会话的同一线程上完成。
#[allow(unsafe_code)]
fn destroy(session: u64) -> serde_json::Value {
    unsafe {
        let raw = msime_client_destroy(session);
        assert!(!raw.is_null());
        let reply = CStr::from_ptr(raw).to_bytes().to_vec();
        msime_client_string_free(raw);
        serde_json::from_slice(&reply).unwrap()
    }
}

#[test]
fn session_creation_fits_a_96_kib_thread_stack() {
    const CHILD: &str = "MSIME_HOST_OPTIONS_SMALL_STACK_CHILD";
    const COMPLETE: &str = "HOST_OPTIONS_SMALL_STACK_OK";
    if std::env::var_os(CHILD).is_none() {
        // 栈溢出会终止进程；隔离在子进程里，父测试仍能报告真实失败。
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "session_creation_fits_a_96_kib_thread_stack",
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

    // 96 KiB 就是 Unreal 编辑器建窗那条线程的整条栈（#11893 的两个 dmp 里 RSP 到栈顶是 85,704–86,536 字节），构造链自己就要 110 KiB，宿主还在上面压着 MSCTF、user32 的十几 KiB。这条测试钉的是：建会话在这么小的线程栈上也要跑得通，靠的是 `ffi::session::create_session` 在剩余栈不够时切到自己的栈上，而不是靠不断削帧。
    let defaults = serde_json::to_value(Preferences::default()).unwrap();
    let document = json!({
        "api_version": 999,
        "resources": "synthetic-resources",
        "user_data": "synthetic-user",
        "cache": "synthetic-cache",
        "dictionaries": "synthetic-dictionaries",
        "preferences": defaults,
    });
    let root = tempfile::tempdir().unwrap();
    for name in ["resources", "user", "cache", "dictionaries"] {
        std::fs::create_dir(root.path().join(name)).unwrap();
    }
    // 空资源目录会跳过 SQLite 打开路径；使用合成词库覆盖真实装机时的构造深度。
    for directory in ["resources", "dictionaries"] {
        rusqlite::Connection::open(root.path().join(directory).join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni','n','合成',100);
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
    }
    let mut valid = document.clone();
    valid["api_version"] = json!(1);
    for (field, directory) in [
        ("resources", "resources"),
        ("user_data", "user"),
        ("cache", "cache"),
        ("dictionaries", "dictionaries"),
    ] {
        valid[field] = json!(root.path().join(directory));
    }
    std::thread::Builder::new()
        .stack_size(96 * 1024)
        .spawn(move || {
            // API 版本在完整配置解析后检查；用拒绝路径避免访问真实词库和用户目录。
            assert_eq!(
                call(&document),
                json!({"ok": false, "error": "unsupported host API version"})
            );
            // 不能只验解析后的拒绝路径：真实编辑器还会在同一小栈上构造引擎会话。
            let created = call(&valid);
            assert_eq!(created["ok"], true, "{created}");
            let session = created["value"]["session"].as_u64().unwrap();
            assert_eq!(destroy(session)["ok"], true);
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
