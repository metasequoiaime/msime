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
fn prepared_options_fit_a_192_kib_thread_stack() {
    const CHILD: &str = "MSIME_HOST_OPTIONS_SMALL_STACK_CHILD";
    const COMPLETE: &str = "HOST_OPTIONS_SMALL_STACK_OK";
    if std::env::var_os(CHILD).is_none() {
        // 栈溢出会终止进程；隔离在子进程里，父测试仍能报告真实失败。
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "prepared_options_fit_a_192_kib_thread_stack",
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

    // 记事本的主线程栈是 512 KiB，系统与 TSF 调用链在下面还要占掉一截；Unreal 编辑器在建窗时激活输入法的那条线程更小，`#11893` 里 141 KiB 的建会话帧就把剩下的栈吃到不足一层 42 KiB 的 `InputSession`。预算收到 192 KiB：既留下那半条链，又钉住建会话不许再把大结构体摊在栈上。
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
        .stack_size(192 * 1024)
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
