//! Account protocol and session state independent of UI and platform hosts.

use crate::cloud::dictionary::DictionaryKind;
use reqwest::blocking::{Client, Response};
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const ACCOUNT_ORIGIN: &str = "https://api.msime.app";
const MAX_JSON_BYTES: usize = 1024 * 1024;
const MAX_ACCOUNT_PREFERENCE_FIELDS: usize = 512;
const MAX_ACCOUNT_PREFERENCE_KEY_BYTES: usize = 128;
// A custom keyboard skin can carry roughly 512 KiB of image bytes as base64. The negotiated
// document limit is the real bound, so one string may occupy almost the full 1 MiB envelope.
const MAX_ACCOUNT_PREFERENCE_STRING_BYTES: usize = MAX_JSON_BYTES;
const MAX_DICTIONARY_PAGE_ENTRIES: usize = 100;
const MAX_DICTIONARY_EXPORT_BYTES: usize = 384 * 1024 * 1024;
const MAX_DICTIONARY_SNAPSHOT_BYTES: usize = 512 * 1024 * 1024;
const MAX_CHAT_MODELS: usize = 33;
const MAX_CHAT_MODEL_ID_BYTES: usize = 200;
const MAX_CHAT_MESSAGES: usize = 16;
const MAX_CHAT_MESSAGE_BYTES: usize = 16 * 1024;
const MAX_CHAT_REQUEST_BYTES: usize = 64 * 1024;
const MAX_CHAT_RESPONSE_BYTES: usize = 16 * 1024;
const CHAT_PATH: &str = "/v1/chat/completions";
/// 对话请求从连接到读完回答的总时限。服务端整条请求（流式也一样）受 `timeout_seconds` 约束，最多 120 秒，多给 5 秒让服务端自己的超时先到。
const CHAT_TIMEOUT: Duration = Duration::from_secs(125);
const REFRESH_EARLY_SECONDS: u64 = 30;
/// Backend bearer sessions are short-lived; reject malformed responses that would create a
/// practically permanent local session.
const MAX_SESSION_SECONDS: u64 = 86_400 * 30;

pub(crate) fn valid_model_catalog<'a>(
    models: impl IntoIterator<Item = &'a str>,
    default_model: &str,
    maximum_models: usize,
    maximum_id_bytes: usize,
) -> bool {
    if maximum_models == 0
        || default_model.is_empty()
        || !crate::text::is_bounded_text(default_model, maximum_id_bytes)
    {
        return false;
    }
    let mut ids = HashSet::new();
    let mut default_present = false;
    for id in models {
        if id.is_empty() || !crate::text::is_bounded_text(id, maximum_id_bytes) {
            return false;
        }
        if ids.len() >= maximum_models || !ids.insert(id) {
            return false;
        }
        if id == default_model {
            default_present = true;
        }
        if ids.len() == 1 {
            ids.reserve(maximum_models.saturating_sub(ids.len()));
        }
    }
    default_present && !ids.is_empty()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountIdentity {
    pub user_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountUser {
    pub id: String,
    pub display_name: String,
    pub created_at: String,
    /// The verified email of the linked Google identity. Absent for other sign-ins, from older backends, and in sessions saved before it existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// The uploaded avatar's public URL, else the Google picture; absent when the user has neither. Fetched only through [`fetch_account_avatar`], which accepts the two hosts it can name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountChallenge {
    pub challenge_id: String,
    pub expires_in: u64,
    pub nonce: Option<String>,
    pub authorization_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountProfileIdentity {
    pub provider: String,
    pub subject: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountProfile {
    pub user: AccountUser,
    pub identities: Vec<AccountProfileIdentity>,
}

/// 云剪贴板里解密后的一条，形状与 `docs/cloud-clipboard-e2e.md` 里驱动 `add` 的结果相同。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardItem {
    pub id: String,
    pub text: String,
    pub updated_at: String,
    #[serde(default)]
    pub pinned: bool,
    /// 写入这条的设备名，由驱动清理过控制字符和长度。
    #[serde(default)]
    pub device: String,
}

/// 本机能不能读写云剪贴板。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountClipboardE2eStatus {
    /// 本机持有当前的剪贴板密钥。
    Ready,
    /// 本机已登记，等待一台已批准的设备点「允许」。
    Pending,
    /// 账号关闭了云剪贴板。
    Disabled,
}

/// 驱动 `list` 结果里的 `e2e` 对象。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardE2e {
    pub status: AccountClipboardE2eStatus,
    /// 本机的核对码，16 位数字每 4 位一组。
    pub code: String,
    /// 等待本机批准的设备数。
    pub pending_devices: usize,
    /// 解不开而被略去的条目数。
    pub undecryptable: usize,
}

/// 驱动 `list` 的结果：解密并在本机搜索过的条目。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardPage {
    pub enabled: bool,
    #[serde(default)]
    pub retention_days: i64,
    pub items: Vec<AccountClipboardItem>,
    pub e2e: AccountClipboardE2e,
}

/// 账号下登记的一台云剪贴板设备。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardDevice {
    pub device_id: String,
    pub name: String,
    pub created_at: String,
    pub approved: bool,
    /// 是否就是本机。
    pub current: bool,
    pub code: String,
}

/// 驱动 `status`、`approve`、`reject`、`remove`、`reset` 的结果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardDevices {
    pub enabled: bool,
    pub status: AccountClipboardE2eStatus,
    pub device_id: String,
    pub code: String,
    pub devices: Vec<AccountClipboardDevice>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryEntry {
    pub id: String,
    pub kind: DictionaryKind,
    pub code: String,
    pub word: String,
    pub weight: i64,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryPage {
    pub entries: Vec<AccountDictionaryEntry>,
    pub has_more: bool,
    pub offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryCatalogEntry {
    pub kind: DictionaryKind,
    pub code: String,
    pub word: String,
    pub weight: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryCatalogPage {
    pub entries: Vec<AccountDictionaryCatalogEntry>,
    pub has_more: bool,
    pub offset: usize,
    pub revision: i64,
    pub normalized: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountCandidateQuery {
    pub text: String,
    pub kind: String,
    pub scheme: String,
    pub profile: String,
    pub limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountChatModel {
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountChatModels {
    pub data: Vec<AccountChatModel>,
    pub default_model: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountPersonalCandidate {
    pub code: String,
    pub word: String,
    pub weight: i64,
    pub canonical_pinyin: Option<String>,
}

impl AccountPersonalCandidate {
    pub fn mutation_code(&self) -> &str {
        self.canonical_pinyin
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or(&self.code)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountPersonalCandidates {
    pub candidates: Vec<AccountPersonalCandidate>,
    pub context: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRankingSelection {
    pub count: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountRankingResult {
    pub revision: i64,
    pub changed: bool,
    pub selection: AccountRankingSelection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountFixedPosition {
    pub context: String,
    pub code: String,
    pub word: String,
    pub position: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountFixedPositions {
    pub positions: Vec<AccountFixedPosition>,
    pub offset: usize,
    pub has_more: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryRevision {
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryChange {
    pub revision: i64,
    pub previous: Option<AccountDictionaryEntry>,
    pub replacement: Option<AccountDictionaryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryChangePage {
    pub changes: Vec<AccountDictionaryChange>,
    pub next: i64,
    pub has_more: bool,
}

/// 云同步探测云端词库的结果（`GET /v1/users/me/dictionary/changes?after=&limit=1`）：`changed` 表示自 `after` 以来有改动，`revision` 是这一页的 `next`（没有改动时等于 `after`），`reset` 表示第一条改动是 `reset`。服务端的整份快照上传和「删除云端数据」都写这样一条，后者还会清掉此前的全部改动。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryProbe {
    pub changed: bool,
    pub revision: i64,
    pub reset: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionaryImportResult {
    pub imported: usize,
    pub revision: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountDictionaryExport {
    pub text: String,
    pub filename: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountDictionarySnapshotRestore {
    pub revision: i64,
    pub reset: bool,
}

/// 定义在 `preferences`（`edition` 也要用它，放在这里会让 `edition` 依赖 `account`），从这里导回，原来的路径照常可用。
pub use crate::preferences::AccountPreferenceValue;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccountPreferences {
    pub revision: i64,
    pub settings: BTreeMap<String, AccountPreferenceValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountPreferenceField {
    #[serde(rename = "type")]
    pub value_type: String,
}

/// 服务端的偏好字段表。序列化用服务端的 snake_case；反序列化另外接受 `msime_client_account` 的 `preferences_schema` 交给宿主的 camelCase（[`page::PreferenceSchemaResponse`]），宿主把它原样传回 `msime_client_account_harmony_settings_export`、`msime_client_account_ios_settings_export`、`msime_client_account_settings_export` 等入口时不用自己改键名（#8201 的 P5）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountPreferenceSchema {
    pub fields: BTreeMap<String, AccountPreferenceField>,
    #[serde(alias = "maximumBytes")]
    pub maximum_bytes: usize,
    #[serde(alias = "updateMode")]
    pub update_mode: String,
    #[serde(alias = "revisionRequired")]
    pub revision_required: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AccountTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: AccountUser,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedAccountSession {
    pub tokens: AccountTokens,
    pub expires_at_unix_ms: u64,
    /// 刷新时保持不变；每次登录都会取得新 ID，即使用户相同。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<uuid::Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AccountError {
    #[error("invalid account request")]
    Invalid,
    #[error("account authorization is required")]
    Unauthorized,
    #[error("account operation is forbidden")]
    Forbidden,
    #[error("account data changed")]
    Conflict,
    #[error("account resource was not found")]
    NotFound,
    #[error("account request was rate limited")]
    RateLimited,
    #[error("account service is unavailable")]
    Unavailable,
    #[error("secure account storage is unavailable")]
    Storage,
    #[error("account operation was cancelled")]
    Cancelled,
    /// The server's content screening refused the upload (`422 blocked_content`). The text has to change; the service itself is up.
    #[error("content contains words that may not be published")]
    BlockedContent,
    /// The server could not screen the upload right now (`503 screening_unavailable`); the same request can be sent again later.
    #[error("content screening is temporarily unavailable")]
    ScreeningUnavailable,
    /// The account is banned (`403 account_banned`).
    #[error("account is banned")]
    Banned,
}

impl AccountError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "account_invalid",
            Self::Unauthorized | Self::Forbidden => "account_unauthorized",
            Self::RateLimited => "account_rate_limited",
            Self::Storage => "account_storage",
            Self::Cancelled => "account_cancelled",
            Self::Conflict => "account_conflict",
            Self::NotFound | Self::Unavailable => "account_unavailable",
            Self::BlockedContent => "account_blocked_content",
            Self::ScreeningUnavailable => "account_screening_unavailable",
            Self::Banned => "account_banned",
        }
    }

    /// 社区页面认的错误码：与 [`Self::code`] 不同，`forbidden` 和 `not_found` 各有自己的码，因为社区页面要分别说「不是你的作品」和「作品已不存在」。共享应用（Tauri）的社区命令和 `msime_client_community` 都用它。
    pub fn community_code(&self) -> &'static str {
        match self {
            Self::Invalid => "community_invalid",
            Self::Unauthorized => "community_unauthorized",
            Self::Forbidden => "community_forbidden",
            Self::NotFound => "community_not_found",
            Self::RateLimited => "community_rate_limited",
            Self::Cancelled => "community_cancelled",
            Self::Storage => "community_storage",
            Self::Conflict => "community_conflict",
            Self::Unavailable => "community_unavailable",
            Self::BlockedContent => "community_blocked_content",
            Self::ScreeningUnavailable => "community_screening_unavailable",
            Self::Banned => "community_account_banned",
        }
    }

    /// The Chinese sentence a host shows for a refusal the user can act on (`blocked_content`, `screening_unavailable`, `account_banned`), or `None` for errors the host words itself.
    pub fn moderation_message(&self) -> Option<&'static str> {
        match self {
            Self::BlockedContent => Some("内容包含不允许发布的词语，请修改后再提交"),
            Self::ScreeningUnavailable => Some("审核服务暂时不可用，请稍后重试"),
            Self::Banned => Some("该账号已被封禁，暂时无法使用账号相关功能"),
            _ => None,
        }
    }

    /// The error for a failed response a host received itself: `status` is the HTTP status and `body` the response body (the server's `{"error":{"code":...}}`), mapped exactly as this crate's own requests map theirs.
    pub fn from_http_response(status: u16, body: &[u8]) -> Self {
        let status = StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE);
        Self::from_response(status, body)
    }

    /// Maps a failed response by the server's error code (`{"error":{"code":...}}`) where it names a condition the caller must tell apart, and by the status otherwise.
    fn from_response(status: StatusCode, body: &[u8]) -> Self {
        #[derive(Deserialize)]
        struct Envelope {
            error: Detail,
        }
        #[derive(Deserialize)]
        struct Detail {
            code: String,
        }
        let code = serde_json::from_slice::<Envelope>(body)
            .map(|envelope| envelope.error.code)
            .unwrap_or_default();
        match code.as_str() {
            "blocked_content" => Self::BlockedContent,
            "screening_unavailable" => Self::ScreeningUnavailable,
            "account_banned" => Self::Banned,
            _ => Self::from_status(status),
        }
    }

    fn from_status(status: StatusCode) -> Self {
        match status.as_u16() {
            400 => Self::Invalid,
            401 => Self::Unauthorized,
            403 => Self::Forbidden,
            404 => Self::NotFound,
            409 => Self::Conflict,
            429 => Self::RateLimited,
            503 => Self::Unavailable,
            _ => Self::Unavailable,
        }
    }
}

/// 新接口（释义、常用语、我的设备、云端数据、反馈、诊断上传、对话、AI 皮肤、注销账号的 `account_delete`）的失败：在 [`AccountError`] 之外带着 HTTP 状态码、服务端的错误码、`Retry-After` 的秒数和有没有拿到响应，宿主据此区分「需要最近登录」「请求太大」「不存在」「被限流、多久后重试」「断网」。`AccountError` 的变体不变，已有接口返回的码也不变。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountFailure {
    pub error: AccountError,
    /// 服务端回的 HTTP 状态码；没拿到响应或本地就拒绝时为 `None`。
    pub status: Option<u16>,
    /// 服务端 `{"error":{"code":...}}` 里的码；本地预检用 [`Self::too_large`] 和 [`Self::identity_changed`] 的两个本地码。
    pub server_code: Option<String>,
    /// `Retry-After` 的秒数，只认整数秒；HTTP-date 形式忽略。
    pub retry_after_seconds: Option<u64>,
    /// 请求发出去以后没有拿到响应：连不上、DNS 或 TLS 失败、超时、连接在响应头到达之前断开。这时 `error` 仍是 [`AccountError::Unavailable`]，经 `error` 取码的已有接口看到的还是 `account_unavailable`；[`Self::code`] 是 `account_network`，宿主据此说「网络连接失败」，与服务端 5xx 的「服务暂不可用」分开。
    pub network_failure: bool,
}

/// 本地预检发现请求体超过服务端整包上限时的码。服务端走 `decode` 的接口超限时回的是 400 `invalid_json`，所以本地先拦下。
const LOCAL_TOO_LARGE: &str = "request_too_large";
/// 绑定账号的操作发现当前会话已不是那个账号时的码。
const LOCAL_IDENTITY_CHANGED: &str = "identity_changed";
/// 宿主给的请求 id 已经有一个在进行时的码。
const LOCAL_BUSY: &str = "request_busy";

impl From<AccountError> for AccountFailure {
    fn from(error: AccountError) -> Self {
        Self {
            error,
            status: None,
            server_code: None,
            retry_after_seconds: None,
            network_failure: false,
        }
    }
}

impl AccountFailure {
    /// 请求没有拿到响应（见 [`Self::network_failure`]），`code()` 是 `account_network`。
    pub fn network() -> Self {
        Self {
            network_failure: true,
            ..AccountError::Unavailable.into()
        }
    }

    /// `send()` 失败时的码。只有真的没拿到响应（连不上、DNS 或 TLS 失败、超时、连接在响应头到达之前断开）才是 [`Self::network`]：reqwest 在发出之前就建不出请求（`is_builder`，例如地址或头不合法）是 `account_invalid`，请求根本没有发出去；重定向被策略拒绝（`is_redirect`，客户端不跟随重定向，正常碰不到）时服务端已经回了响应，是 `account_unavailable`。
    pub(crate) fn from_send_error(error: &reqwest::Error) -> Self {
        if error.is_builder() {
            AccountError::Invalid.into()
        } else if error.is_redirect() {
            AccountError::Unavailable.into()
        } else {
            Self::network()
        }
    }

    /// 请求体超过服务端的上限，在本地就拒绝，`code()` 是 `account_too_large`。
    pub fn too_large() -> Self {
        Self {
            server_code: Some(LOCAL_TOO_LARGE.to_owned()),
            ..AccountError::Invalid.into()
        }
    }

    /// 操作绑定的账号已不是当前登录或匿名会话的账号，`code()` 是 `account_identity_changed`；不会退回另一个账号去做。
    pub fn identity_changed() -> Self {
        Self {
            server_code: Some(LOCAL_IDENTITY_CHANGED.to_owned()),
            ..AccountError::Cancelled.into()
        }
    }

    /// 同一个请求 id 已经有一个在进行（[`ChatStreamRequests::begin`]），`code()` 是 `account_busy`。
    pub fn busy() -> Self {
        Self {
            server_code: Some(LOCAL_BUSY.to_owned()),
            ..AccountError::Conflict.into()
        }
    }

    /// 宿主自己收到的失败响应：`status` 是 HTTP 状态码，`body` 是响应正文，`retry_after` 是 `Retry-After` 头的原文。映射与本 crate 自己的请求相同。
    pub fn from_http_response(status: u16, body: &[u8], retry_after: Option<&str>) -> Self {
        let parsed = StatusCode::from_u16(status).unwrap_or(StatusCode::SERVICE_UNAVAILABLE);
        Self {
            error: AccountError::from_response(parsed, body),
            status: Some(status),
            server_code: server_error_code(body),
            retry_after_seconds: retry_after.and_then(parse_retry_after),
            network_failure: false,
        }
    }

    /// 交给宿主的错误码。`account_too_large` 的 413 规则只对反馈和快照接口有效：走 64 KiB 整包上限的接口超限时服务端回 400 `invalid_json`，那些接口靠本地预检报这个码。
    pub fn code(&self) -> &'static str {
        if self.network_failure {
            return "account_network";
        }
        let server = self.server_code.as_deref();
        if server == Some("recent_login_required") {
            return "account_recent_login_required";
        }
        if server == Some(LOCAL_IDENTITY_CHANGED) {
            return "account_identity_changed";
        }
        if server == Some(LOCAL_BUSY) {
            return "account_busy";
        }
        if server == Some("no_verified_email") {
            return "account_no_verified_email";
        }
        if server == Some("qq_link_code_not_found") {
            return "account_qq_link_code_not_found";
        }
        if server == Some("qq_link_limit") {
            return "account_qq_link_limit";
        }
        if self.status == Some(413)
            || matches!(
                server,
                Some(
                    "feedback_too_large"
                        | "screenshot_too_large"
                        | "snapshot_too_large"
                        | LOCAL_TOO_LARGE
                )
            )
        {
            return "account_too_large";
        }
        match self.status {
            Some(404) => "account_not_found",
            Some(429) => "account_rate_limited",
            Some(503) if server == Some("server_busy") => "account_rate_limited",
            _ => self.error.code(),
        }
    }

    /// 是不是服务端以 401 拒绝了令牌（或本地没有会话），调用方据此带着被拒的令牌刷新后重试一次。
    pub fn is_unauthorized(&self) -> bool {
        self.error == AccountError::Unauthorized
    }
}

/// `Retry-After` 的整数秒；HTTP-date 和其他写法都不认。
pub(crate) fn parse_retry_after(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

/// 在 UTF-8 字符边界上截到不超过 `maximum` 字节。
pub(crate) fn truncate_at_char_boundary(value: &mut String, maximum: usize) {
    if value.len() > maximum {
        let mut end = maximum;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
    }
}

/// 按 RFC 3339 读一个时间，读不出时为 `None`；超过 64 字节的不读。
pub(crate) fn parse_rfc3339(value: &str) -> Option<time::OffsetDateTime> {
    if value.len() > 64 {
        return None;
    }
    time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}

/// 服务端错误正文 `{"error":{"code":...}}` 里的码，读不出时为 `None`。
fn server_error_code(body: &[u8]) -> Option<String> {
    #[derive(Deserialize)]
    struct Envelope {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        code: String,
    }
    serde_json::from_slice::<Envelope>(body)
        .ok()
        .map(|envelope| envelope.error.code)
        .filter(|code| !code.is_empty() && code.len() <= 64)
}

// 共用的数据类型和上限留在这里，行为按用途拆到各子模块。每个部分都重新导出，所以 `msime_client_services::account::X` 仍能解析到这里还是单个文件时的全部内容。
mod anonymous;
mod api;
/// 安卓的 Apple 网页登录（PKCE 式一次性授权码）。
mod apple_web;
mod avatar;
/// 对话的流式请求（SSE）和取消。
mod chat_stream;
mod client;
mod clipboard;
/// 多分类自动云同步的纯决策、状态文件和单轮状态机。
pub mod cloud_sync;
/// 「我的设备」：列出和撤销登录会话。
mod devices;
/// 云词库导出边下载边写进文件。
mod dictionary_export;
/// 「其他平台下载」的「发送链接」。
mod download_link;
/// App 内反馈。
mod feedback;
mod file_storage;
mod google;
/// 账号设置文档与鸿蒙宿主之间的映射（`platform.harmony.*`），C 接口和自动云同步共用。
pub mod harmony_settings;
/// 宿主注入的会话存储（钥匙串、App Group、AndroidKeyStore），C 接口的 `msime_client_session_store_register` 经它接进来。
mod host_storage;
/// 账号设置文档与 iOS 键盘设置（App Group）之间的映射，Tauri iOS 和 C 接口共用。
pub mod ios_settings;
pub mod name;
/// 账号页读到的脱敏响应，Tauri 的账号命令和原生设置窗口的 C 接口共用。
pub mod page;
/// 无编码常用语的整列表同步。
mod phrases;
/// 「绑定 QQ 群助手」：用机器人给的绑定码绑定、列出和解绑。
mod qq_links;
mod session;
/// 本地偏好与账号设置同步文档之间的映射，Tauri 和安卓原生宿主共用。
pub mod settings_sync;
/// 账号释义（`/v1/translate`）。
mod translate;
/// 打字统计上云：上传最近几天的计数和清空云端副本。
mod typing_upload;
/// 云端数据的汇总、导出和删除。
mod user_data;
mod validate;
/// 语音贡献。
mod voice_contribution;

pub use anonymous::{
    delete_anonymous_voice_samples, ensure_anonymous_account, ensure_anonymous_account_in,
    ensure_anonymous_account_with, has_saved_anonymous_identity, read_published_anonymous_identity,
    shared_access_token, shared_access_token_in, with_shared_access_token, AnonymousIdentitySource,
    AnonymousSessionStorage, LockedAnonymousSessionStorage, PublishedAnonymousIdentity,
    SharedAccessToken, ANONYMOUS_ACCOUNT_FILE, ANONYMOUS_SESSION_FILE,
};
pub use api::*;
pub use apple_web::{AccountAppleWebChallenge, AppleWebSignIn};
pub use avatar::*;
pub use chat_stream::*;
pub use client::*;
pub use clipboard::*;
pub use devices::*;
pub use download_link::*;
pub use feedback::*;
pub use file_storage::*;
pub use google::*;
pub use host_storage::*;
pub use phrases::*;
pub use qq_links::*;
pub use session::*;
pub use translate::*;
pub use typing_upload::*;
pub use user_data::*;
pub use validate::*;
pub use voice_contribution::*;

#[cfg(test)]
mod client_agent_tests;
#[cfg(test)]
mod error_cases_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod preference_schema_tests;
#[cfg(test)]
mod sign_in_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod transport_tests;
