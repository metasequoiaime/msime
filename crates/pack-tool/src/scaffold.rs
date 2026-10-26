//! `msime-pack new`：生成一个在仓库外面也能独立构建的脚本插件 crate。
//!
//! 生成的 crate 依赖 crates.io 上的 SDK `msime-script-sdk`，带默认目标和 64 KiB 栈的 `.cargo/config.toml`、一个按所选钩子能跑通的 `src/lib.rs`、schema 2 的清单（只申请这个例子用到的能力）、占位的 `pack/LICENSE.txt`、给 `msime-pack run` 的夹具和一份简短的说明。模板都在 `templates/` 下，用 `include_str!` 编进二进制。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use msime_client_base::text::safe_id;
use msime_client_services::plugins::{self, script_pack, PluginKind};

use crate::{EXIT_INVALID, EXIT_OK};

/// 生成的 `Cargo.toml` 依赖的 SDK 版本要求，跟着 `crates/script-sdk/Cargo.toml` 的版本走（`tests/author.rs` 核对两边一致）。
pub const SDK_REQUIREMENT: &str = "0.1";

/// `--hook` 不写时生成的例子。
pub const DEFAULT_HOOK: Hook = Hook::Command;

/// `--hook` 的取值：生成哪种例子。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    /// 候选窗口：给单字加注释（`read_candidates`、`annotate_candidates`）。
    Candidates,
    /// `/` 指令：掷骰子或者转大写（`provide_commands`）。
    Command,
    /// 上屏前改写：`...` 换成 `……`（`read_commit`、`rewrite_commit`）。
    BeforeCommit,
}

impl Hook {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "candidates" => Some(Self::Candidates),
            "command" => Some(Self::Command),
            "before_commit" => Some(Self::BeforeCommit),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidates => "candidates",
            Self::Command => "command",
            Self::BeforeCommit => "before_commit",
        }
    }

    /// 这个例子的 `src/lib.rs`、`pack/plugin.toml`、`fixture.jsonl` 和说明里的一句话。
    fn files(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::Candidates => (
                include_str!("../templates/candidates/lib.rs"),
                include_str!("../templates/candidates/plugin.toml"),
                include_str!("../templates/candidates/fixture.jsonl"),
                "现在它给候选窗口里的单字加注释「单字」。",
            ),
            Self::Command => (
                include_str!("../templates/command/lib.rs"),
                include_str!("../templates/command/plugin.toml"),
                include_str!("../templates/command/fixture.jsonl"),
                "现在它在 `/` 指令里出候选：只打触发词时掷骰子，后面跟着字母时给出它们的大写。",
            ),
            Self::BeforeCommit => (
                include_str!("../templates/before_commit/lib.rs"),
                include_str!("../templates/before_commit/plugin.toml"),
                include_str!("../templates/before_commit/fixture.jsonl"),
                "现在它在上屏前把 `...` 换成 `……`。",
            ),
        }
    }
}

/// 生成到 `directory`（不写时是当前目录下的 `<id>`），成功时在 `out` 上报告生成的目录和下一步。
pub fn command(
    id: &str,
    hook: Hook,
    directory: Option<&Path>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> i32 {
    if !safe_id(id) {
        let _ = writeln!(
            err,
            "error: 插件 id 无效：只能用小写字母、数字、`.`、`-` 和 `_`，以字母或数字开头，不超过 64 个字符"
        );
        return EXIT_INVALID;
    }
    if plugins::is_builtin(PluginKind::Script, id) {
        let _ = writeln!(err, "error: {id} 是内置插件的 id，导入时会被拒绝，换一个");
        return EXIT_INVALID;
    }
    let trigger = match hook {
        Hook::Command => match trigger_for(id) {
            Some(trigger) => trigger,
            None => {
                let _ = writeln!(
                    err,
                    "error: 从 id 推不出 / 指令的触发词：触发词取 id 里的小写字母（最多 16 个），至少要 2 个，也不能和内置指令重名。换一个 id，或者用 --hook candidates、--hook before_commit"
                );
                return EXIT_INVALID;
            }
        },
        _ => String::new(),
    };
    let directory = directory.map_or_else(|| PathBuf::from(id), Path::to_path_buf);
    if fs::read_dir(&directory).is_ok_and(|mut entries| entries.next().is_some()) {
        let _ = writeln!(err, "error: {} 已经存在而且不是空的", directory.display());
        return EXIT_INVALID;
    }
    if let Err(error) = write_crate(&directory, id, hook, &trigger) {
        let _ = writeln!(err, "error: 写不进 {}：{error}", directory.display());
        return EXIT_INVALID;
    }
    let _ = writeln!(
        out,
        "created {} ({} plugin {id})\nnext: cd {} && msime-pack build",
        directory.display(),
        hook.as_str(),
        directory.display()
    );
    EXIT_OK
}

/// `/` 指令的触发词：id 里的小写字母，最多取 16 个；不到 2 个或者和内置指令重名时没有。
fn trigger_for(id: &str) -> Option<String> {
    let letters: String = id
        .chars()
        .filter(char::is_ascii_lowercase)
        .take(*script_pack::COMMAND_TRIGGER_BYTES.end())
        .collect();
    (script_pack::COMMAND_TRIGGER_BYTES.contains(&letters.len())
        && !script_pack::BUILTIN_COMMAND_TRIGGERS.contains(&letters.as_str()))
    .then_some(letters)
}

/// Cargo 的包名：`.` 不能出现在包名里，换成 `-`；包名也不能以数字开头，这时加上 `plugin-`。
fn package_name(id: &str) -> String {
    let name = id.replace('.', "-");
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        format!("plugin-{name}")
    } else {
        name
    }
}

fn write_crate(directory: &Path, id: &str, hook: Hook, trigger: &str) -> io::Result<()> {
    let (source, manifest, fixture, summary) = hook.files();
    let module = format!("{id}.wasm");
    // 指令的标题取触发词的前 8 个字母（标题最多 8 个字符）。
    let title: String = trigger
        .chars()
        .take(*script_pack::COMMAND_TITLE_CHARS.end())
        .collect();
    let fill = |template: &str| {
        template
            .replace("{{id}}", id)
            .replace("{{package}}", &package_name(id))
            .replace("{{module}}", &module)
            .replace("{{trigger}}", trigger)
            .replace("{{title}}", &title)
            .replace("{{hook}}", hook.as_str())
            .replace("{{summary}}", summary)
            .replace("{{sdk}}", SDK_REQUIREMENT)
    };
    for folder in [".cargo", "src", "pack"] {
        fs::create_dir_all(directory.join(folder))?;
    }
    let files = [
        (
            "Cargo.toml",
            fill(include_str!("../templates/Cargo.toml.in")),
        ),
        (
            ".cargo/config.toml",
            include_str!("../templates/cargo-config.toml").to_owned(),
        ),
        (
            ".gitignore",
            include_str!("../templates/gitignore").to_owned(),
        ),
        ("README.md", fill(include_str!("../templates/README.md.in"))),
        ("src/lib.rs", fill(source)),
        ("pack/plugin.toml", fill(manifest)),
        (
            "pack/LICENSE.txt",
            include_str!("../templates/LICENSE.txt").to_owned(),
        ),
        ("fixture.jsonl", fill(fixture)),
    ];
    for (name, content) in files {
        fs::write(directory.join(name), content)?;
    }
    Ok(())
}
