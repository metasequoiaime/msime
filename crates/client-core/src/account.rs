//! Account protocol and session state independent of UI and platform hosts.

use crate::cloud::dictionary::DictionaryKind;
use reqwest::blocking::{Client, Response};
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    let mut ids = Vec::with_capacity(maximum_models);
    ids.extend(models);
    !ids.is_empty()
        && ids.len() <= maximum_models
        && !default_model.is_empty()
        && crate::text::is_bounded_text(default_model, maximum_id_bytes)
        && ids.contains(&default_model)
        && ids
            .iter()
            .all(|id| !id.is_empty() && crate::text::is_bounded_text(id, maximum_id_bytes))
        && {
            let mut unique = std::collections::HashSet::with_capacity(ids.len());
            for id in &ids {
                unique.insert(id);
            }
            unique.len() == ids.len()
        }
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardItem {
    pub id: String,
    pub text: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountClipboardPage {
    pub enabled: bool,
    pub items: Vec<AccountClipboardItem>,
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

/// The deliberately small value set accepted by the account preferences API.
/// Credentials, arbitrary JSON objects, and input contents never cross this
/// boundary; platform hosts map their safe local settings to these scalars.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AccountPreferenceValue {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
}

impl AccountPreferenceValue {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Number(_) => "number",
            Self::String(_) => "string",
        }
    }
}

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountPreferenceSchema {
    pub fields: BTreeMap<String, AccountPreferenceField>,
    pub maximum_bytes: usize,
    pub update_mode: String,
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

// The shared data types and limits stay here; the behaviour is split by what it
// does. Each part is re-exported, so `client_core::account::X` still resolves to
// everything it did when this was one file.
mod anonymous;
mod api;
mod avatar;
mod client;
mod file_storage;
mod google;
mod session;
/// 本地偏好与账号设置同步文档之间的映射，Tauri 和安卓原生宿主共用。
pub mod settings_sync;
mod validate;

pub use anonymous::{
    ensure_anonymous_account, AnonymousSessionStorage, ANONYMOUS_ACCOUNT_FILE,
    ANONYMOUS_SESSION_FILE,
};
pub use api::*;
pub use avatar::*;
pub use client::*;
pub use file_storage::*;
pub use google::*;
pub use session::*;
pub use validate::*;

#[cfg(test)]
mod tests;
