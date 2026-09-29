//! Where the server finds the input method's state, and what it may do with it.
//!
//! Everything here is settled when the agent registers the server: the command line is written into the agent's configuration once, so a model talking to the server can neither widen what it may do nor point it at another directory.

use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The largest runtime-options document read. A macOS document carries the preferences, and with them a custom screen-keyboard photo of up to 1 MiB of base64.
const OPTIONS_READ_LIMIT: u64 = 2 << 20;

pub const USAGE: &str = "usage: msime-mcp [--options <runtime-options.json>] [--state-dir <directory>] [--allow-write] [--allow-dictionary-read]

Serves the Model Context Protocol over stdio for 水杉输入法 (MSIME).

  --options <path>     The runtime-options document the input method hosts read. Defaults to MSIME_CLIENT_HOST_OPTIONS, then MSIME_IBUS_OPTIONS, then the platform's usual location.
  --state-dir <path>   The directory holding preferences.json and typing-statistics.json. Defaults to MSIME_CLIENT_STATE_DIR, then the document's preferences_directory.
  --allow-write        Offer the tools that change quick phrases and preferences. Without it the server is read-only.
  --allow-dictionary-read
                       Offer the tools that read the user's own dictionary words and look up the candidates a code offers. With --allow-write as well, also the tools that add, reweight, remove and import words.
  --help, --version";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve(Config),
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub options: PathBuf,
    /// The explicit state directory, from `--state-dir` or `MSIME_CLIENT_STATE_DIR`. Absent means the document's `preferences_directory`, read on every call so a moved data directory is followed.
    pub state_dir: Option<PathBuf>,
    pub allow_write: bool,
    /// The user's own words are what they type, so reading them is a separate choice from writing quick phrases and preferences.
    pub allow_dictionary_read: bool,
}

/// Parse the command line. `env` is the process environment, passed in so the lookup order can be tested.
pub fn parse(
    args: impl IntoIterator<Item = OsString>,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<Command, String> {
    let mut options = None;
    let mut state_dir = None;
    let mut allow_write = false;
    let mut allow_dictionary_read = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--version" | "-V") => return Ok(Command::Version),
            Some("--allow-write") => allow_write = true,
            Some("--allow-dictionary-read") => allow_dictionary_read = true,
            Some(flag @ ("--options" | "--state-dir")) => {
                let value = args
                    .next()
                    .map(PathBuf::from)
                    .ok_or_else(|| format!("{flag} needs a path"))?;
                if flag == "--options" {
                    options = Some(value);
                } else {
                    state_dir = Some(value);
                }
            }
            _ => return Err(format!("unknown argument {}", arg.to_string_lossy())),
        }
    }
    let options = match options {
        Some(path) => path,
        None => env("MSIME_CLIENT_HOST_OPTIONS")
            .or_else(|| env("MSIME_IBUS_OPTIONS"))
            .map(PathBuf::from)
            .or_else(|| default_options_path(&env))
            .ok_or("no runtime options found; pass --options <path>")?,
    };
    let state_dir = state_dir.or_else(|| env("MSIME_CLIENT_STATE_DIR").map(PathBuf::from));
    for path in std::iter::once(&options).chain(state_dir.iter()) {
        if !path.is_absolute() {
            return Err(format!("{} must be an absolute path", path.display()));
        }
    }
    Ok(Command::Serve(Config {
        options,
        state_dir,
        allow_write,
        allow_dictionary_read,
    }))
}

/// Where the desktop app keeps the document when nothing says otherwise.
fn default_options_path(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |value: OsString| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
    if cfg!(target_os = "macos") {
        // `native_locator_root` in the desktop app.
        return env("HOME").and_then(absolute).map(|home| {
            home.join("Library/Application Support/app.msime.client/runtime-options.json")
        });
    }
    if cfg!(target_os = "linux") {
        // The fixed locator every Linux frontend reads (`user_runtime_options` in the desktop app). A relative XDG value is ignored, as the specification requires.
        return env("XDG_CONFIG_HOME")
            .and_then(absolute)
            .or_else(|| {
                env("HOME")
                    .and_then(absolute)
                    .map(|home| home.join(".config"))
            })
            .map(|config| config.join("msime-client/runtime-options.json"));
    }
    #[cfg(windows)]
    {
        // The Server prepares the document in its state directory (`windows_server_state_directory` in the desktop app). That directory is resolved from the real environment and the registry, not from `env`.
        let _ = env;
        return msime_host_windows::server_state_directory()
            .map(|directory| directory.join("runtime-options.json"));
    }
    #[allow(unreachable_code)]
    None
}

impl Config {
    /// The runtime-options document as it is now.
    pub fn read_options(&self) -> Result<Value, String> {
        let file = std::fs::File::open(&self.options)
            .map_err(|_| "cannot open the runtime options; is the input method set up?")?;
        let bytes =
            crate::bounded::read(file, OPTIONS_READ_LIMIT).map_err(|error| match error {
                crate::bounded::ReadError::TooLarge => "the runtime options are too large",
                crate::bounded::ReadError::Io => "cannot read the runtime options",
            })?;
        let document: Value =
            serde_json::from_slice(&bytes).map_err(|_| "cannot parse the runtime options")?;
        if !document.is_object() {
            return Err("cannot parse the runtime options".into());
        }
        Ok(document)
    }

    /// The directory holding preferences.json and typing-statistics.json.
    pub fn state_dir(&self, document: &Value) -> Result<PathBuf, String> {
        state_dir(self.state_dir.as_deref(), document, &self.options)
    }
}

fn state_dir(explicit: Option<&Path>, document: &Value, options: &Path) -> Result<PathBuf, String> {
    if let Some(directory) = explicit {
        return Ok(directory.to_owned());
    }
    match document.get("preferences_directory") {
        Some(Value::String(value)) if Path::new(value).is_absolute() => {
            return Ok(PathBuf::from(value))
        }
        None | Some(Value::Null) => {}
        Some(Value::String(value)) if value.is_empty() => {}
        _ => {
            return Err(
                "the runtime options name a preferences directory that is not absolute".into(),
            )
        }
    }
    // The macOS desktop app keeps its state beside the document, and the Windows Server uses its state directory, where the document is, when it names no other.
    if cfg!(any(target_os = "macos", windows)) {
        if let Some(parent) = options.parent() {
            return Ok(parent.to_owned());
        }
    }
    Err("no preferences directory; pass --state-dir <path>".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn serve(command: Command) -> Config {
        match command {
            Command::Serve(config) => config,
            other => panic!("expected a configuration, got {other:?}"),
        }
    }

    #[test]
    fn the_command_line_wins_over_the_environment_and_writing_is_off_by_default() {
        let env = |name: &str| match name {
            "MSIME_CLIENT_HOST_OPTIONS" => Some("/env/host.json".into()),
            "MSIME_IBUS_OPTIONS" => Some("/env/ibus.json".into()),
            "MSIME_CLIENT_STATE_DIR" => Some("/env/state".into()),
            _ => None,
        };
        let config = serve(parse(args(&["--options", "/flag/options.json"]), env).unwrap());
        assert_eq!(config.options, PathBuf::from("/flag/options.json"));
        assert_eq!(config.state_dir, Some(PathBuf::from("/env/state")));
        assert!(!config.allow_write);
        assert!(!config.allow_dictionary_read);

        let config = serve(parse(args(&[]), env).unwrap());
        assert_eq!(config.options, PathBuf::from("/env/host.json"));
        let ibus_only =
            |name: &str| (name == "MSIME_IBUS_OPTIONS").then(|| "/env/ibus.json".into());
        assert_eq!(
            serve(parse(args(&[]), ibus_only).unwrap()).options,
            PathBuf::from("/env/ibus.json")
        );
    }

    #[test]
    fn bad_command_lines_are_refused() {
        let env = |_: &str| None;
        assert!(parse(args(&["--options"]), env).is_err());
        assert!(parse(args(&["--surprise"]), env).is_err());
        assert!(parse(args(&["--options", "relative.json"]), env).is_err());
        assert!(parse(
            args(&["--options", "/a.json", "--state-dir", "relative"]),
            env
        )
        .is_err());
        assert_eq!(parse(args(&["--help"]), env).unwrap(), Command::Help);
        assert_eq!(parse(args(&["--version"]), env).unwrap(), Command::Version);
    }

    #[test]
    fn writing_and_reading_the_dictionary_are_explicit_flags() {
        let config = |list: &[&str]| serve(parse(args(list), |_: &str| None).unwrap());
        let write = config(&["--options", "/a.json", "--allow-write"]);
        assert!(write.allow_write && !write.allow_dictionary_read);
        let read = config(&["--options", "/a.json", "--allow-dictionary-read"]);
        assert!(!read.allow_write && read.allow_dictionary_read);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn the_platform_default_follows_the_desktop_app() {
        let env = |name: &str| (name == "HOME").then(|| "/home/someone".into());
        let expected = if cfg!(target_os = "macos") {
            "/home/someone/Library/Application Support/app.msime.client/runtime-options.json"
        } else {
            "/home/someone/.config/msime-client/runtime-options.json"
        };
        assert_eq!(
            serve(parse(args(&[]), env).unwrap()).options,
            PathBuf::from(expected)
        );
    }

    #[test]
    fn the_state_directory_comes_from_the_flag_then_the_document() {
        let options = Path::new("/state/runtime-options.json");
        let document = json!({ "preferences_directory": "/prefs" });
        assert_eq!(
            state_dir(Some(Path::new("/flag")), &document, options).unwrap(),
            PathBuf::from("/flag")
        );
        assert_eq!(
            state_dir(None, &document, options).unwrap(),
            PathBuf::from("/prefs")
        );
        assert!(state_dir(
            None,
            &json!({ "preferences_directory": "relative" }),
            options
        )
        .is_err());
        let fallback = state_dir(None, &json!({}), options);
        if cfg!(target_os = "macos") {
            assert_eq!(fallback.unwrap(), PathBuf::from("/state"));
        } else {
            assert!(fallback.is_err());
        }
    }
}
