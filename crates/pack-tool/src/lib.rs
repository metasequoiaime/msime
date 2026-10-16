//! `msime-pack`: checks extension packs by exactly the rules the input method imports them by.
//!
//! ```text
//! msime-pack validate [--builtin] <pack folder or .zip>...
//! msime-pack run <pack folder or .zip> --events <fixture.jsonl> [--grant all|<capability>,...] [--builtin]
//! msime-pack hash [--builtin] <pack folder or .zip>
//! ```
//!
//! Every rule lives in `msime_client_services::plugins`: `validate` stages each pack the way `import` does and runs the same checks, so a pack this tool accepts is one the 插件 page installs, and the other way round. Nothing is installed and no plugins directory is touched. One line per pack, in the order given: `ok <id> <kind> <version>`, or `error <path>: <reason>` where the reason starts with the same failure code the settings page decodes (`plugin_invalid`, `plugin_archive`, ...) followed by the broken rule in Chinese. The exit status is 0 when every pack is valid, 1 when any is not, and 2 for a usage error.
//!
//! `run`（只在桌面）按夹具里的事件运行一个脚本插件，每个事件输出一行 JSON，细节见 `src/run.rs`（那个模块只在桌面编译）。退出码：全部事件都没有失败为 0；有任何失败（trap、fuel 耗尽、返回负数、初始化失败）或者包不合规为 1；命令行、`--grant` 或夹具写错为 2。
//!
//! `hash`（只在桌面）按 `validate` 的规则检查一个脚本包，合规时打印它的内容哈希（64 位小写十六进制，授权绑定在它上面），退出码同 `validate`。仓库的 `scripts/build-official-plugins.sh` 拿它和 `plugins/official-packs.json` 里锁定的哈希比对。
//!
//! `--builtin` 只给仓库自己的构建脚本用：随桌面安装包分发的官方插件包本来就用保留 id（`plugins::is_builtin`），不加它时这些 id 和导入一样报 `plugin_reserved`。其余规则一条不少。

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use msime_client_services::plugins::{self, PluginError, ReservedIds};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub mod run;

/// 成功：`validate` 给的包全部合规，或者 `run` 的每个事件都没有失败。
pub const EXIT_OK: i32 = 0;
/// 失败：`validate` 有包不合规；`run` 的包不合规，或者有事件失败（trap、fuel 耗尽、返回负数、初始化失败）。
pub const EXIT_INVALID: i32 = 1;
/// 用法错误：命令行看不懂，或者 `run` 的 `--grant` 写错、夹具读不出或写错。
pub const EXIT_USAGE: i32 = 2;

const USAGE: &str = "usage: msime-pack validate [--builtin] <pack folder or .zip>...\n       msime-pack run <pack folder or .zip> --events <fixture.jsonl> [--grant all|<capability>,...] [--builtin]\n       msime-pack hash [--builtin] <pack folder or .zip>";

/// 命令行里的 `--builtin`。
const BUILTIN_FLAG: &str = "--builtin";

/// 去掉开头的 `--builtin`，返回剩下的参数和保留 id 的处理方式。
fn reserved_flag(args: &[OsString]) -> (&[OsString], ReservedIds) {
    match args.split_first() {
        Some((flag, rest)) if flag == BUILTIN_FLAG => (rest, ReservedIds::Allow),
        _ => (args, ReservedIds::Refuse),
    }
}

/// Run the command line `args` (without the program name), writing the report to `out` and usage errors to `err`, and return the exit status.
pub fn run(args: &[OsString], out: &mut impl Write, err: &mut impl Write) -> i32 {
    match args.split_first() {
        Some((command, rest)) if command == "validate" => {
            let (sources, reserved) = reserved_flag(rest);
            if sources.is_empty()
                || sources
                    .iter()
                    .any(|source| source.is_empty() || source == BUILTIN_FLAG)
            {
                let _ = writeln!(err, "{USAGE}");
                return EXIT_USAGE;
            }
            let mut status = EXIT_OK;
            for source in sources {
                let source = Path::new(source);
                let line = match plugins::validate_with(source, reserved) {
                    Ok(summary) => format!(
                        "ok {} {} {}",
                        summary.id,
                        summary.kind().as_str(),
                        summary.version
                    ),
                    Err(error) => {
                        status = EXIT_INVALID;
                        format!("error {}: {}", source.display(), reason(&error))
                    }
                };
                if writeln!(out, "{line}").is_err() {
                    return EXIT_INVALID;
                }
            }
            status
        }
        Some((command, rest)) if command == "run" => run_command(rest, out, err),
        Some((command, rest)) if command == "hash" => hash_command(rest, out, err),
        Some((flag, rest)) if rest.is_empty() && (flag == "--help" || flag == "-h") => {
            let _ = writeln!(out, "{USAGE}");
            EXIT_OK
        }
        _ => {
            let _ = writeln!(err, "{USAGE}");
            EXIT_USAGE
        }
    }
}

/// `hash` 的参数：可选的 `--builtin` 和一个包。
fn hash_command(args: &[OsString], out: &mut impl Write, err: &mut impl Write) -> i32 {
    let (rest, reserved) = reserved_flag(args);
    let [pack] = rest else {
        let _ = writeln!(err, "{USAGE}");
        return EXIT_USAGE;
    };
    if pack.is_empty() || pack.to_string_lossy().starts_with("--") {
        let _ = writeln!(err, "{USAGE}");
        return EXIT_USAGE;
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        let pack = Path::new(pack);
        match plugins::load_script_source(pack, reserved) {
            Ok(loaded) => {
                if writeln!(out, "{}", loaded.pack.content_hash).is_err() {
                    return EXIT_INVALID;
                }
                EXIT_OK
            }
            Err(error) => {
                let _ = writeln!(out, "error {}: {}", pack.display(), reason(&error));
                EXIT_INVALID
            }
        }
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        let _ = (pack, reserved, out);
        let _ = writeln!(err, "msime-pack hash 只在 macOS、Windows 和 Linux 上可用");
        EXIT_USAGE
    }
}

/// `run` 的参数：一个包，`--events` 必填，`--grant` 和 `--builtin` 可选，各出现一次，顺序不限。
fn run_command(args: &[OsString], out: &mut impl Write, err: &mut impl Write) -> i32 {
    let mut pack = None;
    let mut events = None;
    let mut grant = None;
    let mut reserved = ReservedIds::Refuse;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == BUILTIN_FLAG && reserved == ReservedIds::Refuse {
            reserved = ReservedIds::Allow;
            continue;
        }
        let slot = if arg == "--events" {
            &mut events
        } else if arg == "--grant" {
            &mut grant
        } else if pack.is_none() && !arg.is_empty() && !arg.to_string_lossy().starts_with("--") {
            pack = Some(arg.clone());
            continue;
        } else {
            let _ = writeln!(err, "{USAGE}");
            return EXIT_USAGE;
        };
        match args.next() {
            Some(value) if slot.is_none() && !value.is_empty() => *slot = Some(value.clone()),
            _ => {
                let _ = writeln!(err, "{USAGE}");
                return EXIT_USAGE;
            }
        }
    }
    let (Some(pack), Some(events)) = (pack, events) else {
        let _ = writeln!(err, "{USAGE}");
        return EXIT_USAGE;
    };
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        let grant = match grant {
            None => run::GrantSpec::Nothing,
            Some(text) => match text.to_str() {
                Some(text) => run::GrantSpec::parse(text),
                None => {
                    let _ = writeln!(err, "{USAGE}");
                    return EXIT_USAGE;
                }
            },
        };
        run::command(
            Path::new(&pack),
            Path::new(&events),
            &grant,
            reserved,
            out,
            err,
        )
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        let _ = (pack, events, grant, reserved, out);
        let _ = writeln!(err, "msime-pack run 只在 macOS、Windows 和 Linux 上可用");
        EXIT_USAGE
    }
}

/// The failure code and, for the codes that carry no detail, what it means for an author.
pub(crate) fn reason(error: &PluginError) -> String {
    match error {
        PluginError::UnsupportedSource => {
            "plugin_unsupported_source: 只接受插件文件夹或 .zip 文件".to_owned()
        }
        PluginError::Reserved => "plugin_reserved: 这个 id 属于内置插件，不能导入".to_owned(),
        other => other.to_string(),
    }
}
