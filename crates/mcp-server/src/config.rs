//! Where the server finds the input method's state, and what it may do with it.
//!
//! Everything here is settled when the agent registers the server: the command line is written into the agent's configuration once, so a model talking to the server can neither widen what it may do nor point it at another directory.

use msime_client_core::edition::Edition;
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The largest runtime-options document read. A macOS document carries the preferences, and with them a custom screen-keyboard photo of up to 1 MiB of base64.
const OPTIONS_READ_LIMIT: u64 = 2 << 20;

/// 帮助文本。`{program}` 换成用户实际敲的命令名，见 [`usage`]。终端里常见 80 到 100 列，每行控制在 80 列以内，免得被终端从单词中间折断。
const USAGE: &str = "usage: {program} expand <keys> [--scheme <scheme>] [--limit <n>] [--json]
       {program} config [--json]
       {program} config set <key>=<value>...
       {program} [flags] tools | call <tool> [<json>|-|@file]
       {program} [flags] prompts | prompt <name> [<json>|@file]
       {program} [flags]

Test 水杉输入法 (MSIME) by hand, or let an AI assistant manage it.

Testing by hand:
  expand <keys>      Show the candidates typing <keys> offers, one per line:
                     rank, text, code, origin and weight. Typed in your
                     current scheme unless --scheme names quanpin, shuangpin
                     or wubi; --limit takes 1 to 50 (20 by default); --json
                     prints the raw result.
  config             Show your current preferences, one key = value per line.
  config set <key>=<value>...
                     Change preferences, such as scheme=shuangpin or
                     candidate_page_size=9, and show the result. The input
                     method picks the change up within a few seconds.

  Every run reads the preferences anew, so there is nothing to reload.
  expand leaves out quick phrases, cloud and AI candidates and the context
  of earlier words. expand implies --allow-dictionary-read and config set
  implies --allow-write.

For an AI assistant:
  (no command)       Serve the Model Context Protocol over stdio.
  tools              List the tools, with their argument schemas, as JSON.
  call <tool> [<json>|-|@file]
                     Run one tool and print its result as JSON. The
                     arguments are a JSON object (default {}), read from
                     stdin for -, or from a UTF-8 file for @file, which
                     works in every shell. Tool names may use - for _. A
                     refused call prints the reason to stderr and exits 1.
  prompts            List the guided tasks (prompts), as JSON.
  prompt <name> [<json>|@file]
                     Print a guided task's instructions, such as diagnose or
                     make_skin.

  The server and the commands offer the same tools and prompts under the
  same flags.

Flags:
  --options <path>   The runtime-options document the input method hosts
                     read. Defaults to MSIME_CLIENT_HOST_OPTIONS, then
                     MSIME_IBUS_OPTIONS, then the platform's usual location.
  --state-dir <path> The directory holding preferences.json,
                     typing-statistics.json and the skins folder. Defaults
                     to MSIME_CLIENT_STATE_DIR, then the document's
                     preferences_directory.
  --allow-write      Offer the tools that change quick phrases and
                     preferences and install candidate-window skins.
                     Without it the server is read-only.
  --allow-dictionary-read
                     Offer the tools that read the user's own dictionary
                     words and look up the candidates a code offers. With
                     --allow-write as well, also the tools that add,
                     reweight, remove and import words.
  --help, --version";

/// 帮助文本，用户敲的是什么命令名就写什么。
pub fn usage() -> String {
    USAGE.replace("{program}", program())
}

/// 用户敲的命令名：经 Homebrew 或手动链接成 `msime` 时是 `msime`，否则是 `msime-mcp`。帮助和报错都用它，照着抄就能运行。
pub fn program() -> &'static str {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        std::env::args_os()
            .next()
            .as_deref()
            .map(Path::new)
            .and_then(Path::file_stem)
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("msime-mcp")
            .to_owned()
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve(Config),
    /// List the tools the flags offer.
    Tools(Config),
    /// Run one tool from a shell.
    Call {
        config: Config,
        tool: String,
        arguments: Arguments,
    },
    /// List the prompts the flags offer.
    Prompts(Config),
    /// Print one prompt.
    Prompt {
        config: Config,
        name: String,
        arguments: Arguments,
    },
    /// `expand`：查一串按键在当前方案（或 `--scheme` 指定的方案）下给出的候选。
    Expand {
        config: Config,
        code: String,
        scheme: Option<String>,
        limit: Option<u64>,
        json: bool,
    },
    /// `config`：打印当前偏好。
    ShowConfig {
        config: Config,
        json: bool,
    },
    /// `config set <key>=<value>...`：按当前 revision 修改偏好。值能按 JSON 解析就按 JSON（数字、布尔、null），否则当字符串。
    SetConfig {
        config: Config,
        changes: Vec<(String, Value)>,
        json: bool,
    },
    Help,
    Version,
}

/// Where `call` finds the tool's arguments.
#[derive(Debug, PartialEq, Eq)]
pub enum Arguments {
    Inline(String),
    /// `-`: a document too long or too awkward to quote on a command line, such as a skin with its images.
    Stdin,
    /// `@path`: a UTF-8 file holding the document, for a shell that cannot pass JSON intact, such as Windows PowerShell, which strips its quotes on the way to a program and pipes text in the console code page.
    File(PathBuf),
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
    // 只有 expand 和 config 认的开关；别的命令带上它们时拒绝，免得被悄悄忽略。
    let mut scheme = None;
    let mut limit = None;
    let mut json = false;
    let mut positional = Vec::with_capacity(4);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--version" | "-V") => return Ok(Command::Version),
            Some("--allow-write") => allow_write = true,
            Some("--allow-dictionary-read") => allow_dictionary_read = true,
            Some("--json") => json = true,
            Some("--scheme") => {
                scheme = Some(
                    args.next()
                        .and_then(|value| value.into_string().ok())
                        .ok_or("--scheme needs quanpin, shuangpin or wubi")?,
                );
            }
            Some("--limit") => {
                limit = Some(
                    args.next()
                        .and_then(|value| value.to_str()?.parse::<u64>().ok())
                        .ok_or("--limit needs a number")?,
                );
            }
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
            Some(word) if !word.starts_with('-') || word == "-" => positional.push(word.to_owned()),
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
    let mut config = Config {
        options,
        state_dir,
        allow_write,
        allow_dictionary_read,
    };
    match positional.first().map(String::as_str) {
        Some("expand") => {
            let [_, code] = <[String; 2]>::try_from(positional)
                .map_err(|_| "expand needs exactly one string of keys")?;
            // 在终端里亲手敲 expand 就是要看词库候选，不必再另加开关。
            config.allow_dictionary_read = true;
            return Ok(Command::Expand {
                config,
                code,
                scheme,
                limit,
                json,
            });
        }
        Some("config") if scheme.is_none() && limit.is_none() => {
            return match positional.get(1).map(String::as_str) {
                None => Ok(Command::ShowConfig { config, json }),
                Some("set") if positional.len() > 2 => {
                    let changes = positional[2..]
                        .iter()
                        .map(|pair| setting(pair))
                        .collect::<Result<_, _>>()?;
                    config.allow_write = true;
                    Ok(Command::SetConfig {
                        config,
                        changes,
                        json,
                    })
                }
                Some("set") => Err("config set needs at least one <key>=<value>".into()),
                Some(word) => Err(format!("unknown config command {word}")),
            };
        }
        _ if scheme.is_some() || limit.is_some() => {
            return Err("--scheme and --limit only go with expand".into())
        }
        _ if json => return Err("--json only goes with expand and config".into()),
        _ => {}
    }
    let mut positional = positional.into_iter();
    match (
        positional.next().as_deref(),
        positional.next(),
        positional.next(),
        positional.next(),
    ) {
        (None, ..) => Ok(Command::Serve(config)),
        (Some("tools"), None, ..) => Ok(Command::Tools(config)),
        (Some("call"), Some(tool), arguments, None) => Ok(Command::Call {
            config,
            // Tool names are snake_case; a shell user reaching for kebab-case means the same tool.
            tool: tool.replace('-', "_"),
            arguments: match arguments {
                None => Arguments::Inline("{}".into()),
                Some(text) if text == "-" => Arguments::Stdin,
                Some(text) if text.starts_with('@') => Arguments::File(PathBuf::from(&text[1..])),
                Some(text) => Arguments::Inline(text),
            },
        }),
        (Some("call"), None, ..) => Err("call needs a tool name".into()),
        (Some("prompts"), None, ..) => Ok(Command::Prompts(config)),
        (Some("prompt"), Some(name), arguments, None) => Ok(Command::Prompt {
            config,
            name: name.replace('-', "_"),
            arguments: match arguments {
                None => Arguments::Inline("{}".into()),
                Some(text) if text == "-" => Arguments::Stdin,
                Some(text) if text.starts_with('@') => Arguments::File(PathBuf::from(&text[1..])),
                Some(text) => Arguments::Inline(text),
            },
        }),
        (Some("prompt"), None, ..) => Err("prompt needs a prompt name".into()),
        (Some(word), ..) => Err(format!("unknown command {word}")),
    }
}

/// `config set` 的一项 `<key>=<value>`。`scheme=wubi` 这样的值不是 JSON，按字符串传；`9`、`true`、`null` 按 JSON 传，与 update_preferences 的参数类型对上。
fn setting(pair: &str) -> Result<(String, Value), String> {
    let (key, value) = pair
        .split_once('=')
        .filter(|(key, _)| !key.is_empty())
        .ok_or_else(|| format!("{pair} is not <key>=<value>"))?;
    let value = serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_owned()));
    // 和 call 的工具名一样，接受 kebab-case。
    Ok((key.replace('-', "_"), value))
}

/// Where the desktop app keeps the document when nothing says otherwise.
fn default_options_path(env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |value: OsString| Some(PathBuf::from(value)).filter(|path| path.is_absolute());
    if cfg!(target_os = "macos") {
        // 设置应用的 `native_locator_root`：目录名是 msime-mcp 所在安装包所属版本的设置应用 identifier（full 是 app.msime.macos）。安装包的版本声明坏了时没有默认位置，而不是退回 full 的。
        let identity = msime_client_core::edition::Edition::of_macos_bundle()
            .ok()?
            .macos()?;
        return env("HOME").and_then(absolute).map(|home| {
            home.join("Library/Application Support")
                .join(&identity.settings_bundle_id)
                .join("runtime-options.json")
        });
    }
    if cfg!(target_os = "linux") {
        // The fixed locator every Linux frontend reads (`user_runtime_options` in the desktop app). A relative XDG value is ignored, as the specification requires.
        // 目录名随本进程所在安装包的版本（前缀 bin 目录里的 edition.json，full 是 msime-client）；声明坏了时不猜成 full，免得读写 full 的状态。
        let directory = &msime_client_core::edition::Edition::linux_package_identity()
            .ok()?
            .client_directory;
        return env("XDG_CONFIG_HOME")
            .and_then(absolute)
            .or_else(|| {
                env("HOME")
                    .and_then(absolute)
                    .map(|home| home.join(".config"))
            })
            .map(|config| config.join(directory).join("runtime-options.json"));
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

    /// The runtime-options document with its `preferences` replaced by the live preferences.json, which the hosts poll; the document's own copy is only what the settings page wrote when it prepared the host, and on macOS and Windows nothing refreshes it afterwards. A store that was never written leaves the document's copy alone.
    pub fn read_host_options(&self) -> Result<Value, String> {
        let mut document = self.read_options()?;
        let snapshot =
            msime_client_core::preferences::PreferencesStore::new(self.state_dir(&document)?)
                .load()
                .map_err(|error| error.to_string())?;
        if snapshot.revision > 0 {
            document["preferences"] =
                serde_json::to_value(&snapshot.preferences).map_err(|error| error.to_string())?;
        }
        Ok(document)
    }

    /// 运行时选项记录的版本，没有 `edition` 键就是 full。记录了不认识的版本时拒绝，而不是按 full 去改另一个版本的偏好。
    pub fn edition(&self, document: &Value) -> Result<&'static Edition, String> {
        Edition::of_host_options(document)
            .ok_or_else(|| "the runtime options name an unknown edition of the input method".into())
    }

    /// 向助手报告的服务器名，与设置页登记条目用的键相同（`Edition::mcp_server_name`）：full 是 `msime`。运行时选项还读不了时按 full 报告，各个工具被调用时会再读一次并报告问题。
    pub fn server_name(&self) -> String {
        self.read_options()
            .ok()
            .and_then(|document| Edition::of_host_options(&document))
            .unwrap_or_else(Edition::full)
            .mcp_server_name()
    }

    /// The directory holding preferences.json, typing-statistics.json and the skins folder.
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
    fn the_help_fits_an_80_column_terminal_under_either_name() {
        for name in ["msime", "msime-mcp"] {
            let text = USAGE.replace("{program}", name);
            for line in text.lines() {
                assert!(line.chars().count() <= 80, "{line}");
            }
        }
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
    fn a_tool_runs_from_the_command_line_under_the_same_flags() {
        let parsed = |list: &[&str]| parse(args(list), |_: &str| None);
        let Command::Tools(config) = parsed(&["tools", "--options", "/a.json"]).unwrap() else {
            panic!("expected tools");
        };
        assert!(!config.allow_write);
        assert_eq!(
            parsed(&[
                "--options",
                "/a.json",
                "--allow-write",
                "call",
                "edit-quick-phrases",
                "-"
            ])
            .unwrap(),
            Command::Call {
                config: Config {
                    options: PathBuf::from("/a.json"),
                    state_dir: None,
                    allow_write: true,
                    allow_dictionary_read: false,
                },
                tool: "edit_quick_phrases".into(),
                arguments: Arguments::Stdin,
            }
        );
        let Command::Call { arguments, .. } =
            parsed(&["--options", "/a.json", "call", "get_preferences"]).unwrap()
        else {
            panic!("expected call");
        };
        assert_eq!(arguments, Arguments::Inline("{}".into()));
        assert!(parsed(&["--options", "/a.json", "call"]).is_err());
        assert!(parsed(&["--options", "/a.json", "tools", "extra"]).is_err());
        assert!(parsed(&["--options", "/a.json", "call", "a", "{}", "extra"]).is_err());
        assert!(parsed(&["--options", "/a.json", "serve"]).is_err());
        assert!(matches!(
            parsed(&["--options", "/a.json", "prompts"]).unwrap(),
            Command::Prompts(_)
        ));
        let Command::Prompt {
            name, arguments, ..
        } = parsed(&[
            "--options",
            "/a.json",
            "prompt",
            "make-skin",
            r#"{"style":"夜色"}"#,
        ])
        .unwrap()
        else {
            panic!("expected prompt");
        };
        assert_eq!(name, "make_skin");
        assert_eq!(arguments, Arguments::Inline(r#"{"style":"夜色"}"#.into()));
        assert!(parsed(&["--options", "/a.json", "prompt"]).is_err());
        let Command::Call { arguments, .. } = parsed(&[
            "--options",
            "/a.json",
            "call",
            "get_preferences",
            "@C:\\args.json",
        ])
        .unwrap() else {
            panic!("expected call");
        };
        assert_eq!(arguments, Arguments::File(PathBuf::from("C:\\args.json")));
    }

    #[test]
    fn expand_and_config_are_shortcuts_with_their_own_switches() {
        let parsed = |list: &[&str]| parse(args(list), |_: &str| None);
        let Command::Expand {
            config,
            code,
            scheme,
            limit,
            json,
        } = parsed(&[
            "--options",
            "/a.json",
            "expand",
            "ni'hao",
            "--scheme",
            "wubi",
            "--limit",
            "5",
        ])
        .unwrap()
        else {
            panic!("expected expand");
        };
        assert!(config.allow_dictionary_read && !config.allow_write);
        assert_eq!(code, "ni'hao");
        assert_eq!(scheme.as_deref(), Some("wubi"));
        assert_eq!(limit, Some(5));
        assert!(!json);
        assert!(parsed(&["--options", "/a.json", "expand"]).is_err());
        assert!(parsed(&["--options", "/a.json", "expand", "a", "b"]).is_err());
        assert!(parsed(&["--options", "/a.json", "expand", "a", "--limit", "x"]).is_err());

        let Command::ShowConfig { config, json } =
            parsed(&["--options", "/a.json", "config", "--json"]).unwrap()
        else {
            panic!("expected config");
        };
        assert!(json && !config.allow_write && !config.allow_dictionary_read);

        let Command::SetConfig {
            config, changes, ..
        } = parsed(&[
            "--options",
            "/a.json",
            "config",
            "set",
            "scheme=shuangpin",
            "candidate-page-size=9",
            "fuzzy_pinyin=true",
            "candidate_corner_radius=null",
        ])
        .unwrap()
        else {
            panic!("expected config set");
        };
        assert!(config.allow_write && !config.allow_dictionary_read);
        assert_eq!(
            changes,
            [
                ("scheme".to_owned(), json!("shuangpin")),
                ("candidate_page_size".to_owned(), json!(9)),
                ("fuzzy_pinyin".to_owned(), json!(true)),
                ("candidate_corner_radius".to_owned(), Value::Null),
            ]
        );
        assert!(parsed(&["--options", "/a.json", "config", "set"]).is_err());
        assert!(parsed(&["--options", "/a.json", "config", "set", "scheme"]).is_err());
        assert!(parsed(&["--options", "/a.json", "config", "set", "=wubi"]).is_err());
        assert!(parsed(&["--options", "/a.json", "config", "get"]).is_err());
        // 只属于 expand 和 config 的开关不能悄悄跟着别的命令。
        assert!(parsed(&["--options", "/a.json", "config", "--scheme", "wubi"]).is_err());
        assert!(parsed(&["--options", "/a.json", "tools", "--json"]).is_err());
        assert!(parsed(&["--options", "/a.json", "--limit", "3"]).is_err());
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn the_platform_default_follows_the_desktop_app() {
        let env = |name: &str| (name == "HOME").then(|| "/home/someone".into());
        let expected = if cfg!(target_os = "macos") {
            "/home/someone/Library/Application Support/app.msime.macos/runtime-options.json"
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

    #[test]
    fn the_live_preferences_replace_the_document_copy_once_written() {
        use msime_client_core::preferences::{InputScheme, Preferences, PreferencesStore};
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        std::fs::write(
            &options,
            json!({
                "preferences": { "scheme": "wubi" },
                "preferences_directory": directory.path().to_str().unwrap(),
            })
            .to_string(),
        )
        .unwrap();
        let config = Config {
            options,
            state_dir: None,
            allow_write: false,
            allow_dictionary_read: true,
        };
        assert_eq!(
            config.read_host_options().unwrap()["preferences"]["scheme"],
            "wubi"
        );
        let preferences = Preferences {
            scheme: InputScheme::Quanpin,
            ..Preferences::default()
        };
        PreferencesStore::new(directory.path())
            .save(0, preferences)
            .unwrap();
        assert_eq!(
            config.read_host_options().unwrap()["preferences"]["scheme"],
            "quanpin"
        );
    }

    #[test]
    fn the_edition_and_server_name_come_from_the_runtime_options() {
        let directory = tempfile::tempdir().unwrap();
        let options = directory.path().join("runtime-options.json");
        let config = Config {
            options: options.clone(),
            state_dir: None,
            allow_write: false,
            allow_dictionary_read: false,
        };
        // 还没有运行时选项：按 full 报告服务器名。
        assert_eq!(config.server_name(), "msime");

        std::fs::write(&options, br#"{"api_version":1}"#).unwrap();
        assert!(config
            .edition(&config.read_options().unwrap())
            .unwrap()
            .is_full());
        assert_eq!(config.server_name(), "msime");

        std::fs::write(&options, br#"{"api_version":1,"edition":"wubi"}"#).unwrap();
        assert_eq!(
            config.edition(&config.read_options().unwrap()).unwrap().id,
            "wubi"
        );
        assert_eq!(config.server_name(), "msime-wubi");

        std::fs::write(&options, br#"{"api_version":1,"edition":"klingon"}"#).unwrap();
        assert!(config.edition(&config.read_options().unwrap()).is_err());
    }
}
