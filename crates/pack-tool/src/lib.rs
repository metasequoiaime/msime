//! `msime-pack`: checks extension packs by exactly the rules the input method imports them by.
//!
//! ```text
//! msime-pack validate <pack folder or .zip>...
//! msime-pack run <pack folder or .zip> --events <fixture.jsonl> [--grant all|<capability>,...]
//! ```
//!
//! Every rule lives in `msime_client_core::plugins`: `validate` stages each pack the way `import` does and runs the same checks, so a pack this tool accepts is one the 插件 page installs, and the other way round. Nothing is installed and no plugins directory is touched. One line per pack, in the order given: `ok <id> <kind> <version>`, or `error <path>: <reason>` where the reason starts with the same failure code the settings page decodes (`plugin_invalid`, `plugin_archive`, ...) followed by the broken rule in Chinese. The exit status is 0 when every pack is valid, 1 when any is not, and 2 for a usage error.
//!
//! `run`（只在桌面）按夹具里的事件运行一个脚本插件，每个事件输出一行 JSON，细节见 `src/run.rs`（那个模块只在桌面编译）。退出码：全部事件都没有失败为 0；有任何失败（trap、fuel 耗尽、返回负数、初始化失败）或者包不合规为 1；命令行、`--grant` 或夹具写错为 2。

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use msime_client_core::plugins::{self, PluginError};

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

const USAGE: &str = "usage: msime-pack validate <pack folder or .zip>...\n       msime-pack run <pack folder or .zip> --events <fixture.jsonl> [--grant all|<capability>,...]";

/// Run the command line `args` (without the program name), writing the report to `out` and usage errors to `err`, and return the exit status.
pub fn run(args: &[OsString], out: &mut impl Write, err: &mut impl Write) -> i32 {
    match args.split_first() {
        Some((command, sources)) if command == "validate" && !sources.is_empty() => {
            if sources.iter().any(|source| source.is_empty()) {
                let _ = writeln!(err, "{USAGE}");
                return EXIT_USAGE;
            }
            let mut status = EXIT_OK;
            for source in sources {
                let source = Path::new(source);
                let line = match plugins::validate(source) {
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

/// `run` 的参数：一个包，`--events` 必填，`--grant` 可选，各出现一次，顺序不限。
fn run_command(args: &[OsString], out: &mut impl Write, err: &mut impl Write) -> i32 {
    let mut pack = None;
    let mut events = None;
    let mut grant = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
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
        run::command(Path::new(&pack), Path::new(&events), &grant, out, err)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        let _ = (pack, events, grant, out);
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
