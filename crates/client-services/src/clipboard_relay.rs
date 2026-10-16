//! 自建中继上的云剪贴板。
//!
//! 中继实现的是 `docs/cloud-clipboard-e2e.md` 里同一组接口，只是挂在用户填的基地址下，凭据换成中继管理员发的静态令牌，协议差异见 `docs/cloud-clipboard-relay.md`。列表、上传和设备批准照旧经端到端加密驱动（[`clipboard_e2e::run`]），中继只见到密文；开关、删除、清空、置顶和保留期直接请求。
//!
//! 这个模块自己联网，所以不放在只做 sans-IO 的 `cloud` 下面（见 `cloud.rs` 开头）。它不复用 [`crate::account::BackendAccountClient`]：那个客户端按 `origin.join(path)` 拼地址，会丢掉基地址里的路径前缀，user agent 也是固定的，而账号、词库、头像等全部请求都经过它，为中继改它会牵连所有账号路径。
//!
//! 本机的密钥串单独存一份（[`RELAY_KEYSTORE_FILE`]），与水杉账号的那份互不相干，退出账号也不删除它。文件里记着它属于哪个中继地址和哪个令牌空间，换了任何一个都当作没有密钥串，驱动会生成新的设备身份。
//!
//! 这份文件只靠文件权限保护。能把密钥串交给系统密钥库加密的宿主不用 [`RelayClipboard`]：Android 自己跑驱动、自己用 AndroidKeyStore 加密保存密钥串，只借 [`RelayTransport`] 发请求（经 `msime_client_relay_cloud_clipboard_exchange`），与它的水杉账号路径一样。

use crate::account::{
    AccountClipboardDevices, AccountClipboardItem, AccountClipboardPage, AccountError,
    AccountSessionFileLayout, FileAccountSessionStorage, CLIPBOARD_PATH,
    MAX_CLIPBOARD_RESPONSE_BYTES, MAX_ERROR_BODY_BYTES,
};
use crate::cloud::clipboard_e2e::{self, DriverError, HttpResponse, Operation};
use crate::preferences::{parse_relay_url, CloudClipboardRelayPreferences};
/// 中继地址和令牌的校验定义在 `preferences`（偏好校验要用，放在这里会让 `preferences` 依赖本模块），从这里导回。
pub use crate::preferences::{valid_relay_token, valid_relay_url};
use reqwest::blocking::Client;
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// 本机密钥串的文件名，放在宿主给的目录里，仅本人可读写。
pub const RELAY_KEYSTORE_FILE: &str = "cloud-clipboard-relay-keystore";
/// 同一台设备上的几个进程（输入法、设置窗口、面板）同时用中继时串行，免得第一次使用时各自登记出一台设备。
const RELAY_LOCK_FILE: &str = "cloud-clipboard-relay.lock";
/// 文件里是基地址、空间编号和约 130 个字符的密钥串，明显更大的不是这里写的。
const MAX_KEYSTORE_FILE_BYTES: u64 = 8192;
/// 驱动发出的请求体最多是 20 个封装或一条 16384 个字符的密文，远小于这个上限。
const MAX_REQUEST_BYTES: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// 等另一个进程跑完一次操作。一次操作最多发几次请求，每次 30 秒超时，正常情况下几秒内就能拿到锁。
const LOCK_TIMEOUT: Duration = Duration::from_secs(60);
const LOCK_POLL: Duration = Duration::from_millis(50);
/// 保留期允许的天数，0 表示一直保留，与官方服务端相同。
pub const RETENTION_DAYS: [u32; 4] = [0, 1, 7, 30];

/// 自建中继出错的原因。
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RelayError {
    /// 偏好里没有配置中继，或者配置的地址、令牌不合规。
    #[error("cloud clipboard relay is not configured")]
    NotConfigured,
    /// 中继以 401 拒绝了令牌。中继没有刷新令牌这回事，要用户换一个。
    #[error("cloud clipboard relay rejected the token")]
    Unauthorized,
    /// 请求本身不合规（动作、条目编号、保留天数等），没有发出去。
    #[error("invalid cloud clipboard relay request")]
    Invalid,
    /// 没有发出去或没有拿到可用的响应：断网、超时、正文过大或格式不对。
    #[error("cloud clipboard relay is unavailable")]
    Unavailable,
    /// 本机的密钥串或锁文件读写失败。
    #[error("cloud clipboard relay keystore is unavailable")]
    Storage,
    /// 驱动或中继给出的结果，例如本机还在等待批准（`clipboard_device_pending`）。
    #[error("cloud clipboard relay operation failed: {}", .0.code)]
    Clipboard(DriverError),
}

impl RelayError {
    /// 交给共享面板的错误码。驱动和中继给出的已知码原样保留，其余按状态码归类。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "relay_not_configured",
            Self::Unauthorized => "relay_unauthorized",
            Self::Invalid => "invalid_cloud_clipboard",
            Self::Unavailable => "relay_unavailable",
            Self::Storage => "relay_storage",
            Self::Clipboard(error) => crate::account::cloud_clipboard_error_code(&error.code)
                .unwrap_or(match error.status {
                    400 => "invalid_cloud_clipboard",
                    404 => "not_found",
                    _ => "relay_unavailable",
                }),
        }
    }

    fn from_account(error: AccountError) -> Self {
        match error {
            AccountError::Invalid => Self::Invalid,
            _ => Self::Unavailable,
        }
    }
}

/// 一个校验过的中继配置：规范化的基地址（去掉末尾的 `/`）和令牌。
#[derive(Clone, PartialEq, Eq)]
pub struct RelayConfig {
    base: Url,
    url: String,
    token: String,
}

impl std::fmt::Debug for RelayConfig {
    // 令牌是凭据，不进任何日志。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RelayConfig")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl RelayConfig {
    pub fn new(url: &str, token: &str) -> Result<Self, RelayError> {
        let base = parse_relay_url(url).ok_or(RelayError::NotConfigured)?;
        if !valid_relay_token(token) {
            return Err(RelayError::NotConfigured);
        }
        let url = format!(
            "{}{}",
            base.origin().ascii_serialization(),
            base.path().trim_end_matches('/')
        );
        Ok(Self {
            base,
            url,
            token: token.to_owned(),
        })
    }

    /// 偏好里的中继；没有配置时为 `None`。
    pub fn from_preferences(
        preferences: &CloudClipboardRelayPreferences,
    ) -> Result<Option<Self>, RelayError> {
        if !preferences.is_configured() {
            return Ok(None);
        }
        Self::new(&preferences.url, &preferences.token).map(Some)
    }

    /// 规范化后的基地址，例如 `https://relay.example.com/msime`。
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 令牌对应的剪贴板空间编号：SHA-256(令牌) 的前 16 字节，小写十六进制。与中继自己算的空间编号无关，只用来认出本机的密钥串属于哪个空间。
    fn space(&self) -> String {
        hex::encode(&Sha256::digest(self.token.as_bytes())[..16])
    }

    /// 基地址拼上驱动给的路径。只接受云剪贴板接口下由驱动生成的那几种字符，拼完再确认协议、主机、端口和路径前缀都没变。
    fn request_url(&self, path: &str) -> Result<Url, RelayError> {
        let in_scope = path == CLIPBOARD_PATH
            || path
                .strip_prefix(CLIPBOARD_PATH)
                .is_some_and(|rest| rest.starts_with('/'));
        let plain = path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/_-?=".contains(&byte));
        if !in_scope || !plain || path.contains("//") {
            return Err(RelayError::Invalid);
        }
        let url = Url::parse(&format!("{}{path}", self.url)).map_err(|_| RelayError::Invalid)?;
        let prefix = format!("{}{CLIPBOARD_PATH}", self.base.path().trim_end_matches('/'));
        if url.scheme() != self.base.scheme()
            || url.host_str() != self.base.host_str()
            || url.port_or_known_default() != self.base.port_or_known_default()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || !url.path().starts_with(&prefix)
        {
            return Err(RelayError::Invalid);
        }
        Ok(url)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredKeystore {
    relay: String,
    space: String,
    keystore: String,
}

/// 向一个中继发单个请求：只发往配置的基地址下云剪贴板的那组接口，带上令牌，不跟随重定向，响应有上限。[`RelayClipboard`] 的每个请求都经过它；自己跑端到端加密驱动、自己保存密钥串的宿主（Android 用 AndroidKeyStore 加密密钥串，见 `docs/cloud-clipboard-relay.md`）只借它发请求，不落盘。
pub struct RelayTransport {
    config: RelayConfig,
    client: Client,
}

impl RelayTransport {
    pub fn new(config: RelayConfig) -> Result<Self, RelayError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!("MSIME/{}", std::env::consts::OS))
            .build()
            .map_err(|_| RelayError::Unavailable)?;
        Ok(Self { config, client })
    }

    /// 发一个请求，状态码和正文原样交回，由端到端加密驱动或调用方解读；401 直接是 [`RelayError::Unauthorized`]，不交给驱动。`path` 必须在云剪贴板接口之下（`/v1/users/me/clipboard…`），否则是 [`RelayError::Invalid`]，不发出去。会联网。
    pub fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<HttpResponse, RelayError> {
        let method = match method {
            "GET" => Method::GET,
            "PUT" => Method::PUT,
            "POST" => Method::POST,
            "DELETE" => Method::DELETE,
            _ => return Err(RelayError::Invalid),
        };
        if body.is_some_and(|body| body.len() > MAX_REQUEST_BYTES) {
            return Err(RelayError::Invalid);
        }
        let url = self.config.request_url(path)?;
        let mut request = self
            .client
            .request(method, url)
            .header(reqwest::header::ACCEPT, "application/json")
            .bearer_auth(&self.config.token);
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_owned());
        }
        let response = request.send().map_err(|_| RelayError::Unavailable)?;
        let status = response.status().as_u16();
        if status == 401 {
            return Err(RelayError::Unauthorized);
        }
        if !response.status().is_success() {
            // 错误正文只用来读 `error.code`，读不出时驱动按状态码处理。
            let body = crate::bounded_io::read_bounded(response, MAX_ERROR_BODY_BYTES)
                .ok()
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .unwrap_or_default();
            return Ok(HttpResponse { status, body });
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CLIPBOARD_RESPONSE_BYTES as u64)
        {
            return Err(RelayError::Unavailable);
        }
        let bytes = crate::bounded_io::read_bounded(response, MAX_CLIPBOARD_RESPONSE_BYTES as u64)
            .map_err(|_| RelayError::Unavailable)?;
        let body = String::from_utf8(bytes).map_err(|_| RelayError::Unavailable)?;
        Ok(HttpResponse { status, body })
    }
}

/// 经一个自建中继读写云剪贴板。每个方法都会联网，不要在界面线程或输入线程上调用。
pub struct RelayClipboard {
    transport: RelayTransport,
    directory: PathBuf,
    device_name: String,
}

impl RelayClipboard {
    /// `directory` 是放本机密钥串的绝对路径目录，同一台设备上的各个进程要传同一个；`device_name` 是登记设备时显示给其他设备的名称。
    pub fn new(
        config: RelayConfig,
        directory: impl Into<PathBuf>,
        device_name: &str,
    ) -> Result<Self, RelayError> {
        let directory = directory.into();
        if !directory.is_absolute() {
            return Err(RelayError::Invalid);
        }
        Ok(Self {
            transport: RelayTransport::new(config)?,
            directory,
            device_name: device_name.to_owned(),
        })
    }

    /// 解密后的条目，`search` 在本机解密后匹配。本机还在等待批准时 `e2e.status` 为 `pending`、条目为空。
    pub fn list(&self, search: &str) -> Result<AccountClipboardPage, RelayError> {
        crate::account::validate_clipboard_search(search).map_err(RelayError::from_account)?;
        let page: AccountClipboardPage = decode(self.drive(Operation::List {
            search: search.to_owned(),
        })?)?;
        crate::account::validate_clipboard_page(&page).map_err(|_| RelayError::Unavailable)?;
        Ok(page)
    }

    /// 在本机加密后上传一条。本机还没有获准时失败，错误码为 `clipboard_device_pending`。
    pub fn add(&self, text: &str) -> Result<AccountClipboardItem, RelayError> {
        crate::account::validate_clipboard_text(text).map_err(RelayError::from_account)?;
        let item: AccountClipboardItem = decode(self.drive(Operation::Add {
            text: text.to_owned(),
        })?)?;
        crate::account::validate_clipboard_item(&item).map_err(|_| RelayError::Unavailable)?;
        Ok(item)
    }

    /// 本机状态、核对码和这个空间里登记的全部设备。
    pub fn devices(&self) -> Result<AccountClipboardDevices, RelayError> {
        self.device_operation(Operation::Status)
    }

    pub fn approve(&self, device_id: &str) -> Result<AccountClipboardDevices, RelayError> {
        crate::account::validate_device_id(device_id).map_err(RelayError::from_account)?;
        self.device_operation(Operation::Approve {
            device_id: device_id.to_owned(),
        })
    }

    pub fn reject(&self, device_id: &str) -> Result<AccountClipboardDevices, RelayError> {
        crate::account::validate_device_id(device_id).map_err(RelayError::from_account)?;
        self.device_operation(Operation::Reject {
            device_id: device_id.to_owned(),
        })
    }

    /// 移除一台设备并换钥：条目清空，其他设备都要在本机上重新批准。
    pub fn remove(&self, device_id: &str) -> Result<AccountClipboardDevices, RelayError> {
        crate::account::validate_device_id(device_id).map_err(RelayError::from_account)?;
        self.device_operation(Operation::Remove {
            device_id: device_id.to_owned(),
        })
    }

    /// 换一把只给本机的新密钥，条目清空，其他设备都要重新批准。
    pub fn reset(&self) -> Result<AccountClipboardDevices, RelayError> {
        self.device_operation(Operation::Reset)
    }

    /// 打开或关闭这个空间的云剪贴板。关闭时中继删除条目、密钥、封装和全部设备登记。
    pub fn set_enabled(&self, enabled: bool) -> Result<(), RelayError> {
        self.direct(
            "PUT",
            &format!("{CLIPBOARD_PATH}/settings"),
            Some(json!({ "enabled": enabled })),
        )
    }

    pub fn delete(&self, id: &str) -> Result<(), RelayError> {
        crate::account::validate_clipboard_id(id).map_err(RelayError::from_account)?;
        self.direct("DELETE", &format!("{CLIPBOARD_PATH}/{id}"), None)
    }

    /// 清空全部条目，开关、密钥和设备登记不变。
    pub fn clear(&self) -> Result<(), RelayError> {
        self.direct("DELETE", CLIPBOARD_PATH, None)
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), RelayError> {
        crate::account::validate_clipboard_id(id).map_err(RelayError::from_account)?;
        self.direct(
            "PUT",
            &format!("{CLIPBOARD_PATH}/{id}/pin"),
            Some(json!({ "pinned": pinned })),
        )
    }

    /// 保留天数，取 [`RETENTION_DAYS`] 中的一个；0 表示一直保留。
    pub fn set_retention(&self, days: u32) -> Result<(), RelayError> {
        if !RETENTION_DAYS.contains(&days) {
            return Err(RelayError::Invalid);
        }
        self.direct(
            "PUT",
            &format!("{CLIPBOARD_PATH}/retention"),
            Some(json!({ "days": days })),
        )
    }

    fn device_operation(
        &self,
        operation: Operation,
    ) -> Result<AccountClipboardDevices, RelayError> {
        let devices: AccountClipboardDevices = decode(self.drive(operation)?)?;
        crate::account::validate_devices(&devices).map_err(|_| RelayError::Unavailable)?;
        Ok(devices)
    }

    /// 跑完一个驱动操作。整个操作持有本机的中继锁；驱动交回的密钥串在发出下一个请求之前保存。
    fn drive(&self, operation: Operation) -> Result<Value, RelayError> {
        let _lock = self.lock()?;
        let keystore = self.load_keystore()?;
        let result = clipboard_e2e::run(
            operation,
            keystore.as_deref(),
            &self.device_name,
            |request| {
                self.transport
                    .send(&request.method, &request.path, request.body.as_deref())
            },
            |value| self.save_keystore(value),
            || RelayError::Unavailable,
        )?;
        result.map_err(RelayError::Clipboard)
    }

    fn direct(&self, method: &str, path: &str, body: Option<Value>) -> Result<(), RelayError> {
        let body = body.map(|body| body.to_string());
        let response = self.transport.send(method, path, body.as_deref())?;
        if (200..300).contains(&response.status) {
            Ok(())
        } else {
            Err(RelayError::Clipboard(DriverError::from_response(&response)))
        }
    }

    fn storage(&self) -> FileAccountSessionStorage {
        FileAccountSessionStorage::new(&self.directory, AccountSessionFileLayout::Native)
    }

    fn lock(&self) -> Result<crate::file_lock::ScopedFileLock, RelayError> {
        self.storage()
            .create_directory()
            .map_err(|_| RelayError::Storage)?;
        let file = crate::file_lock::open_private_lock_file(self.directory.join(RELAY_LOCK_FILE))
            .map_err(|_| RelayError::Storage)?;
        let deadline = Instant::now() + LOCK_TIMEOUT;
        loop {
            if crate::file_lock::try_exclusive(&file).map_err(|_| RelayError::Storage)? {
                return Ok(crate::file_lock::ScopedFileLock::from_locked(file));
            }
            if Instant::now() >= deadline {
                return Err(RelayError::Unavailable);
            }
            std::thread::sleep(LOCK_POLL);
        }
    }

    /// 本机给这个中继、这个令牌空间保存的密钥串。文件属于别的中继或别的空间、或者解析不了时当作没有。
    fn load_keystore(&self) -> Result<Option<String>, RelayError> {
        let Some(bytes) = self
            .storage()
            .read_private(RELAY_KEYSTORE_FILE, MAX_KEYSTORE_FILE_BYTES)
            .map_err(|_| RelayError::Storage)?
        else {
            return Ok(None);
        };
        let Ok(stored) = serde_json::from_slice::<StoredKeystore>(&bytes) else {
            return Ok(None);
        };
        Ok((stored.relay == self.transport.config.url
            && stored.space == self.transport.config.space())
        .then_some(stored.keystore))
    }

    fn save_keystore(&self, keystore: &str) -> Result<(), RelayError> {
        let bytes = serde_json::to_vec(&StoredKeystore {
            relay: self.transport.config.url.clone(),
            space: self.transport.config.space(),
            keystore: keystore.to_owned(),
        })
        .map_err(|_| RelayError::Storage)?;
        self.storage()
            .write_private(RELAY_KEYSTORE_FILE, &bytes)
            .map_err(|_| RelayError::Storage)
    }
}

fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RelayError> {
    serde_json::from_value(value).map_err(|_| RelayError::Unavailable)
}

#[cfg(test)]
#[path = "clipboard_relay_tests.rs"]
mod tests;
