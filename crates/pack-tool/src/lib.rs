//! `msime-pack`: checks extension packs by exactly the rules the input method imports them by.
//!
//! ```text
//! msime-pack validate <pack folder or .zip>...
//! ```
//!
//! Every rule lives in `msime_client_core::plugins`: `validate` stages each pack the way `import` does and runs the same checks, so a pack this tool accepts is one the 插件 page installs, and the other way round. Nothing is installed and no plugins directory is touched. One line per pack, in the order given: `ok <id> <kind> <version>`, or `error <path>: <reason>` where the reason starts with the same failure code the settings page decodes (`plugin_invalid`, `plugin_archive`, ...) followed by the broken rule in Chinese. The exit status is 0 when every pack is valid, 1 when any is not, and 2 for a usage error.

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use msime_client_core::plugins::{self, PluginError};

/// Every pack given is valid.
pub const EXIT_OK: i32 = 0;
/// At least one pack is not.
pub const EXIT_INVALID: i32 = 1;
/// The command line was not understood.
pub const EXIT_USAGE: i32 = 2;

const USAGE: &str = "usage: msime-pack validate <pack folder or .zip>...";

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

/// The failure code and, for the codes that carry no detail, what it means for an author.
fn reason(error: &PluginError) -> String {
    match error {
        PluginError::UnsupportedSource => {
            "plugin_unsupported_source: 只接受插件文件夹或 .zip 文件".to_owned()
        }
        PluginError::Reserved => "plugin_reserved: 这个 id 属于内置插件，不能导入".to_owned(),
        other => other.to_string(),
    }
}
