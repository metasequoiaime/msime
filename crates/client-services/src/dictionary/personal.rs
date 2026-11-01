//! Cross-process queue for personal dictionary edits.
//!
//! The Engine remains the authority for entry validation and dictionary data. This
//! module only owns the small host/keyboard hand-off file, which is shared by the
//! Android settings process and the `:ime` process.

use crate::file_lock;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_STATE_BYTES: usize = 8 * 1024 * 1024;
const MAX_REQUESTS: usize = 160;
const MAX_ACTIVE_REQUESTS: usize = 128;
const MAX_PAGE_ENTRIES: usize = 100;
/// The code prefix a page may be filtered by, as the Engine list accepts it.
const MAX_QUERY_BYTES: usize = 256;
const MAX_HISTORY: usize = 32;
/// 用户词库来源戳的长度上限；它是宿主给的几段文件元数据，远小于此。
const MAX_SOURCE_BYTES: usize = 512;
/// How many "already in the dictionary" receipts the state holds until the host that asked for them reads them back. A full list drops its oldest receipt. Only additions from the dictionary collections produce one, at most `MAX_ACTIVE_REQUESTS` of them are in flight, and each collections flush acknowledges what it read, so the list stays far below this and the state file well inside `MAX_STATE_BYTES`.
const MAX_ALREADY_PRESENT: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PersonalWordKind {
    #[serde(rename = "pinyin")]
    Pinyin,
    #[serde(rename = "wubi")]
    Wubi,
    #[serde(rename = "quickPhrase")]
    QuickPhrase,
    #[serde(rename = "english")]
    English,
    #[serde(rename = "wubi98")]
    Wubi98,
    /// 粤语，编码是粤拼音节。只存在本机：云词库和词库集合都不收它。
    #[serde(rename = "cantonese")]
    Cantonese,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersonalWord {
    pub kind: PersonalWordKind,
    pub key: String,
    pub value: String,
    pub weight: i64,
}

impl PersonalWord {
    /// The identity deliberately includes a length prefix so arbitrary phrase
    /// text cannot collide with a key separator.
    pub fn identity(&self) -> String {
        format!(
            "{}:{}:{}{}",
            match self.kind {
                PersonalWordKind::Pinyin => "pinyin",
                PersonalWordKind::Wubi => "wubi",
                PersonalWordKind::Wubi98 => "wubi98",
                PersonalWordKind::QuickPhrase => "quickPhrase",
                PersonalWordKind::English => "english",
                PersonalWordKind::Cantonese => "cantonese",
            },
            self.key.len(),
            self.key,
            self.value
        )
    }

    /// This is transport validation only. The Engine still validates and
    /// normalizes entries before applying them.
    pub fn validate(&self) -> Result<(), &'static str> {
        let key_limit = match self.kind {
            PersonalWordKind::Pinyin => 512,
            PersonalWordKind::Wubi | PersonalWordKind::Wubi98 => 4,
            PersonalWordKind::QuickPhrase => 32,
            PersonalWordKind::English => 64,
            PersonalWordKind::Cantonese => 512,
        };
        let key_valid = match self.kind {
            PersonalWordKind::Pinyin => super::pinyin_code_is_well_formed(&self.key, true),
            PersonalWordKind::Wubi | PersonalWordKind::Wubi98 => {
                super::wubi_code_is_well_formed(&self.key)
            }
            PersonalWordKind::QuickPhrase => {
                super::quick_phrase_transport_code_is_well_formed(&self.key)
            }
            PersonalWordKind::English => super::english_code_is_well_formed(&self.key),
            PersonalWordKind::Cantonese => super::cantonese_code_is_well_formed(&self.key),
        };
        let allowed_controls: &[char] = if self.kind == PersonalWordKind::QuickPhrase {
            &['\n', '\t']
        } else {
            &[]
        };
        let value_has_invalid_control =
            crate::has_disallowed_control_with_allowed(&self.value, allowed_controls);
        if self.key.is_empty()
            || self.key.len() > key_limit
            || !key_valid
            || self.value.is_empty()
            || value_has_invalid_control
            || self.weight < 0
        {
            return Err("invalid personal dictionary entry");
        }
        Ok(())
    }

    /// Validation for an entry that is being written: `validate` plus the stricter rules new input and imports follow. A quick phrase code must be letters only, as in the reference; a stored row with a digit still passes `validate`, so it loads and can be deleted.
    pub fn validate_new(&self) -> Result<(), &'static str> {
        self.validate()?;
        if self.kind == PersonalWordKind::QuickPhrase
            && !super::quick_phrase_code_is_well_formed(&self.key)
        {
            return Err("invalid personal dictionary entry");
        }
        Ok(())
    }
}

/// 键盘通过 [`PersonalDictionaryStore::synchronize_reporting_present`] 报告应用一条请求后的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersonalWordApplied {
    /// Engine 词库已改变。
    Written,
    /// 要加入的词已在用户词库中，因此保持原样。替换词的身份会保存在 [`PersonalDictionaryState::already_present`] 中，直到宿主确认。
    AlreadyPresent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonalWordRequestStatus {
    Pending,
    Applied,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalWordRequest {
    pub id: String,
    pub previous: Option<PersonalWord>,
    pub replacement: Option<PersonalWord>,
    pub status: PersonalWordRequestStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

impl PersonalWordRequest {
    fn identities(&self) -> impl Iterator<Item = String> + '_ {
        [self.previous.as_ref(), self.replacement.as_ref()]
            .into_iter()
            .flatten()
            .map(PersonalWord::identity)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalDictionaryState {
    pub version: u32,
    pub requests: Vec<PersonalWordRequest>,
    pub entries: Vec<PersonalWord>,
    pub has_more: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_error: Option<String>,
    pub page_offset: usize,
    pub requested_page_offset: usize,
    /// The dictionary and code prefix the host asked the keyboard to page through. The keyboard answers from the user's whole store, so a filter chosen in the host no longer has to be applied to one unfiltered page, which left a later page's matches unreachable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_kind: Option<PersonalWordKind>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub requested_query: String,
    /// The filter `entries` answer, so a host can tell a page still waiting for the keyboard from a page of its own search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_kind: Option<PersonalWordKind>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub page_query: String,
    /// 键盘读出 `entries` 时用户词库的来源戳（宿主给的不透明字符串）。下次同步时戳没变、页参数也没变，就沿用这一页，不再查词库；为 `None` 时每次同步都重查。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_source: Option<String>,
    pub refresh_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_refresh_id: Option<String>,
    /// 键盘发现已在用户词库中而未写入的加入项身份，按最早在前排列。词库集合存储会读取它们，因此停用集合时不会删除集合加入前用户已有的词。已应用的请求本身只保留短历史，所以回执会留在这里直到确认。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub already_present: Vec<String>,
}

impl Default for PersonalDictionaryState {
    fn default() -> Self {
        Self {
            version: 1,
            requests: Vec::new(),
            entries: Vec::new(),
            has_more: false,
            snapshot_date: None,
            snapshot_error: None,
            page_offset: 0,
            requested_page_offset: 0,
            requested_kind: None,
            requested_query: String::new(),
            page_kind: None,
            page_query: String::new(),
            snapshot_source: None,
            refresh_id: Uuid::new_v4().to_string(),
            completed_refresh_id: None,
            already_present: Vec::new(),
        }
    }
}

impl PersonalDictionaryState {
    pub fn pending_count(&self) -> usize {
        self.requests
            .iter()
            .filter(|request| request.status == PersonalWordRequestStatus::Pending)
            .count()
    }
}

/// One page of the user's own words, optionally within one dictionary and under one code prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersonalPageRequest {
    pub offset: usize,
    pub kind: Option<PersonalWordKind>,
    pub query: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersonalWordPage {
    pub entries: Vec<PersonalWord>,
    pub has_more: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PersonalDictionaryError {
    #[error("personal dictionary shared directory is unavailable")]
    Unavailable,
    #[error("personal dictionary state is invalid; the existing file was preserved")]
    InvalidState,
    #[error("personal dictionary is busy")]
    Busy,
    #[error("too many personal dictionary requests are waiting")]
    TooManyRequests,
    #[error("a personal dictionary entry already has a waiting request")]
    Conflict,
    #[error("personal dictionary request is invalid")]
    InvalidRequest,
    #[error("personal dictionary batch was not committed")]
    BatchFailed,
    #[error("personal dictionary I/O failed")]
    Io(#[source] io::Error),
}

impl From<io::Error> for PersonalDictionaryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A lock-protected state file shared by host UI and keyboard processes.
pub struct PersonalDictionaryStore {
    directory: PathBuf,
}

struct PersonalDictionaryLock {
    directory: crate::file_lock::PrivateDirectory,
    _lock: File,
}

#[cfg(unix)]
impl Drop for PersonalDictionaryLock {
    fn drop(&mut self) {
        // 操作结束即释放互斥，不能让 fork 到 exec 之间继承的描述符延长持锁时间。
        let _ = file_lock::unlock(&self._lock);
    }
}

impl PersonalDictionaryStore {
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            directory: directory.as_ref().to_owned(),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn read(&self) -> Result<PersonalDictionaryState, PersonalDictionaryError> {
        if let Some(parent) = self.directory.parent() {
            crate::storage::reject_symlink(parent)?;
        }
        crate::storage::reject_symlink(&self.directory)?;
        let file = self.directory.join("sync.json");
        match fs::symlink_metadata(&file) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(PersonalDictionaryError::InvalidState)
            }
            Ok(metadata) if !metadata.file_type().is_file() => {
                return Err(PersonalDictionaryError::InvalidState)
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PersonalDictionaryState::default())
            }
            Err(error) => return Err(error.into()),
        }
        read_file(&file)
    }

    pub fn enqueue(
        &self,
        previous: Option<PersonalWord>,
        replacement: Option<PersonalWord>,
        id: String,
    ) -> Result<(), PersonalDictionaryError> {
        let request = PersonalWordRequest {
            id,
            previous,
            replacement,
            status: PersonalWordRequestStatus::Pending,
            error: None,
            created_at: None,
        };
        if request.previous.is_none() && request.replacement.is_none() {
            return Err(PersonalDictionaryError::InvalidRequest);
        }
        validate_request_id(&request.id)?;
        // The previous entry is a stored row and only has to be one the store can hold; the replacement is new input and follows the stricter rules.
        if let Some(word) = &request.previous {
            word.validate()
                .map_err(|_| PersonalDictionaryError::InvalidRequest)?;
        }
        if let Some(word) = &request.replacement {
            word.validate_new()
                .map_err(|_| PersonalDictionaryError::InvalidRequest)?;
        }
        self.update(|state| enqueue_request(state, request))
    }

    pub fn enqueue_import(
        &self,
        words: Vec<PersonalWord>,
        id_prefix: String,
    ) -> Result<(), PersonalDictionaryError> {
        if words.is_empty() || words.len() > MAX_ACTIVE_REQUESTS || id_prefix.is_empty() {
            return Err(PersonalDictionaryError::InvalidRequest);
        }
        validate_request_id(&id_prefix)?;
        let mut identities = std::collections::HashSet::with_capacity(words.len());
        for word in &words {
            word.validate_new()
                .map_err(|_| PersonalDictionaryError::InvalidRequest)?;
            if !identities.insert(word.identity()) {
                return Err(PersonalDictionaryError::InvalidRequest);
            }
        }
        self.update(|state| {
            let active = state
                .requests
                .iter()
                .filter(|request| request.status != PersonalWordRequestStatus::Applied)
                .count();
            if active + words.len() > MAX_ACTIVE_REQUESTS {
                return Err(PersonalDictionaryError::TooManyRequests);
            }
            if has_identity_conflict(
                &state.requests,
                &identities,
                &[
                    PersonalWordRequestStatus::Pending,
                    PersonalWordRequestStatus::Failed,
                ],
            ) {
                return Err(PersonalDictionaryError::Conflict);
            }
            prune_history(state);
            for (index, word) in words.iter().cloned().enumerate() {
                state.requests.push(PersonalWordRequest {
                    id: format!("{id_prefix}-{index}"),
                    previous: None,
                    replacement: Some(word),
                    status: PersonalWordRequestStatus::Pending,
                    error: None,
                    created_at: None,
                });
            }
            state.refresh_id = Uuid::new_v4().to_string();
            Ok(())
        })
    }

    pub fn retry(&self, id: &str) -> Result<(), PersonalDictionaryError> {
        self.update(|state| {
            let Some(index) = state.requests.iter().position(|request| {
                request.id == id && request.status == PersonalWordRequestStatus::Failed
            }) else {
                return Ok(());
            };
            let mut identities = std::collections::HashSet::with_capacity(2);
            identities.extend(state.requests[index].identities());
            if has_identity_conflict(
                &state.requests,
                &identities,
                &[PersonalWordRequestStatus::Pending],
            ) {
                return Err(PersonalDictionaryError::Conflict);
            }
            state.requests[index].status = PersonalWordRequestStatus::Pending;
            state.requests[index].error = None;
            state.refresh_id = Uuid::new_v4().to_string();
            Ok(())
        })
    }

    pub fn dismiss_failure(&self, id: &str) -> Result<(), PersonalDictionaryError> {
        self.update(|state| {
            state.requests.retain(|request| {
                !(request.id == id && request.status == PersonalWordRequestStatus::Failed)
            });
            Ok(())
        })
    }

    pub fn request_page(
        &self,
        offset: usize,
        kind: Option<PersonalWordKind>,
        query: &str,
    ) -> Result<(), PersonalDictionaryError> {
        let query = query.trim();
        if offset > 1_000_000 || query.len() > MAX_QUERY_BYTES {
            return Err(PersonalDictionaryError::InvalidRequest);
        }
        self.update(|state| {
            state.requested_page_offset = offset;
            state.requested_kind = kind;
            state.requested_query = query.to_owned();
            state.refresh_id = Uuid::new_v4().to_string();
            Ok(())
        })
    }

    /// Apply at most four pending requests and refresh the confirmed page while
    /// the queue lock is held. A failed request remains independently retryable.
    pub fn synchronize<Apply, Page>(
        &self,
        mut apply: Apply,
        page: Page,
    ) -> Result<(), PersonalDictionaryError>
    where
        Apply: FnMut(&PersonalWordRequest) -> Result<(), String>,
        Page: FnMut(&PersonalPageRequest) -> Result<PersonalWordPage, String>,
    {
        self.synchronize_reporting_present(
            |request| apply(request).map(|()| PersonalWordApplied::Written),
            page,
        )
    }

    /// [`Self::synchronize`] for a keyboard that can tell an addition of a word the dictionary already holds from one that wrote it. Such an addition is still marked applied, and its identity is recorded in [`PersonalDictionaryState::already_present`].
    pub fn synchronize_reporting_present<Apply, Page>(
        &self,
        mut apply: Apply,
        page: Page,
    ) -> Result<(), PersonalDictionaryError>
    where
        Apply: FnMut(&PersonalWordRequest) -> Result<PersonalWordApplied, String>,
        Page: FnMut(&PersonalPageRequest) -> Result<PersonalWordPage, String>,
    {
        self.synchronize_batch(
            4,
            |requests| Ok(requests.iter().map(|request| apply(request)).collect()),
            || None,
            page,
        )
    }

    /// 批量编辑最多处理活动队列的 128 项；回调必须先提交数据库，再返回逐项结果。
    /// 回调整体失败或返回数量不符时不修改队列，数据库已成功的重试由请求回执识别。
    pub fn synchronize_batch_reporting_present<Apply, Page>(
        &self,
        apply: Apply,
        page: Page,
    ) -> Result<(), PersonalDictionaryError>
    where
        Apply: FnMut(
            &[&PersonalWordRequest],
        ) -> Result<Vec<Result<PersonalWordApplied, String>>, String>,
        Page: FnMut(&PersonalPageRequest) -> Result<PersonalWordPage, String>,
    {
        self.synchronize_batch(MAX_ACTIVE_REQUESTS, apply, || None, page)
    }

    /// [`Self::synchronize_batch_reporting_present`]，另由 `source` 给出用户词库当前的来源戳（应用完本批之后才取）。没有请求要应用、宿主没要新页、页参数和戳都与已存的一页相同时，不查词库，状态没变也不重写 sync.json：键盘每次聚焦都会同步，队列空着时这一步原本每次都要扫一页日志再 fsync 一次。`source` 返回 `None` 时照常重查。
    pub fn synchronize_batch_reporting_present_with_source<Apply, Source, Page>(
        &self,
        apply: Apply,
        source: Source,
        page: Page,
    ) -> Result<(), PersonalDictionaryError>
    where
        Apply: FnMut(
            &[&PersonalWordRequest],
        ) -> Result<Vec<Result<PersonalWordApplied, String>>, String>,
        Source: FnOnce() -> Option<String>,
        Page: FnMut(&PersonalPageRequest) -> Result<PersonalWordPage, String>,
    {
        self.synchronize_batch(MAX_ACTIVE_REQUESTS, apply, source, page)
    }

    fn synchronize_batch<Apply, Source, Page>(
        &self,
        limit: usize,
        mut apply: Apply,
        source: Source,
        mut page: Page,
    ) -> Result<(), PersonalDictionaryError>
    where
        Apply: FnMut(
            &[&PersonalWordRequest],
        ) -> Result<Vec<Result<PersonalWordApplied, String>>, String>,
        Source: FnOnce() -> Option<String>,
        Page: FnMut(&PersonalPageRequest) -> Result<PersonalWordPage, String>,
    {
        self.update(|state| {
            // 宿主上次要的页（request_page 换了 refresh_id）还没确认过，这次必须重查。
            let refresh_requested =
                state.completed_refresh_id.as_deref() != Some(state.refresh_id.as_str());
            let pending: Vec<_> = state
                .requests
                .iter()
                .enumerate()
                .filter_map(|(index, request)| {
                    (request.status == PersonalWordRequestStatus::Pending).then_some(index)
                })
                .take(limit)
                .collect();
            let outcomes = if pending.is_empty() {
                Vec::new()
            } else {
                let requests: Vec<_> = pending
                    .iter()
                    .map(|index| &state.requests[*index])
                    .collect();
                apply(&requests).map_err(|_| PersonalDictionaryError::BatchFailed)?
            };
            if outcomes.len() != pending.len() {
                return Err(PersonalDictionaryError::InvalidState);
            }
            let applied_any = !pending.is_empty();
            for (index, outcome) in pending.into_iter().zip(outcomes) {
                match outcome {
                    Ok(applied) => {
                        state.requests[index].status = PersonalWordRequestStatus::Applied;
                        state.requests[index].error = None;
                        if applied == PersonalWordApplied::AlreadyPresent {
                            if let Some(word) = &state.requests[index].replacement {
                                let identity = word.identity();
                                if !state.already_present.contains(&identity) {
                                    if state.already_present.len() >= MAX_ALREADY_PRESENT {
                                        state.already_present.remove(0);
                                    }
                                    state.already_present.push(identity);
                                }
                            }
                        }
                    }
                    Err(error) => {
                        state.requests[index].status = PersonalWordRequestStatus::Failed;
                        state.requests[index].error = Some(error.chars().take(500).collect());
                    }
                }
            }
            if state.pending_count() == 0 {
                state.completed_refresh_id = Some(state.refresh_id.clone());
            }
            let request = PersonalPageRequest {
                offset: state.requested_page_offset,
                kind: state.requested_kind,
                query: state.requested_query.clone(),
            };
            // 超长或空的戳当作读不出，照常重查，不能让它把整个状态判成无效。
            let source =
                source().filter(|source| !source.is_empty() && source.len() <= MAX_SOURCE_BYTES);
            let snapshot_current = !applied_any
                && !refresh_requested
                && source.is_some()
                && state.snapshot_source == source
                && state.snapshot_date.is_some()
                && state.snapshot_error.is_none()
                && state.page_offset == request.offset
                && state.page_kind == request.kind
                && state.page_query == request.query;
            if snapshot_current {
                return Ok(());
            }
            match page(&request) {
                Ok(snapshot) if snapshot.entries.len() <= MAX_PAGE_ENTRIES => {
                    state.entries = snapshot.entries;
                    state.has_more = snapshot.has_more;
                    state.page_offset = request.offset;
                    state.page_kind = request.kind;
                    state.page_query = request.query;
                    state.snapshot_date = Some("updated".to_owned());
                    state.snapshot_error = None;
                    state.snapshot_source = source;
                }
                Ok(_) => {
                    state.snapshot_error = Some("personal dictionary page is too large".into());
                    state.snapshot_source = None;
                }
                Err(error) => {
                    state.snapshot_error = Some(error.chars().take(500).collect());
                    state.snapshot_source = None;
                }
            }
            Ok(())
        })
    }

    /// Drop the receipts in `identities` from [`PersonalDictionaryState::already_present`], once the host has recorded them elsewhere.
    pub fn acknowledge_already_present(
        &self,
        identities: &HashSet<String>,
    ) -> Result<(), PersonalDictionaryError> {
        if identities.is_empty() {
            return Ok(());
        }
        self.update(|state| {
            state
                .already_present
                .retain(|identity| !identities.contains(identity));
            Ok(())
        })
    }

    fn update<F>(&self, action: F) -> Result<(), PersonalDictionaryError>
    where
        F: FnOnce(&mut PersonalDictionaryState) -> Result<(), PersonalDictionaryError>,
    {
        let guard = self.lock()?;
        let mut state = self.read_locked(&guard)?;
        let before = state.clone();
        action(&mut state)?;
        validate_state(&state)?;
        // 状态没变就不重写：原子替换要 fsync，键盘每次聚焦的空同步都会走到这里。
        if state == before {
            return Ok(());
        }
        self.write_locked(&guard, &state)
    }

    fn lock(&self) -> Result<PersonalDictionaryLock, PersonalDictionaryError> {
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(PersonalDictionaryError::InvalidState);
        }
        let directory = crate::file_lock::open_private_directory(&self.directory)?;
        let lock =
            file_lock::open_private_lock_file_at(&directory, std::ffi::OsStr::new("sync.lock"))?;
        if !file_lock::try_exclusive(&lock)? {
            return Err(PersonalDictionaryError::Busy);
        }
        let guard = PersonalDictionaryLock {
            directory,
            _lock: lock,
        };
        Ok(guard)
    }

    fn read_locked(
        &self,
        guard: &PersonalDictionaryLock,
    ) -> Result<PersonalDictionaryState, PersonalDictionaryError> {
        match crate::file_lock::open_private_file_at(
            &guard.directory,
            std::ffi::OsStr::new("sync.json"),
        ) {
            Ok(file) => read_file_handle(file),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(PersonalDictionaryState::default())
            }
            Err(_) => Err(PersonalDictionaryError::InvalidState),
        }
    }

    fn write_locked(
        &self,
        guard: &PersonalDictionaryLock,
        state: &PersonalDictionaryState,
    ) -> Result<(), PersonalDictionaryError> {
        let bytes =
            serde_json::to_vec(&state).map_err(|_| PersonalDictionaryError::InvalidState)?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(PersonalDictionaryError::InvalidState);
        }
        crate::file_lock::write_private_file_at(
            &guard.directory,
            std::ffi::OsStr::new("sync.json"),
            &bytes,
        )?;
        Ok(())
    }
}

fn enqueue_request(
    state: &mut PersonalDictionaryState,
    request: PersonalWordRequest,
) -> Result<(), PersonalDictionaryError> {
    let active = state
        .requests
        .iter()
        .filter(|item| item.status != PersonalWordRequestStatus::Applied)
        .count();
    if active >= MAX_ACTIVE_REQUESTS {
        return Err(PersonalDictionaryError::TooManyRequests);
    }
    let mut identities = HashSet::with_capacity(2);
    identities.extend(request.identities());
    if has_identity_conflict(
        &state.requests,
        &identities,
        &[PersonalWordRequestStatus::Pending],
    ) {
        return Err(PersonalDictionaryError::Conflict);
    }
    prune_history(state);
    state.requests.push(request);
    state.refresh_id = Uuid::new_v4().to_string();
    Ok(())
}

fn has_identity_conflict(
    requests: &[PersonalWordRequest],
    identities: &HashSet<String>,
    statuses: &[PersonalWordRequestStatus],
) -> bool {
    requests.iter().any(|request| {
        statuses.contains(&request.status)
            && request
                .identities()
                .any(|identity| identities.contains(&identity))
    })
}

fn prune_history(state: &mut PersonalDictionaryState) {
    let mut keep = std::collections::HashSet::with_capacity(MAX_HISTORY.min(state.requests.len()));
    keep.extend(
        state
            .requests
            .iter()
            .filter(|request| request.status == PersonalWordRequestStatus::Applied)
            .rev()
            .take(MAX_HISTORY)
            .map(|request| request.id.clone()),
    );
    state.requests.retain(|request| {
        request.status != PersonalWordRequestStatus::Applied || keep.contains(&request.id)
    });
}

fn validate_request_id(id: &str) -> Result<(), PersonalDictionaryError> {
    if !crate::is_bounded_ascii_identifier(id, 120) {
        return Err(PersonalDictionaryError::InvalidRequest);
    }
    Ok(())
}

fn read_file(file: &Path) -> Result<PersonalDictionaryState, PersonalDictionaryError> {
    let metadata = fs::symlink_metadata(file)?;
    if !metadata.file_type().is_file() {
        return Err(PersonalDictionaryError::InvalidState);
    }
    let file_handle = crate::storage::open_private_file_in(file)?;
    read_file_handle(file_handle)
}

fn read_file_handle(file_handle: File) -> Result<PersonalDictionaryState, PersonalDictionaryError> {
    let bytes = crate::bounded_io::read_bounded_file(file_handle, MAX_STATE_BYTES as u64, || {
        PersonalDictionaryError::InvalidState
    })?;
    let state: PersonalDictionaryState =
        serde_json::from_slice(&bytes).map_err(|_| PersonalDictionaryError::InvalidState)?;
    validate_state(&state)?;
    Ok(state)
}

fn validate_state(state: &PersonalDictionaryState) -> Result<(), PersonalDictionaryError> {
    if state.version != 1
        || state.requests.len() > MAX_REQUESTS
        || state.entries.len() > MAX_PAGE_ENTRIES
        || state.page_offset > 1_000_000
        || state.requested_page_offset > 1_000_000
        || state.requested_query.len() > MAX_QUERY_BYTES
        || state.page_query.len() > MAX_QUERY_BYTES
        || state
            .snapshot_source
            .as_ref()
            .is_some_and(|source| source.is_empty() || source.len() > MAX_SOURCE_BYTES)
        || state.refresh_id.is_empty()
        || state.already_present.len() > MAX_ALREADY_PRESENT
        || state.already_present.iter().any(String::is_empty)
    {
        return Err(PersonalDictionaryError::InvalidState);
    }
    let mut ids = std::collections::HashSet::with_capacity(state.requests.len());
    for request in &state.requests {
        if request.id.is_empty() || !ids.insert(&request.id) {
            return Err(PersonalDictionaryError::InvalidState);
        }
        if request.previous.is_none() && request.replacement.is_none() {
            return Err(PersonalDictionaryError::InvalidState);
        }
        if request
            .previous
            .iter()
            .chain(request.replacement.iter())
            .any(|word| word.validate().is_err())
        {
            return Err(PersonalDictionaryError::InvalidState);
        }
    }
    for word in &state.entries {
        if word.validate().is_err() {
            return Err(PersonalDictionaryError::InvalidState);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(key: &str, value: &str) -> PersonalWord {
        PersonalWord {
            kind: PersonalWordKind::QuickPhrase,
            key: key.into(),
            value: value.into(),
            weight: 100_000,
        }
    }

    #[test]
    fn queues_applies_failures_and_pages_without_reapplying_receipts() {
        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        let entry = word("fixture", "测试短语");
        store
            .enqueue(None, Some(entry.clone()), "request-1".into())
            .unwrap();
        assert_eq!(store.read().unwrap().pending_count(), 1);
        let mut applied = Vec::new();
        store
            .synchronize(
                |request| {
                    applied.push(request.id.clone());
                    Ok(())
                },
                |_| {
                    Ok(PersonalWordPage {
                        entries: vec![entry.clone()],
                        has_more: true,
                    })
                },
            )
            .unwrap();
        assert_eq!(applied, ["request-1"]);
        assert_eq!(store.read().unwrap().pending_count(), 0);
        store
            .synchronize(
                |_| panic!("applied request was repeated"),
                |_| Err("page unavailable".into()),
            )
            .unwrap();
        let state = store.read().unwrap();
        assert_eq!(state.entries, vec![entry]);
        assert!(state.snapshot_error.is_some());
    }

    /// 队列空、宿主没要新页、来源戳没变时，同步既不查页也不重写 sync.json；戳变了、宿主要了页、有请求要应用、上次读页失败时都照常重查。
    #[test]
    fn an_idle_sync_with_an_unchanged_source_neither_pages_nor_rewrites() {
        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        let first = word("fixture", "第一");
        let second = word("other", "第二");
        let path = root.path().join("sync.json");
        let pages = std::cell::Cell::new(0);
        let sync = |source: Option<&str>, entries: Vec<PersonalWord>| {
            store
                .synchronize_batch_reporting_present_with_source(
                    |requests| {
                        Ok(requests
                            .iter()
                            .map(|_| Ok(PersonalWordApplied::Written))
                            .collect())
                    },
                    || source.map(str::to_owned),
                    |_| {
                        pages.set(pages.get() + 1);
                        Ok(PersonalWordPage {
                            entries: entries.clone(),
                            has_more: false,
                        })
                    },
                )
                .unwrap();
        };
        let identity = |path: &Path| {
            let metadata = fs::metadata(path).unwrap();
            #[cfg(unix)]
            let inode = std::os::unix::fs::MetadataExt::ino(&metadata);
            #[cfg(not(unix))]
            let inode = 0;
            (inode, metadata.modified().unwrap(), fs::read(path).unwrap())
        };

        sync(Some("a"), vec![first.clone()]);
        assert_eq!(pages.get(), 1);
        assert_eq!(store.read().unwrap().snapshot_source.as_deref(), Some("a"));
        let written = identity(&path);

        sync(Some("a"), vec![second.clone()]);
        assert_eq!(pages.get(), 1, "an unchanged source must not page again");
        assert_eq!(identity(&path), written, "an idle sync rewrote sync.json");
        assert_eq!(store.read().unwrap().entries, vec![first.clone()]);

        // 词库在队列之外变了（例如打字时学到的自造词）：戳不同就重查。
        sync(Some("b"), vec![second.clone()]);
        assert_eq!(pages.get(), 2);
        assert_eq!(store.read().unwrap().entries, vec![second.clone()]);

        // 宿主重新要同一页也要回答，哪怕戳没变。
        store.request_page(0, None, "").unwrap();
        sync(Some("b"), vec![second.clone()]);
        assert_eq!(pages.get(), 3);
        sync(Some("b"), vec![second.clone()]);
        assert_eq!(pages.get(), 3);

        // 应用了请求后重查。
        store
            .enqueue(None, Some(first.clone()), "request-1".into())
            .unwrap();
        sync(Some("b"), vec![first.clone(), second.clone()]);
        assert_eq!(pages.get(), 4);
        assert_eq!(store.read().unwrap().pending_count(), 0);

        // 读不出戳时每次都重查，但内容没变就不重写文件。
        sync(None, vec![first.clone(), second.clone()]);
        assert_eq!(pages.get(), 5);
        let unknown = identity(&path);
        sync(None, vec![first.clone(), second.clone()]);
        assert_eq!(pages.get(), 6);
        assert_eq!(identity(&path), unknown);

        // 读页失败之后，即使戳与失败前相同也要重查。
        sync(Some("c"), vec![first.clone()]);
        assert_eq!(pages.get(), 7);
        store
            .synchronize_batch_reporting_present_with_source(
                |_| Ok(Vec::new()),
                || Some("d".into()),
                |_| Err("page unavailable".into()),
            )
            .unwrap();
        assert!(store.read().unwrap().snapshot_source.is_none());
        sync(Some("d"), vec![second.clone()]);
        assert_eq!(pages.get(), 8);
        assert_eq!(store.read().unwrap().entries, vec![second]);
        assert!(store.read().unwrap().snapshot_error.is_none());
    }

    #[test]
    fn a_page_request_carries_its_filter_to_the_keyboard_and_back() {
        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        let entry = word("dh", "电话");
        store
            .request_page(100, Some(PersonalWordKind::QuickPhrase), " dh ")
            .unwrap();
        let mut seen = None;
        store
            .synchronize(
                |_| Ok(()),
                |request| {
                    seen = Some(request.clone());
                    Ok(PersonalWordPage {
                        entries: vec![entry.clone()],
                        has_more: false,
                    })
                },
            )
            .unwrap();
        assert_eq!(
            seen,
            Some(PersonalPageRequest {
                offset: 100,
                kind: Some(PersonalWordKind::QuickPhrase),
                query: "dh".into(),
            })
        );
        let state = store.read().unwrap();
        assert_eq!(state.page_kind, Some(PersonalWordKind::QuickPhrase));
        assert_eq!(state.page_query, "dh");
        let text = fs::read_to_string(root.path().join("sync.json")).unwrap();
        assert!(text.contains(r#""requestedKind":"quickPhrase""#), "{text}");
        assert!(text.contains(r#""pageQuery":"dh""#), "{text}");

        assert!(matches!(
            store.request_page(0, None, &"a".repeat(MAX_QUERY_BYTES + 1)),
            Err(PersonalDictionaryError::InvalidRequest)
        ));
    }

    #[test]
    fn import_validation_and_conflicts_are_atomic() {
        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        let first = word("one", "一");
        let second = word("two", "二");
        store
            .enqueue_import(vec![first.clone(), second.clone()], "import".into())
            .unwrap();
        let original = fs::read(root.path().join("sync.json")).unwrap();
        assert!(matches!(
            store.enqueue_import(vec![first, word("three", "三")], "second".into()),
            Err(PersonalDictionaryError::Conflict)
        ));
        assert_eq!(fs::read(root.path().join("sync.json")).unwrap(), original);
        assert_eq!(store.read().unwrap().requests.len(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn operation_lock_release_is_independent_of_inherited_descriptors() {
        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        let owner = store.lock().unwrap();
        // 副本与 fork 继承的描述符一样，共享同一打开文件描述；无需依赖进程调度撞中窗口。
        let inherited = owner._lock.try_clone().unwrap();
        assert!(matches!(
            store.request_page(0, None, ""),
            Err(PersonalDictionaryError::Busy)
        ));
        drop(owner);

        store.request_page(0, None, "fixture").unwrap();
        assert_eq!(store.read().unwrap().requested_query, "fixture");

        let next_owner = store.lock().unwrap();
        // 旧副本销毁不能解开另一次操作独立获取的锁，失败的争用方也不能解开它。
        drop(inherited);
        assert!(matches!(
            store.request_page(0, None, ""),
            Err(PersonalDictionaryError::Busy)
        ));
        assert!(matches!(
            store.request_page(0, None, ""),
            Err(PersonalDictionaryError::Busy)
        ));
        drop(next_owner);
        store.request_page(0, None, "").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn writes_stay_bound_to_the_locked_directory_after_replacement() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("personal");
        fs::create_dir(&directory).unwrap();
        let store = PersonalDictionaryStore::new(&directory);
        let lock = store.lock().unwrap();

        let moved = root.path().join("personal-moved");
        fs::rename(&directory, &moved).unwrap();
        fs::create_dir(&directory).unwrap();

        store
            .write_locked(&lock, &PersonalDictionaryState::default())
            .unwrap();

        assert!(moved.join("sync.json").exists());
        assert!(!directory.join("sync.json").exists());
    }

    #[test]
    fn large_import_yields_between_four_entry_batches_and_preserves_receipts() {
        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        let words: Vec<_> = (0..9)
            .map(|index| word(&format!("batch{}", char::from(b'a' + index)), "fixture"))
            .collect();
        store.enqueue_import(words, "import".into()).unwrap();
        let ids: Vec<_> = store
            .read()
            .unwrap()
            .requests
            .iter()
            .map(|request| request.id.clone())
            .collect();
        let refresh_id = store.read().unwrap().refresh_id;
        let mut applied = Vec::new();
        for expected in [4, 8, 9] {
            store
                .synchronize(
                    |request| {
                        applied.push(request.id.clone());
                        Ok(())
                    },
                    |_| {
                        Ok(PersonalWordPage {
                            entries: Vec::new(),
                            has_more: false,
                        })
                    },
                )
                .unwrap();
            assert_eq!(applied, ids[..expected]);
            let state = store.read().unwrap();
            assert_eq!(state.pending_count(), 9 - expected);
            assert_eq!(
                state.completed_refresh_id.as_deref(),
                (expected == 9).then_some(refresh_id.as_str())
            );
        }
    }

    #[test]
    fn malformed_state_is_rejected_without_being_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path();
        fs::write(directory.join("sync.json"), b"not-json").unwrap();
        let store = PersonalDictionaryStore::new(directory);
        assert!(matches!(
            store.read(),
            Err(PersonalDictionaryError::InvalidState)
        ));
        assert_eq!(fs::read(directory.join("sync.json")).unwrap(), b"not-json");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_personal_dictionary_paths() {
        use std::os::unix::fs::symlink;

        let target = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked_root = parent.path().join("user-data");
        symlink(target.path(), &linked_root).unwrap();
        let linked = PersonalDictionaryStore::new(&linked_root);
        assert!(matches!(linked.read(), Err(PersonalDictionaryError::Io(_))));
        assert!(!target.path().join("sync.json").exists());

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("sync.json");
        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("sync.json");
        fs::write(&outside_path, b"{}").unwrap();
        symlink(&outside_path, &path).unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        assert!(matches!(
            store.read(),
            Err(PersonalDictionaryError::InvalidState)
        ));
        assert!(store
            .enqueue(None, Some(word("ni", "ni")), "synthetic-id".into())
            .is_err());
    }

    #[test]
    fn oversized_state_is_rejected_before_json_decode() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("sync.json");
        fs::write(&path, vec![b'x'; MAX_STATE_BYTES + 1]).unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        assert!(matches!(
            store.read(),
            Err(PersonalDictionaryError::InvalidState)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_a_fixed_personal_dictionary_temporary_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let temporary = root
            .path()
            .join(format!("sync.json.tmp-{}", std::process::id()));
        let outside_file = outside.path().join("sync.json");
        fs::write(&outside_file, b"keep me").unwrap();
        symlink(&outside_file, &temporary).unwrap();
        let store = PersonalDictionaryStore::new(root.path());

        store
            .enqueue(None, Some(word("ni", "你")), "synthetic-id".into())
            .unwrap();
        assert_eq!(fs::read(outside_file).unwrap(), b"keep me");
        assert!(root.path().join("sync.json").is_file());
    }

    #[test]
    fn transport_validation_matches_engine_multiline_quick_phrases() {
        assert!(word("fixture", "first line\nsecond\tcolumn")
            .validate()
            .is_ok());
        assert!(PersonalWord {
            kind: PersonalWordKind::Pinyin,
            key: "ni".into(),
            value: "first\nsecond".into(),
            weight: 100_000,
        }
        .validate()
        .is_err());
        assert!(word("fixture", "first\0second").validate().is_err());
    }

    #[test]
    fn digit_quick_phrase_codes_are_refused_for_new_input_but_stored_rows_stay_usable() {
        let legacy = word("nh1", "你好");
        assert!(legacy.validate().is_ok());
        assert!(legacy.validate_new().is_err());
        assert!(word("nh", "你好").validate_new().is_ok());

        let root = tempfile::tempdir().unwrap();
        let store = PersonalDictionaryStore::new(root.path());
        assert!(matches!(
            store.enqueue(None, Some(legacy.clone()), "add".into()),
            Err(PersonalDictionaryError::InvalidRequest)
        ));
        assert!(matches!(
            store.enqueue_import(vec![legacy.clone()], "import".into()),
            Err(PersonalDictionaryError::InvalidRequest)
        ));

        // A row the Engine already holds with a digit still lands in the stored snapshot and loads back.
        store
            .synchronize(
                |_| Ok(()),
                |_| {
                    Ok(PersonalWordPage {
                        entries: vec![legacy.clone()],
                        has_more: false,
                    })
                },
            )
            .unwrap();
        assert_eq!(store.read().unwrap().entries, vec![legacy.clone()]);
        // It can be deleted, and edited to a letters-only code, but not re-saved with the digit.
        assert!(matches!(
            store.enqueue(
                Some(legacy.clone()),
                Some(word("nh2", "你好")),
                "edit-digit".into()
            ),
            Err(PersonalDictionaryError::InvalidRequest)
        ));
        store.enqueue(Some(legacy), None, "delete".into()).unwrap();
        assert_eq!(store.read().unwrap().pending_count(), 1);
    }
}
