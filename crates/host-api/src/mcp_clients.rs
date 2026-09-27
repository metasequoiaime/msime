//! Registering `msime-mcp` with the AI assistants that read a local MCP configuration file.
//!
//! Both settings hosts use this: the shared settings page in the desktop shell, and on Windows the WinUI settings window through `msime_client_mcp_status` and `msime_client_mcp_install`. The page shows the server entry so it can be copied into any assistant, and for Claude Desktop and Cursor writes it into their configuration file. Only `mcpServers.msime` is touched: every other key the user has is kept, a file that is not a JSON object is refused rather than replaced, and the write is atomic so a crash leaves the old file or the new one, never half of either.
//!
//! The entry is read-only: it names the runtime options and no flags. Letting an assistant change quick phrases, preferences or words (`--allow-write`), or read the user's words (`--allow-dictionary-read`), is a decision the user makes by adding the flag to the args themselves.

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The key the entry is stored under in `mcpServers`.
pub const SERVER_NAME: &str = "msime";
/// A configuration file larger than this is not one an assistant wrote; refuse it rather than read it whole.
const CONFIG_READ_LIMIT: u64 = 4 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpClient {
    ClaudeDesktop,
    Cursor,
}

#[derive(Debug, Serialize)]
pub struct McpClientStatus {
    pub id: McpClient,
    /// Where the configuration file is, for the page to show.
    pub path: String,
    /// The file already holds exactly this entry.
    pub configured: bool,
}

#[derive(Debug, Serialize)]
pub struct McpServerStatus {
    /// The absolute path of `msime-mcp` beside this executable.
    pub command: String,
    /// Whether that file exists; a development build may not have built it.
    pub installed: bool,
    /// The runtime-options document the entry points the server at.
    pub options: Option<String>,
    /// `{"mcpServers": {"msime": ...}}`, ready to paste into any assistant's configuration.
    pub config: Option<String>,
    pub clients: Vec<McpClientStatus>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallOutcome {
    /// The file had no `msime` entry, or had no file at all.
    Added,
    /// An `msime` entry that differed was replaced, as the caller allowed.
    Replaced,
    /// The file already held this entry; nothing was written.
    Unchanged,
}

/// `msime-mcp` as it is packaged: beside the settings executable in `Contents/MacOS` on macOS, in the same `bin` directory on Linux, and in the `server` directory on Windows.
pub fn server_command(executable: &Path) -> Option<PathBuf> {
    executable
        .parent()
        .map(|directory| directory.join(format!("msime-mcp{}", std::env::consts::EXE_SUFFIX)))
}

/// The entry an assistant runs: the server and the runtime options, and no flags.
pub fn server_entry(command: &Path, options: &Path) -> Value {
    json!({
        "command": command.to_string_lossy(),
        "args": ["--options", options.to_string_lossy()],
    })
}

/// The entry wrapped the way both assistants, and most others, expect it.
pub fn config_snippet(entry: &Value) -> String {
    let document = json!({ "mcpServers": { SERVER_NAME: entry } });
    serde_json::to_string_pretty(&document).unwrap_or_default()
}

/// The assistants offered on this platform and their configuration files, found from the home directory (`HOME` on macOS and Linux, `USERPROFILE` on Windows) and, for Claude Desktop on Windows, `APPDATA`.
///
/// Claude Desktop: `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS and `%APPDATA%\Claude\claude_desktop_config.json` on Windows, as the Model Context Protocol's "Connect to local MCP servers" guide (modelcontextprotocol.io/quickstart/user) gives them. Claude Desktop has no Linux release, so it is not offered there.
///
/// Cursor: the global `~/.cursor/mcp.json`, `%USERPROFILE%\.cursor\mcp.json` on Windows, per Cursor's Model Context Protocol documentation (docs.cursor.com/context/model-context-protocol).
pub fn client_paths(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> Vec<(McpClient, PathBuf)> {
    let absolute = |name: &str| {
        env(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let mut clients = Vec::new();
    let home = if cfg!(windows) {
        absolute("USERPROFILE")
    } else {
        absolute("HOME")
    };
    if cfg!(target_os = "macos") {
        if let Some(home) = &home {
            clients.push((
                McpClient::ClaudeDesktop,
                home.join("Library/Application Support/Claude/claude_desktop_config.json"),
            ));
        }
    } else if cfg!(windows) {
        if let Some(roaming) = absolute("APPDATA") {
            clients.push((
                McpClient::ClaudeDesktop,
                roaming.join("Claude").join("claude_desktop_config.json"),
            ));
        }
    }
    if let Some(home) = home {
        clients.push((McpClient::Cursor, home.join(".cursor").join("mcp.json")));
    }
    clients
}

/// `msime-mcp` beside `executable`, the entry pointing it at `options`, and whether each assistant offered here already has it. Without `options` the input method is not set up yet, so there is no entry to show or compare.
pub fn status(
    executable: &Path,
    options: Option<&Path>,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<McpServerStatus, &'static str> {
    let command = server_command(executable).ok_or("storage")?;
    let entry = options.map(|options| server_entry(&command, options));
    let clients = client_paths(env)
        .into_iter()
        .map(|(id, path)| McpClientStatus {
            id,
            configured: entry
                .as_ref()
                .is_some_and(|entry| is_configured(&path, entry)),
            path: path.to_string_lossy().into_owned(),
        })
        .collect();
    Ok(McpServerStatus {
        installed: command.is_file(),
        command: command.to_string_lossy().into_owned(),
        options: options.map(|path| path.to_string_lossy().into_owned()),
        config: entry.as_ref().map(config_snippet),
        clients,
    })
}

/// Write the entry for `msime-mcp` beside `executable` into `client`'s configuration file; see `install` for what is kept and when a different entry is replaced.
pub fn install_client(
    executable: &Path,
    options: Option<&Path>,
    client: McpClient,
    replace: bool,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<InstallOutcome, &'static str> {
    let options = options.ok_or("mcp_options_missing")?;
    let command = server_command(executable)
        .filter(|command| command.is_file())
        .ok_or("mcp_server_missing")?;
    let (_, path) = client_paths(env)
        .into_iter()
        .find(|(id, _)| *id == client)
        .ok_or("mcp_client_missing")?;
    install(&path, &server_entry(&command, options), replace)
}

/// The configuration as it is, or an empty object when there is no file yet.
fn read_config(path: &Path) -> Result<Map<String, Value>, &'static str> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(_) => return Err("storage"),
    };
    let bytes = crate::bounded_file::read(file, CONFIG_READ_LIMIT).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            "mcp_config_invalid"
        } else {
            "storage"
        }
    })?;
    // An empty file is what some editors leave behind; it holds nothing to keep.
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(Map::new());
    }
    match serde_json::from_slice(&bytes) {
        Ok(Value::Object(document)) => Ok(document),
        _ => Err("mcp_config_invalid"),
    }
}

/// Whether the file at `path` already holds exactly `entry`. A file that cannot be read reads as not configured.
pub fn is_configured(path: &Path, entry: &Value) -> bool {
    read_config(path).is_ok_and(|document| {
        document
            .get("mcpServers")
            .and_then(|servers| servers.get(SERVER_NAME))
            == Some(entry)
    })
}

/// Put `entry` under `mcpServers.msime` in the file at `path`, keeping everything else.
///
/// The directory must already exist: it is created by the assistant itself, so a missing one means the assistant is not installed, and creating it would leave a directory for an application the user does not have. A different `msime` entry is replaced only when `replace` is set; otherwise the call fails with `mcp_entry_exists` so the page can ask first. A symbolic link, such as a configuration kept in a dotfiles repository, is written through rather than replaced.
pub fn install(path: &Path, entry: &Value, replace: bool) -> Result<InstallOutcome, &'static str> {
    let target = match std::fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_owned(),
        Err(_) => return Err("storage"),
    };
    let directory = target.parent().ok_or("storage")?;
    if !directory.is_dir() {
        return Err("mcp_client_missing");
    }
    let mut document = read_config(&target)?;
    let servers = document
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("mcp_config_invalid")?;
    let outcome = match servers.get(SERVER_NAME) {
        None => InstallOutcome::Added,
        Some(existing) if existing == entry => return Ok(InstallOutcome::Unchanged),
        Some(_) if replace => InstallOutcome::Replaced,
        Some(_) => return Err("mcp_entry_exists"),
    };
    servers.insert(SERVER_NAME.to_owned(), entry.clone());
    let mut text = serde_json::to_string_pretty(&Value::Object(document)).map_err(|_| "storage")?;
    text.push('\n');
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| "storage")?;
    file.write_all(text.as_bytes()).map_err(|_| "storage")?;
    // The temporary file is private to the user; keep the permissions the file had instead.
    if let Ok(metadata) = std::fs::metadata(&target) {
        file.as_file()
            .set_permissions(metadata.permissions())
            .map_err(|_| "storage")?;
    }
    file.as_file().sync_all().map_err(|_| "storage")?;
    file.persist(&target).map_err(|_| "storage")?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry() -> Value {
        server_entry(
            Path::new("/opt/msime/msime-mcp"),
            Path::new("/state/runtime-options.json"),
        )
    }

    #[test]
    fn the_entry_names_the_options_and_no_flags() {
        assert_eq!(
            entry(),
            json!({ "command": "/opt/msime/msime-mcp", "args": ["--options", "/state/runtime-options.json"] })
        );
        let snippet: Value = serde_json::from_str(&config_snippet(&entry())).unwrap();
        assert_eq!(snippet, json!({ "mcpServers": { "msime": entry() } }));
        assert_eq!(
            server_command(Path::new("/opt/msime/msime-desktop")).unwrap(),
            PathBuf::from(format!(
                "/opt/msime/msime-mcp{}",
                std::env::consts::EXE_SUFFIX
            ))
        );
    }

    #[test]
    fn a_new_file_gets_only_the_entry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        assert_eq!(install(&path, &entry(), false), Ok(InstallOutcome::Added));
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written, json!({ "mcpServers": { "msime": entry() } }));
        assert!(is_configured(&path, &entry()));
        assert_eq!(
            install(&path, &entry(), false),
            Ok(InstallOutcome::Unchanged)
        );
    }

    #[test]
    fn everything_else_in_the_file_is_kept() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("claude_desktop_config.json");
        let before = json!({
            "globalShortcut": "Ctrl+Space",
            "mcpServers": { "other": { "command": "/usr/bin/other", "args": [] } },
        });
        std::fs::write(&path, serde_json::to_vec(&before).unwrap()).unwrap();
        assert_eq!(install(&path, &entry(), false), Ok(InstallOutcome::Added));
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            written,
            json!({
                "globalShortcut": "Ctrl+Space",
                "mcpServers": {
                    "other": { "command": "/usr/bin/other", "args": [] },
                    "msime": entry(),
                },
            })
        );
    }

    #[test]
    fn a_different_entry_is_replaced_only_when_allowed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        let before = json!({ "mcpServers": { "msime": { "command": "/old/msime-mcp", "args": ["--allow-write"] } } });
        std::fs::write(&path, serde_json::to_vec(&before).unwrap()).unwrap();
        assert_eq!(install(&path, &entry(), false), Err("mcp_entry_exists"));
        let unchanged: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(unchanged, before);
        assert!(!is_configured(&path, &entry()));
        assert_eq!(install(&path, &entry(), true), Ok(InstallOutcome::Replaced));
        assert!(is_configured(&path, &entry()));
    }

    #[test]
    fn a_file_that_is_not_a_json_object_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        for text in ["{ \"mcpServers\": ", "[]", "{\"mcpServers\": []}"] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(install(&path, &entry(), true), Err("mcp_config_invalid"));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
        std::fs::write(&path, "\n").unwrap();
        assert_eq!(install(&path, &entry(), false), Ok(InstallOutcome::Added));
    }

    #[test]
    fn a_missing_assistant_directory_is_not_created() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".cursor").join("mcp.json");
        assert_eq!(install(&path, &entry(), false), Err("mcp_client_missing"));
        assert!(!directory.path().join(".cursor").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_file_is_written_through_and_keeps_its_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let real = directory.path().join("dotfiles-mcp.json");
        std::fs::write(&real, "{}").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o644)).unwrap();
        let link = directory.path().join("mcp.json");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(install(&link, &entry(), false), Ok(InstallOutcome::Added));
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(is_configured(&real, &entry()));
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn each_platform_offers_the_assistants_it_has() {
        let env = |name: &str| match name {
            "HOME" => Some("/home/someone".into()),
            "USERPROFILE" => Some("C:\\Users\\someone".into()),
            "APPDATA" => Some("C:\\Users\\someone\\AppData\\Roaming".into()),
            _ => None,
        };
        let clients: Vec<McpClient> = client_paths(env).into_iter().map(|(id, _)| id).collect();
        if cfg!(any(target_os = "macos", windows)) {
            assert_eq!(clients, [McpClient::ClaudeDesktop, McpClient::Cursor]);
        } else {
            assert_eq!(clients, [McpClient::Cursor]);
        }
        if cfg!(target_os = "macos") {
            assert_eq!(
                client_paths(env)[0].1,
                PathBuf::from(
                    "/home/someone/Library/Application Support/Claude/claude_desktop_config.json"
                )
            );
        }
        if cfg!(target_os = "linux") {
            assert_eq!(
                client_paths(env)[0].1,
                PathBuf::from("/home/someone/.cursor/mcp.json")
            );
        }
        // A relative home is not a home.
        assert!(client_paths(|name: &str| (name == "HOME").then(|| "relative".into())).is_empty());
    }
}
