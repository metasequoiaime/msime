use std::env;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    for path in [
        "resources/opencc.lock.json",
        "scripts/fetch_opencc.py",
        "scripts/download_retry.py",
        "resources/licenses/OpenCC-Apache-2.0.txt",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
    for name in [
        "MSIME_PYTHON",
        "MSIME_OPENCC_SOURCE_DIR",
        "MSIME_OPENCC_OFFLINE",
        "CARGO_NET_OFFLINE",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let candidates = env::var_os("MSIME_PYTHON")
        .map(|value| vec![value])
        .unwrap_or_else(|| vec!["python3".into(), "python".into()]);
    for python in candidates {
        let result = Command::new(&python)
            .arg(root.join("scripts/fetch_opencc.py"))
            .output();
        let output = match result {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => panic!("运行 OpenCC 构建输入工具失败：{error}"),
        };
        // Windows 的 python3 可能只是未安装解释器的系统别名。
        if output.status.code() == Some(9009) {
            continue;
        }
        assert!(
            output.status.success(),
            "准备 OpenCC 构建输入失败：{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let directory = String::from_utf8(output.stdout).expect("OpenCC path is UTF-8");
        let directory = directory.trim();
        // 所有宿主继续内嵌静态文本；下载只发生在构建主机。
        println!("cargo:rustc-env=MSIME_OPENCC_DIR={directory}");
        println!("cargo:rerun-if-changed={directory}");
        return;
    }
    panic!("构建需要 Python 3.10+；可用 MSIME_PYTHON 指定解释器路径");
}
