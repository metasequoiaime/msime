//! 云剪贴板：列表、上传和设备批准都经端到端加密驱动（`cloud::clipboard_e2e`），账号会话只负责带上凭据发请求、保存密钥串。协议和宿主的职责见 `docs/cloud-clipboard-e2e.md`。置顶、删除、清空和开关不经过驱动，仍在 `client.rs` 里直接调用。

use super::validate::*;
use super::*;
use crate::cloud::clipboard_e2e::{self, DriverError, Operation};
use serde_json::Value;

/// 云剪贴板全部接口的路径前缀，驱动只会请求它下面的路径。
pub(crate) const CLIPBOARD_PATH: &str = "/v1/users/me/clipboard";
/// 云剪贴板响应的上限：列表最多 50 条，每条密文最长 16384 个字符，再加上字段名和其余字段。
pub(crate) const MAX_CLIPBOARD_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CLIPBOARD_DEVICES: usize = 20;

/// 驱动和宿主会交给共享面板的错误码，原样转交，面板据此给出对应的提示。账号层和自建中继自己的错误码也在其中，macOS 的原生 provider 可以直接回报它们。
const CLIPBOARD_ERROR_CODES: &[&str] = &[
    "clipboard_device_pending",
    "clipboard_sync_disabled",
    "clipboard_key_conflict",
    "clipboard_device_limit",
    "clipboard_e2e_required",
    "invalid_clipboard_device",
    "invalid_clipboard_key",
    "invalid_clipboard_ciphertext",
    "invalid_clipboard_text",
    "invalid_device_id",
    "invalid_search",
    "invalid_response",
    "invalid_state",
    "not_found",
    "account_invalid",
    "account_unauthorized",
    "account_rate_limited",
    "account_storage",
    "account_cancelled",
    "account_conflict",
    "account_unavailable",
    "account_banned",
    // 自建中继（`crate::clipboard_relay`）：五十条都已置顶、新的一条放不下，令牌被拒、没有配置中继、连不上中继、本机密钥串存不了。
    "clipboard_full",
    "relay_unauthorized",
    "relay_not_configured",
    "relay_unavailable",
    "relay_storage",
];

/// 把一个云剪贴板错误码换成宿主能长期持有的静态字符串；不认识的码返回 `None`，由调用方按自己的方式归类。
pub fn cloud_clipboard_error_code(code: &str) -> Option<&'static str> {
    CLIPBOARD_ERROR_CODES
        .iter()
        .copied()
        .find(|known| *known == code)
}

/// 云剪贴板操作没有完成的原因。
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AccountClipboardError {
    /// 没有发出去或没有拿到响应：未登录、存储不可用、断网等，与其他账号操作相同。
    #[error(transparent)]
    Account(#[from] AccountError),
    /// 驱动给出的结果，例如本机还在等待批准（`clipboard_device_pending`）或服务端拒绝了请求。
    #[error("cloud clipboard operation failed: {}", .0.code)]
    Clipboard(DriverError),
}

impl AccountClipboardError {
    /// 交给共享面板的错误码。驱动给出的已知码原样保留，其余的按服务端的状态码归入账号错误。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Account(error) => error.code(),
            Self::Clipboard(error) => {
                cloud_clipboard_error_code(&error.code).unwrap_or_else(|| {
                    let body = serde_json::json!({ "error": { "code": error.code } }).to_string();
                    AccountError::from_http_response(error.status, body.as_bytes()).code()
                })
            }
        }
    }
}

/// 登记云剪贴板设备时用的名称，会显示在其他设备的批准提示里。能读到计算机名时用它，否则用平台名。
pub fn cloud_clipboard_device_name() -> String {
    computer_name().unwrap_or_else(|| platform_name().to_owned())
}

fn platform_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Mac"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "ios") {
        "iOS"
    } else if cfg!(target_os = "android") {
        "Android"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else {
        "水杉输入法"
    }
}

/// 不调用系统接口就能拿到的计算机名：Windows 的 `COMPUTERNAME`，Linux 的内核主机名。macOS 和移动端没有这样的来源，用平台名。
fn computer_name() -> Option<String> {
    #[cfg(target_os = "windows")]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(target_os = "linux")]
    let name = ["/proc/sys/kernel/hostname", "/etc/hostname"]
        .iter()
        .find_map(|path| {
            let mut text = String::new();
            std::fs::File::open(path)
                .ok()?
                .take(256)
                .read_to_string(&mut text)
                .ok()?;
            Some(text)
        });
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let name: Option<String> = None;
    let name = name?.trim().to_owned();
    let usable = !name.is_empty()
        && name.chars().count() <= 64
        && !name.chars().any(char::is_control)
        && name != "localhost"
        && !name.starts_with("localhost.");
    usable.then_some(name)
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, AccountError> {
    serde_json::from_value(value).map_err(|_| AccountError::Unavailable)
}

pub(crate) fn validate_device_id(device_id: &str) -> Result<(), AccountError> {
    if crate::text::is_lower_hex(device_id, 32) {
        Ok(())
    } else {
        Err(AccountError::Invalid)
    }
}

pub(crate) fn validate_devices(value: &AccountClipboardDevices) -> Result<(), AccountError> {
    if value.devices.len() > MAX_CLIPBOARD_DEVICES {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

impl<A: AccountApi, S: AccountSessionStorage> BackendAccountSession<A, S> {
    /// 解密后的云端条目，`search` 由驱动在本机解密后匹配。本机还在等待批准时 `e2e.status` 为 `pending`、条目为空。
    pub fn clipboard(&self, search: &str) -> Result<AccountClipboardPage, AccountClipboardError> {
        validate_clipboard_search(search)?;
        let page: AccountClipboardPage = decode(self.clipboard_e2e(Operation::List {
            search: search.to_owned(),
        })?)?;
        validate_clipboard_page(&page)?;
        Ok(page)
    }

    /// 在本机加密后上传一条。本机还没有获准时失败，错误码为 `clipboard_device_pending`。
    pub fn add_clipboard(&self, text: &str) -> Result<AccountClipboardItem, AccountClipboardError> {
        validate_clipboard_text(text)?;
        let item: AccountClipboardItem = decode(self.clipboard_e2e(Operation::Add {
            text: text.to_owned(),
        })?)?;
        validate_clipboard_item(&item)?;
        Ok(item)
    }

    /// 本机状态、核对码和账号下登记的全部设备。
    pub fn clipboard_devices(&self) -> Result<AccountClipboardDevices, AccountClipboardError> {
        self.clipboard_device_operation(Operation::Status)
    }

    /// 把当前剪贴板密钥交给一台待批准的设备。
    pub fn approve_clipboard_device(
        &self,
        device_id: &str,
    ) -> Result<AccountClipboardDevices, AccountClipboardError> {
        validate_device_id(device_id)?;
        self.clipboard_device_operation(Operation::Approve {
            device_id: device_id.to_owned(),
        })
    }

    /// 删除一台待批准设备的登记。
    pub fn reject_clipboard_device(
        &self,
        device_id: &str,
    ) -> Result<AccountClipboardDevices, AccountClipboardError> {
        validate_device_id(device_id)?;
        self.clipboard_device_operation(Operation::Reject {
            device_id: device_id.to_owned(),
        })
    }

    /// 移除一台设备并换钥：云端条目清空，其他设备都要在本机上重新批准。
    pub fn remove_clipboard_device(
        &self,
        device_id: &str,
    ) -> Result<AccountClipboardDevices, AccountClipboardError> {
        validate_device_id(device_id)?;
        self.clipboard_device_operation(Operation::Remove {
            device_id: device_id.to_owned(),
        })
    }

    /// 重置云剪贴板：换一把只给本机的新密钥，云端条目清空，其他设备都要重新批准。
    pub fn reset_clipboard(&self) -> Result<AccountClipboardDevices, AccountClipboardError> {
        self.clipboard_device_operation(Operation::Reset)
    }

    fn clipboard_device_operation(
        &self,
        operation: Operation,
    ) -> Result<AccountClipboardDevices, AccountClipboardError> {
        let devices: AccountClipboardDevices = decode(self.clipboard_e2e(operation)?)?;
        validate_devices(&devices)?;
        Ok(devices)
    }

    /// 用当前登录的账号跑完一个驱动操作。每个请求带上访问令牌；令牌过期（401）时刷新一次再发同一个请求，其他状态码连同正文原样交给驱动。驱动交回的密钥串在发出下一个请求之前保存，保存和读取都在登录代次不变时进行，退出登录后不会再写回一份密钥串。
    fn clipboard_e2e(&self, operation: Operation) -> Result<Value, AccountClipboardError> {
        let (user_id, mut token, generation) = self.credentials_with_generation(None, None)?;
        let keystore = self.with_generation(generation, Some(&user_id), || {
            self.storage.load_clipboard_keystore()
        })?;
        let device_name = cloud_clipboard_device_name();
        let result = clipboard_e2e::run(
            operation,
            keystore.as_deref(),
            &device_name,
            |request| {
                let response = self.api.clipboard_e2e(request, &token)?;
                if response.status != 401 {
                    return Ok(response);
                }
                let (_, replacement, replacement_generation) =
                    self.credentials_with_generation(Some(&token), Some(&user_id))?;
                if replacement_generation != generation {
                    return Err(AccountError::Cancelled);
                }
                token = replacement;
                self.api.clipboard_e2e(request, &token)
            },
            |value| {
                self.with_generation(generation, Some(&user_id), || {
                    self.storage.save_clipboard_keystore(value)
                })
            },
            || AccountError::Unavailable,
        )?;
        // 与其他账号操作一样：结果只交给发起它的那次登录。
        self.with_generation(generation, Some(&user_id), || Ok(()))?;
        result.map_err(AccountClipboardError::Clipboard)
    }
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
