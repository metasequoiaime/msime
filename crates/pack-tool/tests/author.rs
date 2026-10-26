//! 仓库外的作者走的那条路：`msime-pack new` 生成插件 crate，`build` 编译、放进 `pack/` 并校验，`run` 按生成的夹具运行，`pack` 打出确定的压缩包。
//!
//! 生成的 crate 依赖 crates.io 上的 SDK；测试里把这一行换成仓库里 SDK 的路径依赖，不联网，测的也是这一份源码。编译要 `wasm32-unknown-unknown` 目标：没有时这部分打印 skip 跳过，CI 设 `MSIME_REQUIRE_SCRIPT_PLUGINS=1`，缺目标就失败（同 `scripts/test-script-plugins.sh`）。

#![cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]

use std::ffi::OsString;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use msime_pack_tool::scaffold::SDK_REQUIREMENT;
use msime_pack_tool::{run, EXIT_INVALID, EXIT_OK, EXIT_USAGE};
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run the command line in-process and return the exit status, standard output and standard error.
fn invoke(args: &[&dyn AsRef<std::ffi::OsStr>]) -> (i32, String, String) {
    let args: Vec<OsString> = args.iter().map(|arg| arg.as_ref().to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, &mut out, &mut err);
    (
        status,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// `rust-toolchain.toml` 钉住的工具链。生成的 crate 在临时目录里，不在仓库下，rustup 不会替它选这一套，所以子进程显式指定。
fn toolchain() -> String {
    let text = fs::read_to_string(repo().join("rust-toolchain.toml")).unwrap();
    let table: toml::Table = text.parse().unwrap();
    table["toolchain"]["channel"].as_str().unwrap().to_owned()
}

/// 钉住的工具链有没有 wasm 目标。没有时：CI（`MSIME_REQUIRE_SCRIPT_PLUGINS=1`）里失败，别处打印 skip。
fn wasm_target() -> bool {
    let output = Command::new("rustc")
        .args(["--print", "sysroot"])
        .env("RUSTUP_TOOLCHAIN", toolchain())
        .output();
    let installed = output.is_ok_and(|output| {
        output.status.success()
            && Path::new(String::from_utf8_lossy(&output.stdout).trim())
                .join("lib/rustlib/wasm32-unknown-unknown")
                .is_dir()
    });
    if !installed {
        let message = format!(
            "wasm32-unknown-unknown is not installed for {0} (rustup target add wasm32-unknown-unknown --toolchain {0})",
            toolchain()
        );
        assert!(
            std::env::var("MSIME_REQUIRE_SCRIPT_PLUGINS").as_deref() != Ok("1"),
            "{message}"
        );
        eprintln!("skip: {message}");
    }
    installed
}

/// 把生成的 crates.io 依赖换成仓库里 SDK 的路径依赖。
fn use_repo_sdk(crate_dir: &Path) {
    let manifest = crate_dir.join("Cargo.toml");
    let text = fs::read_to_string(&manifest).unwrap();
    let registry = format!("msime-script-sdk = \"{SDK_REQUIREMENT}\"");
    assert!(text.contains(&registry), "{text}");
    let sdk = fs::canonicalize(repo().join("crates/script-sdk")).unwrap();
    // TOML 的字面量字符串：Windows 路径里的反斜线不用转义。
    let local = format!("msime-script-sdk = {{ path = '{}' }}", sdk.display());
    fs::write(&manifest, text.replace(&registry, &local)).unwrap();
}

/// 用编出来的 msime-pack 跑 `build`：它要起 cargo，cargo 的输出直接打在测试的标准错误上。
fn build(crate_dir: &Path) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_msime-pack"))
        .arg("build")
        .arg(crate_dir)
        .env("RUSTUP_TOOLCHAIN", toolchain())
        .env("CARGO_NET_OFFLINE", "true")
        // 环境里的 RUSTFLAGS 会顶掉 .cargo/config.toml 的 rustflags（64 KiB 栈）。
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .output()
        .unwrap();
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

/// `run` 的输出，一行一个 JSON。
fn run_fixture(crate_dir: &Path) -> Vec<Value> {
    let (status, out, err) = invoke(&[
        &"run",
        &crate_dir.join("pack"),
        &"--events",
        &crate_dir.join("fixture.jsonl"),
        &"--grant",
        &"all",
    ]);
    assert_eq!((status, err.as_str()), (EXIT_OK, ""), "{out}");
    out.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn logs(line: &Value) -> Vec<&str> {
    line["log"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["text"].as_str().unwrap())
        .collect()
}

fn actions(line: &Value) -> Vec<(&str, &str)> {
    line["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|action| {
            (
                action["import"].as_str().unwrap(),
                action["result"].as_str().unwrap(),
            )
        })
        .collect()
}

/// 压缩包里的一个条目。
struct Entry {
    name: String,
    stored: bool,
    date: (u16, u8, u8),
    mode: Option<u32>,
}

fn entries(archive: &[u8]) -> Vec<Entry> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    (0..zip.len())
        .map(|index| {
            let entry = zip.by_index(index).unwrap();
            let time = entry.last_modified().unwrap();
            Entry {
                name: entry.name().unwrap().into_owned(),
                stored: entry.compression() == zip::CompressionMethod::Stored,
                date: (time.year(), time.month(), time.day()),
                mode: entry.unix_mode().map(|mode| mode & 0o777),
            }
        })
        .collect()
}

#[test]
fn the_scaffold_asks_for_the_sdk_version_the_crate_has() {
    let text = fs::read_to_string(repo().join("crates/script-sdk/Cargo.toml")).unwrap();
    let manifest: toml::Table = text.parse().unwrap();
    let version = manifest["package"]["version"].as_str().unwrap();
    assert!(
        version.starts_with(&format!("{SDK_REQUIREMENT}.")),
        "SDK {version} does not satisfy {SDK_REQUIREMENT}"
    );
}

#[test]
fn new_refuses_bad_ids_reserved_ids_and_occupied_directories() {
    let root = tempfile::tempdir().unwrap();
    for id in ["Upper", "_under", "has space", "a/b"] {
        let dir = root.path().join("x");
        let (status, _, err) = invoke(&[&"new", &id, &"--dir", &dir]);
        assert_eq!(status, EXIT_INVALID, "{id}");
        assert!(err.contains("插件 id 无效"), "{err}");
        assert!(!dir.exists());
    }
    // 官方插件的 id 导入时会被拒绝。
    let (status, _, err) = invoke(&[&"new", &"uuid", &"--dir", &root.path().join("uuid")]);
    assert_eq!(status, EXIT_INVALID);
    assert!(err.contains("内置插件"), "{err}");

    let occupied = root.path().join("occupied");
    fs::create_dir(&occupied).unwrap();
    fs::write(occupied.join("keep.txt"), "mine").unwrap();
    let (status, _, err) = invoke(&[&"new", &"dice", &"--dir", &occupied]);
    assert_eq!(status, EXIT_INVALID);
    assert!(err.contains("不是空的"), "{err}");
    assert_eq!(fs::read_dir(&occupied).unwrap().count(), 1);

    // 空目录可以用。
    let empty = root.path().join("empty");
    fs::create_dir(&empty).unwrap();
    let (status, out, _) = invoke(&[&"new", &"dice", &"--dir", &empty]);
    assert_eq!(status, EXIT_OK);
    assert!(out.starts_with("created "), "{out}");
}

#[test]
fn new_takes_the_command_trigger_from_the_letters_of_the_id() {
    let root = tempfile::tempdir().unwrap();
    let manifest = |id: &str| -> Option<toml::Table> {
        let dir = root.path().join(id);
        let (status, _, _) = invoke(&[&"new", &id, &"--dir", &dir]);
        (status == EXIT_OK).then(|| {
            fs::read_to_string(dir.join("pack/plugin.toml"))
                .unwrap()
                .parse()
                .unwrap()
        })
    };
    let command = |table: &toml::Table| {
        let command = &table["script"]["commands"][0];
        (
            command["trigger"].as_str().unwrap().to_owned(),
            command["title"].as_str().unwrap().to_owned(),
        )
    };
    let dice = manifest("my-dice2").unwrap();
    assert_eq!(command(&dice), ("mydice".into(), "mydice".into()));
    assert_eq!(dice["id"].as_str(), Some("my-dice2"));
    assert_eq!(dice["script"]["module"].as_str(), Some("my-dice2.wasm"));
    // 触发词最多 16 个字母，标题最多 8 个字符。
    let long = manifest("abcdefghijklmnopqrstuvwxyz").unwrap();
    assert_eq!(
        command(&long),
        ("abcdefghijklmnop".into(), "abcdefgh".into())
    );
    // 不到两个字母，或者和内置指令重名：推不出触发词。
    for id in ["a1", "date", "x-2"] {
        let dir = root.path().join(id);
        let (status, _, err) = invoke(&[&"new", &id, &"--dir", &dir]);
        assert_eq!(status, EXIT_INVALID, "{id}");
        assert!(err.contains("触发词"), "{err}");
        assert!(!dir.exists());
    }
    // 别的钩子不需要触发词。
    let dir = root.path().join("a1-cands");
    let (status, _, _) = invoke(&[&"new", &"a1", &"--hook", &"candidates", &"--dir", &dir]);
    assert_eq!(status, EXIT_OK);

    // 包名里不能有点，也不能以数字开头。
    let dir = root.path().join("dotted");
    let (status, _, _) = invoke(&[&"new", &"9.dice", &"--dir", &dir]);
    assert_eq!(status, EXIT_OK);
    let cargo: toml::Table = fs::read_to_string(dir.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(cargo["package"]["name"].as_str(), Some("plugin-9-dice"));
}

#[test]
fn new_build_and_pack_reject_malformed_command_lines() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("x");
    for args in [
        vec!["new"],
        vec!["new", "dice", "--hook"],
        vec!["new", "dice", "--hook", "commit"],
        vec!["new", "dice", "--dir", "a", "--dir", "b"],
        vec!["new", "dice", "extra"],
        vec!["build", "a", "b"],
        vec!["build", "--release"],
        vec!["pack", "a", "b"],
        vec!["pack", "-o"],
        vec!["pack", "-o", "a.zip", "-o", "b.zip"],
    ] {
        let args: Vec<&dyn AsRef<std::ffi::OsStr>> = args
            .iter()
            .map(|arg| arg as &dyn AsRef<std::ffi::OsStr>)
            .collect();
        let (status, _, err) = invoke(&args);
        assert_eq!(status, EXIT_USAGE, "{err}");
    }
    assert!(!dir.exists());
    // 不是插件 crate 的目录。
    let (status, _, err) = invoke(&[&"build", &root.path()]);
    assert_eq!(status, EXIT_USAGE);
    assert!(err.contains("不是插件 crate"), "{err}");
}

#[test]
fn every_scaffold_builds_validates_runs_and_packs() {
    let root = tempfile::tempdir().unwrap();
    let wasm = wasm_target();
    for hook in ["candidates", "command", "before_commit"] {
        let id = format!("hello-{}", hook.replace('_', "-"));
        let dir = root.path().join(&id);
        let (status, out, err) = invoke(&[&"new", &id, &"--hook", &hook, &"--dir", &dir]);
        assert_eq!((status, err.as_str()), (EXIT_OK, ""), "{out}");
        for file in [
            "Cargo.toml",
            ".cargo/config.toml",
            ".gitignore",
            "README.md",
            "src/lib.rs",
            "pack/plugin.toml",
            "pack/LICENSE.txt",
            "fixture.jsonl",
        ] {
            let text = fs::read_to_string(dir.join(file)).unwrap();
            assert!(!text.contains("{{"), "{file} still has a placeholder");
        }
        if !wasm {
            continue;
        }
        use_repo_sdk(&dir);
        let (status, out) = build(&dir);
        assert_eq!(status, EXIT_OK, "{out}");
        assert!(
            out.contains(&format!("built pack/{id}.wasm: module ")),
            "{out}"
        );
        // 64 KiB 的栈占第一页，静态数据在第二页：和清单写的 2 页一致。
        assert!(out.contains(" memory 2/2 pages\n"), "{out}");
        assert!(out.ends_with(&format!("ok {id} script 0.1.0\n")), "{out}");

        let lines = run_fixture(&dir);
        assert!(lines.iter().all(|line| line["status"] == "ok"), "{lines:?}");
        match hook {
            "candidates" => {
                // 第一个窗口里「你」「呢」是单字，第二个窗口没有单字。
                assert_eq!(
                    actions(&lines[0]),
                    [("candidates_annotate", "ok"), ("candidates_annotate", "ok")]
                );
                assert_eq!(logs(&lines[0]), ["annotated"]);
                assert!(actions(&lines[1]).is_empty());
                assert_eq!(logs(&lines[1]), ["keep"]);
            }
            "command" => {
                let rows: Vec<&Value> = lines.iter().map(|line| &line["rows"]).collect();
                // 全 0 的随机数是 1；第二个事件的第一个字节 0xfe 被丢掉，第二个字节 4 得 5。
                assert_eq!(
                    rows,
                    [
                        &serde_json::json!([["1", "掷骰"]]),
                        &serde_json::json!([["5", "掷骰"]]),
                        &serde_json::json!([["HELLO", "大写"]]),
                    ]
                );
            }
            "before_commit" => {
                assert_eq!(actions(&lines[0]), [("rewrite_commit", "ok")]);
                assert_eq!(logs(&lines[0]), ["等一下……"]);
                assert!(actions(&lines[1]).is_empty());
                assert_eq!(logs(&lines[1]), ["keep"]);
                // 网址模式下不可改写。
                assert!(actions(&lines[2]).is_empty());
                assert_eq!(logs(&lines[2]), ["skip: not rewritable"]);
            }
            _ => unreachable!(),
        }

        // 默认输出在 crate 根目录；同样的文件打两次，字节相同。
        let (status, out, err) = invoke(&[&"pack", &dir]);
        assert_eq!((status, err.as_str()), (EXIT_OK, ""), "{out}");
        let default = dir.join(format!("{id}-0.1.0.zip"));
        assert!(out.ends_with(&format!("ok {id} script 0.1.0\n")), "{out}");
        let again = root.path().join(format!("{id}-again.zip"));
        let (status, _, _) = invoke(&[&"pack", &dir.join("pack"), &"-o", &again]);
        assert_eq!(status, EXIT_OK);
        let archive = fs::read(&default).unwrap();
        assert_eq!(archive, fs::read(&again).unwrap());
        let names: Vec<String> = entries(&archive)
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, ["LICENSE.txt", &format!("{id}.wasm"), "plugin.toml"]);
        // 压缩包和文件夹是同一个包：内容哈希相同。
        let (_, folder_hash, _) = invoke(&[&"hash", &dir.join("pack")]);
        let (_, zip_hash, _) = invoke(&[&"hash", &default]);
        assert_eq!(folder_hash, zip_hash);
    }
}

#[test]
fn pack_is_deterministic_and_leaves_hidden_files_out() {
    let root = tempfile::tempdir().unwrap();
    // 文档里的音效包模板：pack 不限于脚本插件。
    let template = repo().join("docs/plugin-template");
    let copy = root.path().join("keys");
    fs::create_dir(&copy).unwrap();
    for entry in fs::read_dir(&template).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), copy.join(entry.file_name())).unwrap();
    }
    fs::write(copy.join(".DS_Store"), "finder").unwrap();

    let first = root.path().join("first.zip");
    let (status, out, err) = invoke(&[&"pack", &template, &"-o", &first]);
    assert_eq!((status, err.as_str()), (EXIT_OK, ""), "{out}");
    assert!(out.ends_with("ok example-keys sound 1.0.0\n"), "{out}");
    // 默认输出在包文件夹旁边：<id>-<version>.zip。
    let (status, _, _) = invoke(&[&"pack", &copy]);
    assert_eq!(status, EXIT_OK);
    let second = root.path().join("example-keys-1.0.0.zip");
    let archive = fs::read(&first).unwrap();
    assert_eq!(archive, fs::read(&second).unwrap());

    let listed = entries(&archive);
    let mut names: Vec<&str> = listed.iter().map(|entry| entry.name.as_str()).collect();
    assert!(!names.contains(&".DS_Store"));
    let unsorted = names.clone();
    names.sort_unstable();
    assert_eq!(names, unsorted);
    for entry in &listed {
        assert!(entry.stored, "{}", entry.name);
        assert_eq!(entry.date, (1980, 1, 1), "{}", entry.name);
        assert_eq!(entry.mode, Some(0o644), "{}", entry.name);
    }

    // 压缩包不能放进包文件夹里，否则下一次打包会把它打进去。
    let (status, _, err) = invoke(&[&"pack", &copy, &"-o", &copy.join("keys.zip")]);
    assert_eq!(status, EXIT_INVALID);
    assert!(err.contains("不能放在插件包文件夹"), "{err}");
    assert!(!copy.join("keys.zip").exists());

    // 不合规的包不打。
    let broken = root.path().join("broken");
    fs::create_dir(&broken).unwrap();
    fs::write(broken.join("plugin.toml"), "schema_version = 1\n").unwrap();
    let (status, out, _) = invoke(&[&"pack", &broken, &"-o", &root.path().join("broken.zip")]);
    assert_eq!(status, EXIT_INVALID);
    assert!(out.starts_with("error "), "{out}");
    assert!(!root.path().join("broken.zip").exists());
}
