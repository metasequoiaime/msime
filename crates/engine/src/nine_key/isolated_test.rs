use std::process::Command;

const CHILD: &str = "MSIME_ISOLATED_NINE_KEY_TEST";
const COMPLETE: &str = "MSIME_ISOLATED_TEST_COMPLETE:";

fn harness_name(qualified_name: &str) -> &str {
    // libtest 的目标名不包含 crate 前缀。
    qualified_name.split_once("::").unwrap().1
}

pub(super) fn run(qualified_name: &str, test: impl FnOnce()) -> Result<(), String> {
    let name = harness_name(qualified_name);
    match std::env::var(CHILD) {
        Ok(target) if target == name => {
            test();
            // 只有完整测试体成功返回才确认，零目标和提前失败都不能伪装成功。
            eprintln!("{COMPLETE}{name}");
            Ok(())
        }
        Err(std::env::VarError::NotPresent) => run_child(name),
        _ => Err("隔离测试的子进程目标标识不匹配".to_owned()),
    }
}

fn run_child(name: &str) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let output = Command::new(executable)
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .map_err(|error| format!("无法启动隔离测试：{error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let details = || format!("{}\nstdout:\n{stdout}\nstderr:\n{stderr}", output.status);
    if !output.status.success() {
        return Err(format!("隔离测试失败：{}", details()));
    }
    let marker = format!("{COMPLETE}{name}");
    if !stderr.lines().any(|line| line == marker) {
        return Err(format!("未收到隔离测试完成标记：{}", details()));
    }
    Ok(())
}

#[test]
fn a_missing_child_target_is_not_success() {
    let failure = run_child("missing_isolated_nine_key_test").unwrap_err();
    assert!(failure.contains("未收到隔离测试完成标记"), "{failure}");
}

#[test]
fn child_assertion_failures_reach_the_parent() {
    let qualified_name = concat!(
        module_path!(),
        "::child_assertion_failures_reach_the_parent"
    );
    let name = harness_name(qualified_name);
    if std::env::var(CHILD).as_deref() == Ok(name) {
        run(qualified_name, || panic!("合成子进程断言失败")).unwrap();
    } else {
        let failure = run_child(name).unwrap_err();
        assert!(failure.contains("隔离测试失败"), "{failure}");
        assert!(failure.contains("合成子进程断言失败"), "{failure}");
    }
}
