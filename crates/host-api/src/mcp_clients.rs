//! Registering `msime-mcp` with the AI assistants that read a local MCP configuration file.
//!
//! Both settings hosts use this: the shared settings page in the desktop shell, and on Windows the WinUI settings window through `msime_client_mcp_status` and `msime_client_mcp_install`. The page shows the server entry so it can be copied into any assistant, and for Claude Desktop and Cursor writes it into their configuration file. Only `mcpServers.msime` is touched: every other key the user has is kept, a file that is not a JSON object is refused rather than replaced, and the write is atomic so a crash leaves the old file or the new one, never half of either.
//!
//! 条目默认只读：只带运行时选项。允许助手修改快捷短语、设置和词、制作候选窗口皮肤（`--allow-write`），或读取用户词库（`--allow-dictionary-read`），由用户在设置页里打开对应开关后写进 `args`；已写入的条目带了哪些开关，状态里会如实报告，好让设置页显示出来。`msime-mcp` 自己不带这些参数时仍然只读。

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(unix)]
static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The key the entry is stored under in `mcpServers`.
///
/// 这是 full 版本的键；其他版本用 `Edition::mcp_server_name`，由 [`server_name`] 按运行时选项里记录的版本选出。
pub const SERVER_NAME: &str = "msime";
/// A configuration file larger than this is not one an assistant wrote; refuse it rather than read it whole.
const CONFIG_READ_LIMIT: u64 = 4 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpClient {
    ClaudeDesktop,
    Cursor,
}

/// 放宽助手权限的 `msime-mcp` 参数。序列化成参数原文，设置页和条目的 `args` 用同一套字符串。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, Serialize)]
pub enum McpFlag {
    /// 修改快捷短语、设置和词，制作候选窗口皮肤。
    #[serde(rename = "--allow-write")]
    AllowWrite,
    /// 读取用户词库、查看编码的候选。
    #[serde(rename = "--allow-dictionary-read")]
    AllowDictionaryRead,
}

impl McpFlag {
    /// 写进 `args` 的固定顺序。
    pub const ALL: [McpFlag; 2] = [McpFlag::AllowWrite, McpFlag::AllowDictionaryRead];

    pub fn arg(self) -> &'static str {
        match self {
            McpFlag::AllowWrite => "--allow-write",
            McpFlag::AllowDictionaryRead => "--allow-dictionary-read",
        }
    }

    fn parse(arg: &str) -> Option<McpFlag> {
        McpFlag::ALL.into_iter().find(|flag| flag.arg() == arg)
    }
}

/// 去重并按 `McpFlag::ALL` 的顺序排好，同一组权限总是写成同一个条目。
fn canonical_capacity(input_len: usize) -> usize {
    input_len.min(McpFlag::ALL.len())
}

fn canonical(flags: &[McpFlag]) -> Vec<McpFlag> {
    let mut canonical = Vec::with_capacity(canonical_capacity(flags.len()));
    canonical.extend(McpFlag::ALL.into_iter().filter(|flag| flags.contains(flag)));
    canonical
}

#[derive(Debug, Serialize)]
pub struct McpClientStatus {
    pub id: McpClient,
    /// Where the configuration file is, for the page to show.
    pub path: String,
    /// 文件里的 `msime` 条目就是这里的服务器和运行时选项，只可能多了 `flags` 里的权限参数。
    pub configured: bool,
    /// 已写入条目带的权限参数，按固定顺序；未连接时为空。
    pub flags: Vec<McpFlag>,
    /// 已连接，但条目的命令是 Nix store 里的另一个路径（以前写进去的某一版），升级或垃圾回收后会失效；再写一次就换成 `command`，权限照旧。
    pub stale: bool,
}

/// 文件里已有的 `msime` 条目是这里写的：它带的权限参数，以及命令要不要换成这次的。
#[derive(Debug, Default, PartialEq, Eq)]
struct ExistingEntry {
    flags: Vec<McpFlag>,
    stale: bool,
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
    /// 已有的 `msime` 条目是这里的服务器，只是权限参数不同；改成了这次要的那组，不需要 `replace`。
    Updated,
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

/// Nix 的 store。这里的路径带着版本哈希，升级后不再是当前版本，垃圾回收后文件也没了，所以不写进助手配置。`/nix` 是指向别处的符号链接（Fedora Silverblue 这类根目录只读的系统）或用了自定义 `storeDir` 时，设置窗口的真实路径不以它开头，仍写 store 路径，与没做这层处理时相同。
const NIX_STORE: &str = "/nix/store";

/// 写进助手配置的 `msime-mcp`：`server_command` 在 Nix store 里时换成 PATH 上 Nix profile 的链接（比如 `/run/current-system/sw/bin/msime-mcp`），它随每次切换指向当前版本。
fn assistant_command(
    executable: &Path,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    server_command(executable).map(|command| stable_command_in(command, Path::new(NIX_STORE), env))
}

/// `command` 在 `store` 下时，返回 PATH 上第一个自己不在 `store` 下、解析后就是 `command` 这个文件的同名链接；没有时（比如只用 `nix run` 起了设置窗口）原样返回。只认同一个文件：用户级 profile（`~/.nix-profile/bin`、`/etc/profiles/per-user/<用户>/bin`）在 PATH 上排在系统的前面，里面可能是另装的旧版本，选了它助手就跑旧版本。
fn stable_command_in(
    command: PathBuf,
    store: &Path,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> PathBuf {
    if !command.starts_with(store) {
        return command;
    }
    let (Some(name), Some(path)) = (command.file_name(), env("PATH")) else {
        return command;
    };
    std::env::split_paths(&path)
        .filter(|directory| !directory.starts_with(store))
        .map(|directory| directory.join(name))
        .find(|candidate| same_program(candidate, &command))
        .unwrap_or(command)
}

/// The entry an assistant runs: the server and the runtime options, and no flags.
pub fn server_entry(command: &Path, options: &Path) -> Value {
    json!({
        "command": command.to_string_lossy(),
        "args": ["--options", options.to_string_lossy()],
    })
}

/// `base`（`server_entry` 的结果）在 `args` 末尾按固定顺序加上 `flags`。
pub fn entry_with_flags(base: &Value, flags: &[McpFlag]) -> Value {
    let mut entry = base.clone();
    if let Some(args) = entry.get_mut("args").and_then(Value::as_array_mut) {
        args.extend(canonical(flags).into_iter().map(|flag| json!(flag.arg())));
    }
    entry
}

fn split_entry_args(args: &[Value]) -> (Vec<McpFlag>, Vec<Value>) {
    let mut flags = Vec::with_capacity(canonical_capacity(args.len()));
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        match arg.as_str().and_then(McpFlag::parse) {
            Some(flag) if !flags.contains(&flag) => flags.push(flag),
            Some(_) => {}
            None => rest.push(arg.clone()),
        }
    }
    (flags, rest)
}

/// `existing` 是 `base` 加上若干权限参数时，返回这些参数（去重、按固定顺序）；命令、运行时选项或其它参数不同的条目不是这里写的，返回 `None`。命令指向的是同一个程序时（比如 Homebrew 放上 PATH 的 `msime-mcp`、手动链接的 `~/.local/bin/msime`，都是指向安装包里 `msime-mcp` 的符号链接），算作同一个命令。
///
/// 命令是 `store`（Nix store）里同名程序的条目也算作这里写的，不论是哪一版、文件还在不在，并标为 `stale`：它只能是以前的设置窗口写进去的，升级或垃圾回收后会失效，再写一次时换成 `base` 的命令，权限照旧。
fn existing_entry(existing: &Value, base: &Value, store: &Path) -> Option<ExistingEntry> {
    let args = existing.get("args")?.as_array()?;
    let (flags, rest) = split_entry_args(args);
    let mut stripped = existing.as_object()?.clone();
    stripped.insert("args".to_owned(), Value::Array(rest));
    let (stale, same) = match (
        stripped
            .get("command")
            .and_then(Value::as_str)
            .map(Path::new),
        base.get("command").and_then(Value::as_str),
    ) {
        (Some(command), Some(expected)) if command != Path::new(expected) => {
            let stale = command.starts_with(store)
                && command.file_name() == Path::new(expected).file_name();
            (stale, stale || same_program(command, Path::new(expected)))
        }
        _ => (false, false),
    };
    if same {
        stripped.insert("command".to_owned(), base["command"].clone());
    }
    (Value::Object(stripped) == *base).then(|| ExistingEntry {
        flags: canonical(&flags),
        stale,
    })
}

/// 两个绝对路径解析掉符号链接后是不是同一个文件。相对路径（比如只写了 `msime-mcp`、靠 PATH 找）不去猜：按当前目录解析可能碰巧对上一个不相干的文件。
fn same_program(command: &Path, expected: &Path) -> bool {
    let resolve = |path: &Path| {
        Some(path)
            .filter(|path| path.is_absolute())
            .and_then(|path| std::fs::canonicalize(path).ok())
    };
    matches!((resolve(command), resolve(expected)), (Some(a), Some(b)) if a == b)
}

/// `options` 这份运行时选项所属版本登记用的键（`Edition::mcp_server_name`）：full 的文档没有 `edition` 键，得到 [`SERVER_NAME`]。文档读不了或记录了不认识的版本时同样用 [`SERVER_NAME`]：这里只决定条目的名字，文档本身有没有问题由 `msime-mcp` 启动后去报告。
pub fn server_name(options: &Path) -> String {
    crate::bounded_file::open_private(options)
        .ok()
        .and_then(|file| crate::bounded_file::read(file, CONFIG_READ_LIMIT).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|document| msime_client_core::edition::Edition::of_host_options(&document))
        .map_or_else(
            || SERVER_NAME.to_owned(),
            |edition| edition.mcp_server_name(),
        )
}

/// 把条目包成两个助手（以及大多数别的助手）期望的样子，登记在 `name` 下。
pub fn config_snippet(name: &str, entry: &Value) -> String {
    let document = json!({ "mcpServers": { name: entry } });
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
    let mut clients = Vec::with_capacity(2);
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
    let command = assistant_command(executable, &env).ok_or("storage")?;
    let entry = options.map(|options| (server_name(options), server_entry(&command, options)));
    let clients = client_paths(&env)
        .into_iter()
        .map(|(id, path)| {
            let existing = entry
                .as_ref()
                .and_then(|(name, entry)| configured_entry(&path, name, entry));
            let configured = existing.is_some();
            let ExistingEntry { flags, stale } = existing.unwrap_or_default();
            McpClientStatus {
                id,
                configured,
                flags,
                stale,
                path: path.to_string_lossy().into_owned(),
            }
        })
        .collect();
    Ok(McpServerStatus {
        installed: command.is_file(),
        command: command.to_string_lossy().into_owned(),
        options: options.map(|path| path.to_string_lossy().into_owned()),
        config: entry
            .as_ref()
            .map(|(name, entry)| config_snippet(name, entry)),
        clients,
    })
}

/// 把 `executable` 旁边 `msime-mcp` 的条目（`args` 末尾加上 `flags`）写进 `client` 的配置文件；保留什么、什么时候替换见 `install`。
pub fn install_client(
    executable: &Path,
    options: Option<&Path>,
    client: McpClient,
    flags: &[McpFlag],
    replace: bool,
    env: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Result<InstallOutcome, &'static str> {
    let options = options.ok_or("mcp_options_missing")?;
    let command = assistant_command(executable, &env)
        .filter(|command| command.is_file())
        .ok_or("mcp_server_missing")?;
    let (_, path) = client_paths(&env)
        .into_iter()
        .find(|(id, _)| *id == client)
        .ok_or("mcp_client_missing")?;
    install(
        &path,
        &server_name(options),
        &server_entry(&command, options),
        flags,
        replace,
    )
}

/// The configuration as it is, or an empty object when there is no file yet.
fn read_config(path: &Path) -> Result<Map<String, Value>, &'static str> {
    let file = match crate::bounded_file::open_private(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(_) => return Err("storage"),
    };
    parse_config(file)
}

fn parse_config(file: std::fs::File) -> Result<Map<String, Value>, &'static str> {
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

#[cfg(unix)]
fn open_directory(path: &Path) -> std::io::Result<OwnedFd> {
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY
                | libc::O_DIRECTORY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

#[cfg(unix)]
fn open_config_at(
    directory: &OwnedFd,
    name: &std::ffi::OsStr,
) -> std::io::Result<Option<std::fs::File>> {
    let name = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let fd = unsafe {
        libc::openat(
            std::os::fd::AsRawFd::as_raw_fd(directory),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(error)
        }
    } else {
        Ok(Some(unsafe { std::fs::File::from_raw_fd(fd) }))
    }
}

#[cfg(unix)]
fn read_config_at(
    directory: &OwnedFd,
    name: &std::ffi::OsStr,
) -> Result<(Map<String, Value>, Option<std::fs::Permissions>), &'static str> {
    let Some(file) = open_config_at(directory, name).map_err(|_| "storage")? else {
        return Ok((Map::new(), None));
    };
    let metadata = file.metadata().map_err(|_| "storage")?;
    if !metadata.is_file() {
        return Err("storage");
    }
    let permissions = metadata.permissions();
    Ok((parse_config(file)?, Some(permissions)))
}

#[cfg(unix)]
fn write_config_at(
    directory: &OwnedFd,
    name: &std::ffi::OsStr,
    text: &[u8],
    permissions: Option<std::fs::Permissions>,
) -> Result<(), &'static str> {
    let mut temporary = std::ffi::OsString::from(".msime-mcp-");
    temporary.push(std::process::id().to_string());
    temporary.push("-");
    temporary.push(
        TEMPORARY_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .to_string(),
    );
    let temporary = std::ffi::CString::new(temporary.as_bytes()).map_err(|_| "storage")?;
    let target = std::ffi::CString::new(name.as_bytes()).map_err(|_| "storage")?;
    let fd = unsafe {
        libc::openat(
            std::os::fd::AsRawFd::as_raw_fd(directory),
            temporary.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err("storage");
    }
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let result = (|| {
        file.write_all(text).map_err(|_| "storage")?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions).map_err(|_| "storage")?;
        }
        file.sync_all().map_err(|_| "storage")?;
        let result = unsafe {
            libc::renameat(
                std::os::fd::AsRawFd::as_raw_fd(directory),
                temporary.as_ptr(),
                std::os::fd::AsRawFd::as_raw_fd(directory),
                target.as_ptr(),
            )
        };
        if result < 0 {
            return Err("storage");
        }
        Ok(())
    })();
    if result.is_err() {
        unsafe {
            libc::unlinkat(
                std::os::fd::AsRawFd::as_raw_fd(directory),
                temporary.as_ptr(),
                0,
            );
        }
    }
    result
}

/// 文件里 `name`（full 是 `msime`）条目是 `base` 加上若干权限参数时，返回这些参数和它要不要换命令（见 `existing_entry`）；没有条目、条目不是这里写的、或文件读不了时返回 `None`。
fn configured_entry(path: &Path, name: &str, base: &Value) -> Option<ExistingEntry> {
    let document = read_config(path).ok()?;
    existing_entry(
        document.get("mcpServers")?.get(name)?,
        base,
        Path::new(NIX_STORE),
    )
}

/// 把 `base`（`args` 末尾加上 `flags`）写到 `path` 文件的 `mcpServers.<name>` 下（full 是 `mcpServers.msime`），保留其它所有内容；多个版本各写各的键，互不覆盖。
///
/// 所在目录必须已经存在：它由助手自己创建，不存在说明没装这个助手，替用户建出来只会留下一个不存在的应用的目录。已有条目就是 `base` 只差权限参数时直接改成这次的参数（`Updated`），命令是 Nix store 里以前那一版的（`stale`）时同时换成 `base` 的命令；其它不同的 `msime` 条目只在 `replace` 时替换，否则以 `mcp_entry_exists` 失败，让设置页先问。符号链接（比如放在 dotfiles 仓库里的配置）会写穿到目标文件，而不是被替换掉。
pub fn install(
    path: &Path,
    name: &str,
    base: &Value,
    flags: &[McpFlag],
    replace: bool,
) -> Result<InstallOutcome, &'static str> {
    install_in(path, name, base, flags, replace, Path::new(NIX_STORE))
}

fn install_in(
    path: &Path,
    name: &str,
    base: &Value,
    flags: &[McpFlag],
    replace: bool,
    store: &Path,
) -> Result<InstallOutcome, &'static str> {
    let flags = canonical(flags);
    let target = match std::fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_owned(),
        Err(_) => return Err("storage"),
    };
    let directory = target.parent().ok_or("storage")?;
    if !directory.is_dir() {
        return Err("mcp_client_missing");
    }
    #[cfg(unix)]
    {
        let opened = open_directory(directory).map_err(|_| "storage")?;
        let file_name = target.file_name().ok_or("storage")?;
        install_in_open_directory(&opened, file_name, name, base, &flags, replace, store)
    }
    #[cfg(not(unix))]
    {
        let document = read_config(&target)?;
        let permissions = std::fs::metadata(&target)
            .ok()
            .map(|metadata| metadata.permissions());
        let (text, outcome) = updated_document(document, name, base, &flags, replace, store)?;
        let Some(text) = text else { return Ok(outcome) };
        let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| "storage")?;
        file.write_all(text.as_bytes()).map_err(|_| "storage")?;
        if let Some(permissions) = permissions {
            file.as_file()
                .set_permissions(permissions)
                .map_err(|_| "storage")?;
        }
        file.as_file().sync_all().map_err(|_| "storage")?;
        file.persist(&target).map_err(|_| "storage")?;
        Ok(outcome)
    }
}

#[cfg(unix)]
fn install_in_open_directory(
    directory: &OwnedFd,
    file_name: &std::ffi::OsStr,
    name: &str,
    base: &Value,
    flags: &[McpFlag],
    replace: bool,
    store: &Path,
) -> Result<InstallOutcome, &'static str> {
    let (document, permissions) = read_config_at(directory, file_name)?;
    let (text, outcome) = updated_document(document, name, base, flags, replace, store)?;
    if let Some(text) = text {
        write_config_at(directory, file_name, text.as_bytes(), permissions)?;
    }
    Ok(outcome)
}

fn updated_document(
    mut document: Map<String, Value>,
    name: &str,
    base: &Value,
    flags: &[McpFlag],
    replace: bool,
    store: &Path,
) -> Result<(Option<String>, InstallOutcome), &'static str> {
    let mut entry = entry_with_flags(base, flags);
    let servers = document
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("mcp_config_invalid")?;
    let outcome = match servers.get(name) {
        None => InstallOutcome::Added,
        Some(existing) => match existing_entry(existing, base, store) {
            Some(current) if !current.stale && current.flags == flags => {
                return Ok((None, InstallOutcome::Unchanged));
            }
            Some(current) => {
                // 只改权限参数，命令保留用户写的那个：它可能是指向同一个程序的符号链接，是用户自己选的入口。store 里以前那一版的路径换成这次的命令。
                if let Some(command) = existing.get("command").filter(|_| !current.stale) {
                    entry["command"] = command.clone();
                }
                InstallOutcome::Updated
            }
            None if replace => InstallOutcome::Replaced,
            None => return Err("mcp_entry_exists"),
        },
    };
    servers.insert(name.to_owned(), entry);
    let mut text = serde_json::to_string_pretty(&Value::Object(document)).map_err(|_| "storage")?;
    text.push('\n');
    Ok((Some(text), outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn install_stays_in_open_directory_after_its_path_is_replaced() {
        use std::ffi::OsStr;
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original");
        let moved = root.path().join("moved");
        let outside = root.path().join("outside");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let outside_config = outside.join("mcp.json");
        std::fs::write(&outside_config, b"{\"synthetic\":true}").unwrap();
        let directory = open_directory(&original).unwrap();

        std::fs::rename(&original, &moved).unwrap();
        symlink(&outside, &original).unwrap();
        assert_eq!(
            install_in_open_directory(
                &directory,
                OsStr::new("mcp.json"),
                SERVER_NAME,
                &entry(),
                &[],
                false,
                Path::new(NIX_STORE),
            ),
            Ok(InstallOutcome::Added)
        );
        assert_eq!(
            std::fs::read(&outside_config).unwrap(),
            b"{\"synthetic\":true}"
        );
        assert_eq!(
            configured_flags(&moved.join("mcp.json"), SERVER_NAME, &entry()),
            Some(vec![])
        );
    }

    #[test]
    fn canonical_flags_reserve_every_known_slot() {
        assert_eq!(canonical_capacity(0), 0);
        assert_eq!(canonical_capacity(McpFlag::ALL.len()), McpFlag::ALL.len());
        assert_eq!(
            canonical_capacity(McpFlag::ALL.len() + 10),
            McpFlag::ALL.len()
        );
    }

    fn configured_flags(path: &Path, name: &str, base: &Value) -> Option<Vec<McpFlag>> {
        configured_entry(path, name, base).map(|existing| existing.flags)
    }

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
        let snippet: Value = serde_json::from_str(&config_snippet(SERVER_NAME, &entry())).unwrap();
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
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
            Ok(InstallOutcome::Added)
        );
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written, json!({ "mcpServers": { "msime": entry() } }));
        assert_eq!(configured_flags(&path, SERVER_NAME, &entry()), Some(vec![]));
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
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
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
            Ok(InstallOutcome::Added)
        );
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
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
            Err("mcp_entry_exists")
        );
        let unchanged: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(unchanged, before);
        assert_eq!(configured_flags(&path, SERVER_NAME, &entry()), None);
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], true),
            Ok(InstallOutcome::Replaced)
        );
        assert_eq!(configured_flags(&path, SERVER_NAME, &entry()), Some(vec![]));
    }

    #[test]
    fn the_chosen_flags_are_written_in_a_fixed_order_and_read_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        let both = [McpFlag::AllowDictionaryRead, McpFlag::AllowWrite];
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &both, false),
            Ok(InstallOutcome::Added)
        );
        let written: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            written["mcpServers"]["msime"]["args"],
            json!([
                "--options",
                "/state/runtime-options.json",
                "--allow-write",
                "--allow-dictionary-read"
            ])
        );
        assert_eq!(
            configured_flags(&path, SERVER_NAME, &entry()),
            Some(vec![McpFlag::AllowWrite, McpFlag::AllowDictionaryRead])
        );
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &both, false),
            Ok(InstallOutcome::Unchanged)
        );
        // 只差权限参数的条目是这里写的，直接改成新的一组，不需要 replace。
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[McpFlag::AllowWrite], false),
            Ok(InstallOutcome::Updated)
        );
        assert_eq!(
            configured_flags(&path, SERVER_NAME, &entry()),
            Some(vec![McpFlag::AllowWrite])
        );
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
            Ok(InstallOutcome::Updated)
        );
        assert_eq!(configured_flags(&path, SERVER_NAME, &entry()), Some(vec![]));
    }

    #[test]
    fn splitting_entry_args_keeps_known_flags_bounded() {
        let args = vec![json!("--options"), json!("/state/runtime-options.json")];
        let mut args = args;
        args.extend((0..128).map(|_| json!("--allow-write")));

        let (flags, rest) = split_entry_args(&args);
        assert_eq!(flags, vec![McpFlag::AllowWrite]);
        assert!(flags.len() <= McpFlag::ALL.len());
        assert_eq!(rest, args[..2]);
    }

    #[test]
    #[cfg(unix)]
    fn a_command_linked_to_the_packaged_server_is_this_server() {
        let directory = tempfile::tempdir().unwrap();
        let server = directory.path().join("msime-mcp");
        std::fs::write(&server, b"").unwrap();
        let link = directory.path().join("msime");
        std::os::unix::fs::symlink(&server, &link).unwrap();
        let copy = directory.path().join("copy");
        std::fs::write(&copy, b"").unwrap();
        let options = Path::new("/state/runtime-options.json");
        let base = server_entry(&server, options);
        let path = directory.path().join("mcp.json");
        let write = |command: &Path| {
            let document = json!({ "mcpServers": { "msime": {
                "command": command.to_string_lossy(),
                "args": ["--options", "/state/runtime-options.json", "--allow-write"],
            } } });
            std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        };
        // 指向安装包里 msime-mcp 的符号链接就是这里的服务器：设置页显示已连接，改权限时直接改，不必先确认替换。
        write(&link);
        assert_eq!(
            configured_flags(&path, SERVER_NAME, &base),
            Some(vec![McpFlag::AllowWrite])
        );
        assert_eq!(
            install(&path, SERVER_NAME, &base, &[McpFlag::AllowWrite], false),
            Ok(InstallOutcome::Unchanged)
        );
        assert_eq!(
            install(
                &path,
                SERVER_NAME,
                &base,
                &[McpFlag::AllowWrite, McpFlag::AllowDictionaryRead],
                false
            ),
            Ok(InstallOutcome::Updated)
        );
        let document: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            document["mcpServers"]["msime"]["command"],
            link.to_string_lossy().as_ref()
        );
        // 另一份同样内容的副本是另一个程序，可能是别的版本，不当作这里的。
        write(&copy);
        assert_eq!(configured_flags(&path, SERVER_NAME, &base), None);
        // 相对路径不按当前目录去解析。
        write(Path::new("msime-mcp"));
        assert_eq!(configured_flags(&path, SERVER_NAME, &base), None);
    }

    #[test]
    fn flags_added_by_hand_are_read_in_any_order_and_other_args_make_the_entry_foreign() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        let write = |args: Value| {
            let document = json!({ "mcpServers": { "msime": {
                "command": "/opt/msime/msime-mcp",
                "args": args,
            } } });
            std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        };
        write(json!([
            "--allow-dictionary-read",
            "--options",
            "/state/runtime-options.json",
            "--allow-write",
            "--allow-write"
        ]));
        assert_eq!(
            configured_flags(&path, SERVER_NAME, &entry()),
            Some(vec![McpFlag::AllowWrite, McpFlag::AllowDictionaryRead])
        );
        assert_eq!(
            install(
                &path,
                SERVER_NAME,
                &entry(),
                &[McpFlag::AllowWrite, McpFlag::AllowDictionaryRead],
                false
            ),
            Ok(InstallOutcome::Unchanged)
        );
        // 不认识的参数说明条目被别人改过，不当作这里写的，改它要先确认替换。
        write(json!([
            "--options",
            "/state/runtime-options.json",
            "--allow-write",
            "--verbose"
        ]));
        assert_eq!(configured_flags(&path, SERVER_NAME, &entry()), None);
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[McpFlag::AllowWrite], false),
            Err("mcp_entry_exists")
        );
        // 指向另一份运行时选项的条目同样不是这里的。
        write(json!(["--options", "/elsewhere.json", "--allow-write"]));
        assert_eq!(configured_flags(&path, SERVER_NAME, &entry()), None);
    }

    #[test]
    fn the_status_reports_the_flags_a_configured_client_has() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".cursor")).unwrap();
        let executable = home.path().join("msime-desktop");
        let options = Path::new("/state/runtime-options.json");
        let env = |name: &str| {
            matches!(name, "HOME" | "USERPROFILE").then(|| home.path().as_os_str().to_owned())
        };
        let cursor = |status: &McpServerStatus| {
            let client = status
                .clients
                .iter()
                .find(|client| client.id == McpClient::Cursor)
                .unwrap();
            (client.configured, client.flags.clone())
        };
        let before = status(&executable, Some(options), env).unwrap();
        assert_eq!(cursor(&before), (false, vec![]));

        let command = server_command(&executable).unwrap();
        let path = home.path().join(".cursor").join("mcp.json");
        install(
            &path,
            SERVER_NAME,
            &server_entry(&command, options),
            &[McpFlag::AllowDictionaryRead],
            false,
        )
        .unwrap();
        let after = status(&executable, Some(options), env).unwrap();
        assert_eq!(cursor(&after), (true, vec![McpFlag::AllowDictionaryRead]));
        let serialized = serde_json::to_value(&after.clients).unwrap();
        let serialized = serialized
            .as_array()
            .unwrap()
            .iter()
            .find(|client| client["id"] == "cursor")
            .unwrap();
        assert_eq!(serialized["flags"], json!(["--allow-dictionary-read"]));
    }

    /// 五笔版的运行时选项记录了版本，条目登记在 `msime-wubi` 下，旁边 full 的 `msime` 条目不受影响；full 的运行时选项仍用 `msime`。
    #[test]
    fn each_edition_registers_under_its_own_name() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".cursor")).unwrap();
        let executable = home.path().join("msime-desktop");
        let command = server_command(&executable).unwrap();
        std::fs::write(&command, b"").unwrap();
        let env = |name: &str| {
            matches!(name, "HOME" | "USERPROFILE").then(|| home.path().as_os_str().to_owned())
        };
        let full_options = home.path().join("full-runtime-options.json");
        std::fs::write(&full_options, br#"{"api_version":1}"#).unwrap();
        let wubi_options = home.path().join("wubi-runtime-options.json");
        std::fs::write(&wubi_options, br#"{"api_version":1,"edition":"wubi"}"#).unwrap();
        assert_eq!(server_name(&full_options), SERVER_NAME);
        assert_eq!(server_name(&wubi_options), "msime-wubi");
        assert_eq!(server_name(&home.path().join("missing.json")), SERVER_NAME);

        let install = |options: &Path| {
            install_client(
                &executable,
                Some(options),
                McpClient::Cursor,
                &[],
                false,
                env,
            )
        };
        assert_eq!(install(&full_options), Ok(InstallOutcome::Added));
        assert_eq!(install(&wubi_options), Ok(InstallOutcome::Added));
        let document: Value = serde_json::from_slice(
            &std::fs::read(home.path().join(".cursor").join("mcp.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            document["mcpServers"]["msime"],
            server_entry(&command, &full_options)
        );
        assert_eq!(
            document["mcpServers"]["msime-wubi"],
            server_entry(&command, &wubi_options)
        );

        let status = status(&executable, Some(&wubi_options), env).unwrap();
        let snippet: Value = serde_json::from_str(status.config.as_deref().unwrap()).unwrap();
        assert!(snippet["mcpServers"].get("msime-wubi").is_some());
        assert!(status
            .clients
            .iter()
            .any(|client| client.id == McpClient::Cursor && client.configured));
    }

    #[test]
    fn a_file_that_is_not_a_json_object_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mcp.json");
        for text in ["{ \"mcpServers\": ", "[]", "{\"mcpServers\": []}"] {
            std::fs::write(&path, text).unwrap();
            assert_eq!(
                install(&path, SERVER_NAME, &entry(), &[], true),
                Err("mcp_config_invalid")
            );
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
        std::fs::write(&path, "\n").unwrap();
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
            Ok(InstallOutcome::Added)
        );
    }

    #[test]
    fn a_missing_assistant_directory_is_not_created() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".cursor").join("mcp.json");
        assert_eq!(
            install(&path, SERVER_NAME, &entry(), &[], false),
            Err("mcp_client_missing")
        );
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
        assert_eq!(
            install(&link, SERVER_NAME, &entry(), &[], false),
            Ok(InstallOutcome::Added)
        );
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(configured_flags(&real, SERVER_NAME, &entry()), Some(vec![]));
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    /// Nix 装的设置窗口旁边的 `msime-mcp` 在 store 里；条目改用 PATH 上的 profile 链接，升级后仍指向当前版本。
    #[cfg(unix)]
    #[test]
    fn a_server_in_the_nix_store_is_registered_through_the_profile_link() {
        let root = tempfile::tempdir().unwrap();
        let directory = |path: &[&str]| {
            let directory = path
                .iter()
                .fold(root.path().to_owned(), |at, part| at.join(part));
            std::fs::create_dir_all(&directory).unwrap();
            directory
        };
        let path_env = |directories: &[&PathBuf]| {
            let path = std::env::join_paths(directories).unwrap();
            move |name: &str| (name == "PATH").then(|| path.clone())
        };
        let store = directory(&["store"]);
        let server = directory(&["store", "hash-msime-fcitx5", "bin"]).join("msime-mcp");
        std::fs::write(&server, b"").unwrap();
        // PATH 上依次是：store 里另一个包的 bin、用户 profile 里另装的旧版本、不是 Nix 装的副本、空目录、系统 profile 的链接。
        let other_package = directory(&["store", "hash-other", "bin"]);
        std::os::unix::fs::symlink(&server, other_package.join("msime-mcp")).unwrap();
        let old = directory(&["store", "hash-msime-mcp-old", "bin"]).join("msime-mcp");
        std::fs::write(&old, b"").unwrap();
        let user_profile = directory(&["user-profile-bin"]);
        std::os::unix::fs::symlink(&old, user_profile.join("msime-mcp")).unwrap();
        let system = directory(&["usr-bin"]);
        let packaged = system.join("msime-mcp");
        std::fs::write(&packaged, b"").unwrap();
        let empty = directory(&["empty"]);
        let profile = directory(&["profile-bin"]);
        let link = profile.join("msime-mcp");
        std::os::unix::fs::symlink(&server, &link).unwrap();
        let env = path_env(&[&other_package, &user_profile, &system, &empty, &profile]);
        assert_eq!(stable_command_in(server.clone(), &store, &env), link);

        // 没有指向这个文件的链接时仍用 store 里的路径，即使有别的版本的链接。
        assert_eq!(
            stable_command_in(
                server.clone(),
                &store,
                path_env(&[&user_profile, &system, &empty])
            ),
            server
        );
        assert_eq!(
            stable_command_in(server.clone(), &store, |_: &str| None),
            server
        );
        // 不在 store 里的安装（deb、rpm、Homebrew）原样使用，即使 PATH 上有 profile 链接。
        assert_eq!(stable_command_in(packaged.clone(), &store, &env), packaged);
    }

    /// 以前写进配置的 store 路径，不论哪一版、文件还在不在，都算这里写的但已过期：设置页显示已连接并提示更新，更新时换成这次的命令，权限照旧，不用确认替换。
    #[test]
    fn a_store_path_entry_is_stale_and_updated_to_the_profile_link() {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("mcp.json");
        let store = Path::new("/store");
        let options = Path::new("/state/runtime-options.json");
        let base = server_entry(Path::new("/profile/bin/msime-mcp"), options);
        let existing = |command: &str, flags: &[McpFlag]| {
            entry_with_flags(&server_entry(Path::new(command), options), flags)
        };
        let found = |entry: &Value, base: &Value| {
            existing_entry(entry, base, store).map(|found| (found.flags, found.stale))
        };
        let write = || {
            install_in(
                &config,
                SERVER_NAME,
                &base,
                &[McpFlag::AllowWrite],
                false,
                store,
            )
        };
        let old = existing("/store/hash-old/bin/msime-mcp", &[McpFlag::AllowWrite]);
        assert_eq!(found(&old, &base), Some((vec![McpFlag::AllowWrite], true)));
        std::fs::write(
            &config,
            serde_json::to_vec(&json!({ "mcpServers": { "msime": old } })).unwrap(),
        )
        .unwrap();
        // 权限没变也要写：命令得换掉。
        assert_eq!(write(), Ok(InstallOutcome::Updated));
        let written: Value = serde_json::from_slice(&std::fs::read(&config).unwrap()).unwrap();
        let written = &written["mcpServers"]["msime"];
        assert_eq!(*written, entry_with_flags(&base, &[McpFlag::AllowWrite]));
        assert_eq!(
            found(written, &base),
            Some((vec![McpFlag::AllowWrite], false))
        );
        assert_eq!(write(), Ok(InstallOutcome::Unchanged));

        // 找不到 profile 链接、这次也写 store 路径时，原样相同的不算过期，别的版本仍算。
        let current = server_entry(Path::new("/store/hash-new/bin/msime-mcp"), options);
        assert_eq!(found(&current, &current), Some((vec![], false)));
        assert_eq!(
            found(&old, &current),
            Some((vec![McpFlag::AllowWrite], true))
        );
        // store 里别的程序、运行时选项不同、或不在 store 里又不是同一个程序的，都不是这里写的。
        for other in [
            existing("/store/hash-old/bin/other-mcp", &[]),
            server_entry(
                Path::new("/store/hash-old/bin/msime-mcp"),
                Path::new("/elsewhere/runtime-options.json"),
            ),
            existing("/usr/bin/msime-mcp", &[]),
        ] {
            assert_eq!(found(&other, &base), None);
        }
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
