//! Registering `msime-mcp` with an AI assistant, for a settings host that is not the desktop shell: the WinUI settings window on Windows. The desktop shell calls `crate::mcp_clients` directly.
//!
//! The server is looked for beside the calling executable, where every package puts it next to the settings binary.

use crate::mcp_clients::{self, McpClient, McpFlag};
use crate::*;

const MCP_REQUEST_LIMIT: usize = 65_536;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusRequest {
    /// The runtime options the entry points the server at; absent before the input method is set up.
    #[serde(default)]
    options: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallRequest {
    #[serde(default)]
    options: Option<String>,
    client: McpClient,
    /// 写进条目 `args` 的权限参数；省略时写只读条目。
    #[serde(default)]
    flags: Vec<McpFlag>,
    #[serde(default)]
    replace: bool,
}

fn mcp_request<T: serde::de::DeserializeOwned>(
    request: *const u8,
    length: usize,
) -> Result<T, String> {
    if request.is_null() || length == 0 || length > MCP_REQUEST_LIMIT {
        return Err("invalid mcp request buffer".into());
    }
    // SAFETY: the caller contract of every entry point using this guarantees `length` readable bytes; null and size are checked above.
    let bytes = unsafe { std::slice::from_raw_parts(request, length) };
    serde_json::from_slice(bytes).map_err(|_| "invalid mcp request".to_owned())
}

fn options_path(options: Option<&str>) -> Result<Option<&Path>, String> {
    match options {
        None => Ok(None),
        Some(options) if Path::new(options).is_absolute() => Ok(Some(Path::new(options))),
        Some(_) => Err("mcp options must be absolute".into()),
    }
}

fn executable() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|_| "storage".to_owned())
}

/// `msime-mcp` beside the calling executable, the entry to paste into an assistant, and whether each assistant offered on this platform already has it.
///
/// Request `{"options": "<absolute runtime options path>" | null}`; response is `mcp_clients::McpServerStatus`. Reads the assistants' configuration files, so call it off the UI thread.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_mcp_status(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        let request: StatusRequest = mcp_request(request, length)?;
        let options = options_path(request.options.as_deref())?;
        let status = mcp_clients::status(&executable()?, options, |name| std::env::var_os(name))?;
        serde_json::to_value(status).map_err(|error| error.to_string())
    })
}

/// 把条目写进一个助手的配置文件，保留文件里其它所有内容。
///
/// 请求 `{"options": "<绝对路径>", "client": "claude_desktop" | "cursor", "flags": ["--allow-write", "--allow-dictionary-read"], "replace": false}`，`flags` 省略时写只读条目；响应 `"added" | "updated" | "replaced" | "unchanged"`。已有条目只差权限参数时改成这次的参数（`updated`）；其它不同的 `msime` 条目在未设 `replace` 时以 `mcp_entry_exists` 失败，由宿主先问再覆盖。其余错误码是 `mcp_client_missing`、`mcp_config_invalid`、`mcp_server_missing`、`mcp_options_missing` 和 `storage`。会写文件，请在 UI 线程之外调用。
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_mcp_install(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        let request: InstallRequest = mcp_request(request, length)?;
        let options = options_path(request.options.as_deref())?;
        let outcome = mcp_clients::install_client(
            &executable()?,
            options,
            request.client,
            &request.flags,
            request.replace,
            |name| std::env::var_os(name),
        )?;
        serde_json::to_value(outcome).map_err(|error| error.to_string())
    })
}
