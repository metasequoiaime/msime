//! 命名词库：用户自建、导入或从社区安装的一组词条，可以单独启用和停用。
//!
//! Engine 只有一个用户词库，这里不改它，只在 client-core 里加一层集合的元数据；词条真正写进用户词库仍然走个人词库的跨进程队列（[`PersonalDictionaryStore`]），键盘在下一次同步时应用。
//!
//! 存储在 `<preferences_directory>/DictionaryCollections/` 下：`index.json` 是集合列表，`index.json.lock` 是整个目录的文件锁，每个集合的词条在 `<uuid>.json`（所以停用以后还能重新启用），`outbox.json` 是还没交给个人词库队列的增删。个人词库队列同时最多只接受 128 个未完成的请求，一个两万条的集合要分很多批送进去，所以启用和停用先记进待发送队列，再由 [`DictionaryCollectionsStore::flush`] 在队列有空位时一批批送出；每次修改之后也会顺手送一批。
//!
//! 待发送队列按词条身份（[`PersonalWord::identity`]）合并：同一个词先排了「加入」又排「删除」（或反过来）时，两者都还没送出，直接互相抵消。停用或删除一个集合时，只删除不属于任何其他已启用集合的词，Engine 自己学来的词不属于任何集合，不会被删掉。内置主词库不是集合，常开、不能停用。

use crate::community::resource::{validate_resource, CommunityResource, CommunityResourceKind};
use crate::dictionary::import::{self, ImportError, ImportFailure, ImportFormat, ImportKind};
use crate::dictionary::personal::{
    PersonalDictionaryError, PersonalDictionaryStore, PersonalWord, PersonalWordKind,
    PersonalWordRequestStatus,
};
use crate::file_lock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// 集合目录在偏好目录里的名字。
pub const DIRECTORY_NAME: &str = "DictionaryCollections";
/// 每个集合最多的词条数。
pub const MAX_COLLECTION_ENTRIES: usize = 20_000;
/// 最多的集合个数。
pub const MAX_COLLECTIONS: usize = 32;
/// 集合名最多的字符数。
pub const MAX_NAME_CHARS: usize = 32;
/// 导入接受的格式，`load` 原样返回给界面。`txt` 是 `standard` 的别名（列的顺序读不出词条时自动换一种），`hans` 是每行一个汉字词、读音由宿主经 Engine 给出。
pub const IMPORT_FORMATS: [&str; 5] = ["txt", "standard", "windows", "hans", "rime"];
/// 内置主词库在请求里的 id 前缀（例如 `builtin:pinyin`）。它不是集合，任何修改都会被拒绝。
pub const BUILTIN_ID_PREFIX: &str = "builtin";
/// 一次导入的文本上限。
pub const MAX_IMPORT_TEXT_BYTES: usize = 16 * 1024 * 1024;
/// 词条正文的字节上限，与导入解析器相同。
const MAX_VALUE_BYTES: usize = 1024;
/// 个人词库队列同时接受的未完成请求数（`personal.rs` 的 `MAX_ACTIVE_REQUESTS`），也是一批最多送出的条数。
const PERSONAL_QUEUE_CAPACITY: usize = 128;
const MAX_INDEX_BYTES: u64 = 1024 * 1024;
const MAX_COLLECTION_BYTES: u64 = 48 * 1024 * 1024;
const MAX_OUTBOX_BYTES: u64 = 96 * 1024 * 1024;
const INDEX_FILE: &str = "index.json";
const OUTBOX_FILE: &str = "outbox.json";

/// 集合的来源。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollectionSource {
    User,
    Import { format: String },
    Community { resource_id: Uuid, revision: u32 },
}

/// `index.json` 里的一项。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionMetadata {
    pub id: Uuid,
    pub name: String,
    pub kind: PersonalWordKind,
    pub source: CollectionSource,
    pub enabled: bool,
    pub entry_count: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CollectionIndex {
    version: u32,
    collections: Vec<CollectionMetadata>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CollectionFile {
    id: Uuid,
    kind: PersonalWordKind,
    entries: Vec<PersonalWord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum OutboxKind {
    Add,
    Remove,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OutboxOperation {
    op: OutboxKind,
    collection: Uuid,
    word: PersonalWord,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct OutboxFile {
    operations: Vec<OutboxOperation>,
}

/// 界面上的一个集合：元数据，加上还没交给个人词库队列的增删条数。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CollectionSummary {
    pub id: Uuid,
    pub name: String,
    pub kind: PersonalWordKind,
    pub source: CollectionSource,
    pub enabled: bool,
    pub entry_count: usize,
    pub pending: usize,
}

/// 一次导入里每一行的去向。行号从整份文本开头数起，失败原因只描述形状，不回显内容。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CollectionImportReport {
    pub imported: usize,
    /// 与集合里已有的词条（或同一文件里更早的一行）相同而没有再加的行数。
    pub duplicates: usize,
    pub failed: usize,
    pub first_failures: Vec<ImportFailure>,
    /// 集合已满，后面的行没有读。
    pub truncated: bool,
    /// 按要求的列顺序读不出词条，按另一种顺序读了。
    pub swapped: bool,
}

/// 每次操作返回的整份视图。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DictionaryCollectionsView {
    pub collections: Vec<CollectionSummary>,
    pub formats: [&'static str; 5],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<CollectionImportReport>,
    /// 只在 `flush` 时给出：这次送进个人词库队列的条数。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent: Option<usize>,
}

/// 一次操作，形状与 C ABI 的 `action` 相同（`operation` 区分种类）。集合 id 是字符串，内置词库的 `builtin:<kind>` 也能传进来并得到 `builtin_locked`。
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum DictionaryCollectionsAction {
    Load,
    Create {
        name: String,
        kind: PersonalWordKind,
    },
    Rename {
        id: String,
        name: String,
    },
    Delete {
        id: String,
    },
    SetEnabled {
        id: String,
        enabled: bool,
    },
    AddWords {
        id: String,
        entries: Vec<PersonalWord>,
    },
    RemoveWords {
        id: String,
        entries: Vec<PersonalWord>,
    },
    Import {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        name: Option<String>,
        kind: PersonalWordKind,
        format: String,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        bytes_base64: Option<String>,
    },
    InstallCommunity {
        resource: Box<CommunityResource>,
    },
    Flush,
}

/// `hans` 格式的读音来源。client-core 不依赖 Engine，宿主把 Engine 的「汉字转拼音」包成这个接口传进来。
pub trait HanReadings {
    /// `word` 的拼音编码（例如 `ni'hao`），给不出时为 `None`。
    fn pinyin(&self, word: &str) -> Option<String>;
}

#[derive(Debug, Error)]
pub enum DictionaryCollectionsError {
    #[error("dictionary collections storage failed")]
    Io(#[from] std::io::Error),
    #[error("dictionary collections are malformed; the files were left unchanged")]
    Corrupt,
    #[error("dictionary collection request is invalid")]
    Invalid,
    #[error("dictionary collection name is invalid")]
    InvalidName,
    #[error("dictionary collection limit reached")]
    Limit,
    #[error("dictionary collection was not found")]
    NotFound,
    #[error("the built-in dictionary is always enabled")]
    BuiltinLocked,
    #[error("dictionary import format is not supported")]
    UnsupportedFormat,
    #[error("dictionary import failed: {0}")]
    Import(ImportError),
    #[error("dictionary collections would exceed their size limit")]
    TooLarge,
    #[error("personal dictionary queue failed: {0}")]
    Personal(#[from] PersonalDictionaryError),
}

impl DictionaryCollectionsError {
    /// 交给宿主的稳定错误码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "collections_io",
            Self::Corrupt => "collections_corrupt",
            Self::Invalid => "collections_invalid",
            Self::InvalidName => "collections_name_invalid",
            Self::Limit => "collections_limit",
            Self::NotFound => "collections_not_found",
            Self::BuiltinLocked => "builtin_locked",
            Self::UnsupportedFormat | Self::Import(ImportError::UnsupportedFormat) => {
                "unsupported_format"
            }
            Self::Import(ImportError::Empty) => "import_empty",
            Self::Import(ImportError::TooLarge) | Self::TooLarge => "collections_too_large",
            Self::Import(ImportError::ControlCharacters) => "import_control_characters",
            Self::Import(ImportError::NoUsableRows) => "import_no_usable_rows",
            Self::Personal(PersonalDictionaryError::Busy) => "personal_dictionary_busy",
            Self::Personal(_) => "personal_dictionary_failed",
        }
    }
}

type Result<T> = std::result::Result<T, DictionaryCollectionsError>;

/// 集合存储。`directory` 是偏好目录，`personal` 是个人词库队列。
pub struct DictionaryCollectionsStore {
    directory: PathBuf,
    personal: PersonalDictionaryStore,
}

impl DictionaryCollectionsStore {
    pub fn new(preferences_directory: impl AsRef<Path>, personal: PersonalDictionaryStore) -> Self {
        Self {
            directory: preferences_directory.as_ref().join(DIRECTORY_NAME),
            personal,
        }
    }

    /// 执行一个操作。`readings` 只有 `hans` 导入要用，没有时 `hans` 导入返回 `unsupported_format`。
    pub fn perform(
        &self,
        action: DictionaryCollectionsAction,
        readings: Option<&dyn HanReadings>,
    ) -> Result<DictionaryCollectionsView> {
        match action {
            DictionaryCollectionsAction::Load => self.load(),
            DictionaryCollectionsAction::Create { name, kind } => self.create(&name, kind),
            DictionaryCollectionsAction::Rename { id, name } => self.rename(&id, &name),
            DictionaryCollectionsAction::Delete { id } => self.delete(&id),
            DictionaryCollectionsAction::SetEnabled { id, enabled } => {
                self.set_enabled(&id, enabled)
            }
            DictionaryCollectionsAction::AddWords { id, entries } => self.add_words(&id, entries),
            DictionaryCollectionsAction::RemoveWords { id, entries } => {
                self.remove_words(&id, &entries)
            }
            DictionaryCollectionsAction::Import {
                id,
                name,
                kind,
                format,
                text,
                bytes_base64,
            } => {
                // 搜狗 `.scel` 这类二进制格式还没有解析器；文本以外的内容一律按不支持处理。
                if bytes_base64.is_some() {
                    return Err(DictionaryCollectionsError::UnsupportedFormat);
                }
                let text = text.ok_or(DictionaryCollectionsError::Invalid)?;
                self.import(ImportRequest {
                    id: id.as_deref(),
                    name: name.as_deref(),
                    kind,
                    format: &format,
                    text: &text,
                    readings,
                })
            }
            DictionaryCollectionsAction::InstallCommunity { resource } => {
                self.install_community(&resource)
            }
            DictionaryCollectionsAction::Flush => self.flush(),
        }
    }

    pub fn load(&self) -> Result<DictionaryCollectionsView> {
        let _lock = self.lock()?;
        let state = self.read_state()?;
        Ok(state.view())
    }

    pub fn create(&self, name: &str, kind: PersonalWordKind) -> Result<DictionaryCollectionsView> {
        valid_name(name)?;
        self.change(|state| {
            state.insert(name, kind, CollectionSource::User, Vec::new())?;
            Ok(())
        })
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<DictionaryCollectionsView> {
        let id = parse_id(id)?;
        valid_name(name)?;
        self.change(|state| {
            state.metadata_mut(id)?.name = name.to_owned();
            Ok(())
        })
    }

    /// 删除集合；它若是启用的，先像停用一样排好需要删掉的词。
    pub fn delete(&self, id: &str) -> Result<DictionaryCollectionsView> {
        let id = parse_id(id)?;
        self.change(|state| {
            let index = state.position(id)?;
            if state.index.collections[index].enabled {
                state.disable(id)?;
            }
            state.index.collections.remove(index);
            state.entries.remove(&id);
            state.deleted.push(id);
            Ok(())
        })
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<DictionaryCollectionsView> {
        let id = parse_id(id)?;
        self.change(|state| {
            let metadata = state.metadata_mut(id)?;
            if metadata.enabled == enabled {
                return Ok(());
            }
            if enabled {
                state.metadata_mut(id)?.enabled = true;
                state.enable(id)
            } else {
                state.disable(id)?;
                state.metadata_mut(id)?.enabled = false;
                Ok(())
            }
        })
    }

    /// 往集合里加词。已有的同一个词（编码和词相同）不再加；集合是启用的时候，新词排进待发送队列。
    pub fn add_words(
        &self,
        id: &str,
        words: Vec<PersonalWord>,
    ) -> Result<DictionaryCollectionsView> {
        let id = parse_id(id)?;
        self.change(|state| {
            let kind = state.metadata(id)?.kind;
            for word in &words {
                valid_new_word(word, kind)?;
            }
            state.append(id, words)?;
            Ok(())
        })
    }

    pub fn remove_words(
        &self,
        id: &str,
        words: &[PersonalWord],
    ) -> Result<DictionaryCollectionsView> {
        let id = parse_id(id)?;
        if words.iter().any(|word| word.validate().is_err()) {
            return Err(DictionaryCollectionsError::Invalid);
        }
        self.change(|state| {
            let identities: HashSet<String> = words.iter().map(PersonalWord::identity).collect();
            state.remove_identities(id, &identities)
        })
    }

    /// 从一个文件导入：有 `id` 时加进那个集合，否则新建一个以 `name` 命名、来源为 `import{format}` 的集合（默认启用）。
    pub fn import(&self, request: ImportRequest<'_>) -> Result<DictionaryCollectionsView> {
        if !IMPORT_FORMATS.contains(&request.format) {
            return Err(DictionaryCollectionsError::UnsupportedFormat);
        }
        let target = request.id.map(parse_id).transpose()?;
        if target.is_none() {
            valid_name(
                request
                    .name
                    .ok_or(DictionaryCollectionsError::InvalidName)?,
            )?;
        }
        let _lock = self.lock()?;
        let mut state = self.read_state()?;
        let existing: HashSet<String> = match target {
            Some(id) => {
                if state.metadata(id)?.kind != request.kind {
                    return Err(DictionaryCollectionsError::Invalid);
                }
                state
                    .entries(id)
                    .iter()
                    .map(PersonalWord::identity)
                    .collect()
            }
            None => {
                if state.index.collections.len() >= MAX_COLLECTIONS {
                    return Err(DictionaryCollectionsError::Limit);
                }
                HashSet::new()
            }
        };
        let room = MAX_COLLECTION_ENTRIES.saturating_sub(existing.len());
        let (words, mut report) = parse_import(
            request.kind,
            request.format,
            request.text,
            request.readings,
            &existing,
            room,
        )?;
        report.imported = words.len();
        if words.is_empty() && target.is_none() {
            return Err(DictionaryCollectionsError::Import(
                ImportError::NoUsableRows,
            ));
        }
        match target {
            Some(id) => state.append(id, words)?,
            None => {
                let format = if request.format == "txt" {
                    "standard"
                } else {
                    request.format
                };
                state.insert(
                    request.name.unwrap_or_default(),
                    request.kind,
                    CollectionSource::Import {
                        format: format.to_owned(),
                    },
                    words,
                )?;
            }
        }
        self.commit(&mut state)?;
        let mut view = state.view();
        view.import = Some(report);
        Ok(view)
    }

    /// 安装或更新一个社区词库。同一个资源再装一次时更新已有的集合：去掉新版本里没有的词，加上新增的词。
    pub fn install_community(
        &self,
        resource: &CommunityResource,
    ) -> Result<DictionaryCollectionsView> {
        if resource.kind != CommunityResourceKind::Dictionary
            || validate_resource(resource).is_err()
        {
            return Err(DictionaryCollectionsError::Invalid);
        }
        let mut kind = None;
        let mut words = Vec::with_capacity(resource.content.entries.len());
        for entry in &resource.content.entries {
            let entry_kind = personal_kind(entry.kind);
            // 一个集合只有一种词库；混了几种词库的资源装不进同一个集合。
            if kind.is_some_and(|kind| kind != entry_kind) {
                return Err(DictionaryCollectionsError::Invalid);
            }
            kind = Some(entry_kind);
            let word = PersonalWord {
                kind: entry_kind,
                key: entry.code.to_ascii_lowercase(),
                value: entry.word.clone(),
                weight: entry.weight,
            };
            valid_new_word(&word, entry_kind)?;
            words.push(word);
        }
        let kind = kind.ok_or(DictionaryCollectionsError::Invalid)?;
        let source = CollectionSource::Community {
            resource_id: resource.id,
            revision: resource.revision,
        };
        self.change(|state| {
            let existing = state.index.collections.iter().find(|metadata| {
                matches!(metadata.source, CollectionSource::Community { resource_id, .. } if resource_id == resource.id)
            });
            let Some(existing) = existing.cloned() else {
                state.insert(&resource.name, kind, source, words)?;
                return Ok(());
            };
            if existing.kind != kind {
                return Err(DictionaryCollectionsError::Invalid);
            }
            let incoming: HashSet<String> = words.iter().map(PersonalWord::identity).collect();
            let stale: HashSet<String> = state
                .entries(existing.id)
                .iter()
                .map(PersonalWord::identity)
                .filter(|identity| !incoming.contains(identity))
                .collect();
            state.remove_identities(existing.id, &stale)?;
            state.append(existing.id, words)?;
            state.metadata_mut(existing.id)?.source = source;
            Ok(())
        })
    }

    /// 把待发送队列里的增删在个人词库队列有空位时送进去，返回这次送出的条数。个人词库正忙（键盘持有它的锁）或队列已满时什么也不送，等下次再来。
    pub fn flush(&self) -> Result<DictionaryCollectionsView> {
        let _lock = self.lock()?;
        let mut state = self.read_state()?;
        let (sent, failure) = self.flush_locked(&mut state);
        // 已经送出的部分先落盘，再报告送后面那部分时遇到的错误，免得下次重复送。
        if sent > 0 {
            self.write_outbox(&state.outbox)?;
        }
        if let Some(error) = failure {
            return Err(error);
        }
        let mut view = state.view();
        view.sent = Some(sent);
        Ok(view)
    }

    fn change(
        &self,
        action: impl FnOnce(&mut State) -> Result<()>,
    ) -> Result<DictionaryCollectionsView> {
        let _lock = self.lock()?;
        let mut state = self.read_state()?;
        action(&mut state)?;
        self.commit(&mut state)?;
        Ok(state.view())
    }

    /// 写回改过的文件，再顺手送一批。送这一批失败不影响已经写好的修改：集合和待发送队列都已落盘，`flush` 操作会如实报告个人词库的错误。
    fn commit(&self, state: &mut State) -> Result<()> {
        for id in state.dirty.iter().filter(|id| !state.deleted.contains(id)) {
            let entries = state
                .entries
                .get(id)
                .ok_or(DictionaryCollectionsError::NotFound)?;
            let kind = state.metadata(*id)?.kind;
            self.write_json(
                &self.collection_path(*id),
                &CollectionFile {
                    id: *id,
                    kind,
                    entries: entries.clone(),
                },
                MAX_COLLECTION_BYTES,
            )?;
        }
        self.write_outbox(&state.outbox)?;
        self.write_json(
            &self.directory.join(INDEX_FILE),
            &state.index,
            MAX_INDEX_BYTES,
        )?;
        for id in &state.deleted {
            match fs::remove_file(self.collection_path(*id)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let (sent, _) = self.flush_locked(state);
        if sent > 0 {
            self.write_outbox(&state.outbox)?;
        }
        Ok(())
    }

    /// 送一批，返回送出的条数和送后面那部分时遇到的错误（个人词库正忙或队列已满不算错误）。送出的条目已经从内存里的待发送队列拿掉，调用方负责把它写回。
    fn flush_locked(&self, state: &mut State) -> (usize, Option<DictionaryCollectionsError>) {
        if state.outbox.is_empty() {
            return (0, None);
        }
        let queue = match self.personal.read() {
            Ok(queue) => queue,
            Err(error) => return (0, Some(error.into())),
        };
        let active = queue
            .requests
            .iter()
            .filter(|request| request.status != PersonalWordRequestStatus::Applied)
            .count();
        let mut capacity = PERSONAL_QUEUE_CAPACITY.saturating_sub(active);
        // 已有等待中或失败的请求的词这次不送：个人词库队列会因为它拒绝整批。失败的请求由用户在个人词库页重试或忽略之后，这些词下次再送。
        let mut waiting = HashSet::new();
        let mut failed = HashSet::new();
        for request in &queue.requests {
            let target = match request.status {
                PersonalWordRequestStatus::Pending => &mut waiting,
                PersonalWordRequestStatus::Failed => &mut failed,
                PersonalWordRequestStatus::Applied => continue,
            };
            for word in request.previous.iter().chain(request.replacement.iter()) {
                target.insert(word.identity());
            }
        }
        let mut sent = 0;
        let additions: Vec<String> = state
            .outbox
            .operations()
            .filter(|operation| operation.op == OutboxKind::Add)
            .map(|operation| operation.word.identity())
            .filter(|identity| !waiting.contains(identity) && !failed.contains(identity))
            .take(capacity)
            .collect();
        if !additions.is_empty() {
            let words: Vec<PersonalWord> = additions
                .iter()
                .filter_map(|identity| {
                    state
                        .outbox
                        .get(identity)
                        .map(|operation| operation.word.clone())
                })
                .collect();
            match self
                .personal
                .enqueue_import(words, format!("collections-{}", Uuid::new_v4().simple()))
            {
                Ok(()) => {
                    for identity in &additions {
                        state.outbox.take(identity);
                    }
                    sent += additions.len();
                    capacity -= additions.len();
                }
                Err(error) if transient(&error) => return (sent, None),
                Err(error) => return (sent, Some(error.into())),
            }
        }
        let removals: Vec<String> = state
            .outbox
            .operations()
            .filter(|operation| operation.op == OutboxKind::Remove)
            .map(|operation| operation.word.identity())
            .filter(|identity| !waiting.contains(identity))
            .take(capacity)
            .collect();
        for identity in removals {
            let Some(operation) = state.outbox.get(&identity) else {
                continue;
            };
            match self.personal.enqueue(
                Some(operation.word.clone()),
                None,
                format!("collections-{}", Uuid::new_v4().simple()),
            ) {
                Ok(()) => {
                    state.outbox.take(&identity);
                    sent += 1;
                }
                Err(error) if transient(&error) => break,
                Err(error) => return (sent, Some(error.into())),
            }
        }
        (sent, None)
    }

    fn collection_path(&self, id: Uuid) -> PathBuf {
        self.directory.join(format!("{}.json", id.hyphenated()))
    }

    fn lock(&self) -> Result<File> {
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(DictionaryCollectionsError::Corrupt);
        }
        let lock = file_lock::open_lock_file(self.directory.join("index.json.lock"))?;
        file_lock::exclusive(&lock)?;
        Ok(lock)
    }

    fn read_state(&self) -> Result<State> {
        let index: CollectionIndex = self
            .read_json(&self.directory.join(INDEX_FILE), MAX_INDEX_BYTES)?
            .unwrap_or(CollectionIndex {
                version: 1,
                collections: Vec::new(),
            });
        validate_index(&index)?;
        let mut entries = HashMap::with_capacity(index.collections.len());
        for metadata in &index.collections {
            let file: CollectionFile = self
                .read_json(&self.collection_path(metadata.id), MAX_COLLECTION_BYTES)?
                .ok_or(DictionaryCollectionsError::Corrupt)?;
            validate_collection(metadata, &file)?;
            entries.insert(metadata.id, file.entries);
        }
        let outbox: OutboxFile = self
            .read_json(&self.directory.join(OUTBOX_FILE), MAX_OUTBOX_BYTES)?
            .unwrap_or_default();
        let outbox = Outbox::from_file(outbox)?;
        Ok(State {
            index,
            entries,
            outbox,
            dirty: HashSet::new(),
            deleted: Vec::new(),
        })
    }

    fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
        maximum: u64,
    ) -> Result<Option<T>> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() || metadata.len() > maximum {
            return Err(DictionaryCollectionsError::Corrupt);
        }
        let bytes = crate::bounded_io::read_bounded_file(File::open(path)?, maximum, || {
            DictionaryCollectionsError::Corrupt
        })?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| DictionaryCollectionsError::Corrupt)
    }

    fn write_outbox(&self, outbox: &Outbox) -> Result<()> {
        self.write_json(
            &self.directory.join(OUTBOX_FILE),
            &OutboxFile {
                operations: outbox.operations().cloned().collect(),
            },
            MAX_OUTBOX_BYTES,
        )
    }

    fn write_json<T: Serialize>(&self, path: &Path, value: &T, maximum: u64) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|_| DictionaryCollectionsError::Invalid)?;
        if bytes.len() as u64 > maximum {
            return Err(DictionaryCollectionsError::TooLarge);
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// [`DictionaryCollectionsStore::import`] 的参数。
pub struct ImportRequest<'a> {
    pub id: Option<&'a str>,
    pub name: Option<&'a str>,
    pub kind: PersonalWordKind,
    pub format: &'a str,
    pub text: &'a str,
    pub readings: Option<&'a dyn HanReadings>,
}

/// 个人词库正忙、队列已满或某个词已有等待中的请求：下次再送，不算失败。
fn transient(error: &PersonalDictionaryError) -> bool {
    matches!(
        error,
        PersonalDictionaryError::Busy
            | PersonalDictionaryError::TooManyRequests
            | PersonalDictionaryError::Conflict
    )
}

fn parse_id(id: &str) -> Result<Uuid> {
    if id == BUILTIN_ID_PREFIX || id.starts_with(&format!("{BUILTIN_ID_PREFIX}:")) {
        return Err(DictionaryCollectionsError::BuiltinLocked);
    }
    match Uuid::parse_str(id) {
        Ok(id) if !id.is_nil() => Ok(id),
        _ => Err(DictionaryCollectionsError::Invalid),
    }
}

fn valid_name(name: &str) -> Result<()> {
    if crate::community::valid_name(name) && name.chars().count() <= MAX_NAME_CHARS {
        Ok(())
    } else {
        Err(DictionaryCollectionsError::InvalidName)
    }
}

fn valid_stored_word(word: &PersonalWord, kind: PersonalWordKind) -> bool {
    word.kind == kind && word.validate().is_ok() && word.value.len() <= MAX_VALUE_BYTES
}

fn valid_new_word(word: &PersonalWord, kind: PersonalWordKind) -> Result<()> {
    if valid_stored_word(word, kind) && word.validate_new().is_ok() {
        Ok(())
    } else {
        Err(DictionaryCollectionsError::Invalid)
    }
}

fn personal_kind(kind: crate::cloud::dictionary::DictionaryKind) -> PersonalWordKind {
    use crate::cloud::dictionary::DictionaryKind;
    match kind {
        DictionaryKind::Pinyin => PersonalWordKind::Pinyin,
        DictionaryKind::Wubi => PersonalWordKind::Wubi,
        DictionaryKind::Wubi98 => PersonalWordKind::Wubi98,
        DictionaryKind::Quick => PersonalWordKind::QuickPhrase,
        DictionaryKind::English => PersonalWordKind::English,
    }
}

fn import_kind(kind: PersonalWordKind) -> ImportKind {
    match kind {
        PersonalWordKind::Pinyin => ImportKind::Pinyin,
        PersonalWordKind::Wubi => ImportKind::Wubi,
        PersonalWordKind::Wubi98 => ImportKind::Wubi98,
        PersonalWordKind::QuickPhrase => ImportKind::QuickPhrase,
        PersonalWordKind::English => ImportKind::English,
    }
}

fn validate_index(index: &CollectionIndex) -> Result<()> {
    if index.version != 1 || index.collections.len() > MAX_COLLECTIONS {
        return Err(DictionaryCollectionsError::Corrupt);
    }
    let mut ids = HashSet::with_capacity(index.collections.len());
    for metadata in &index.collections {
        let source_valid = match &metadata.source {
            CollectionSource::User => true,
            CollectionSource::Import { format } => IMPORT_FORMATS.contains(&format.as_str()),
            CollectionSource::Community {
                resource_id,
                revision,
            } => !resource_id.is_nil() && *revision != 0,
        };
        if metadata.id.is_nil()
            || !ids.insert(metadata.id)
            || valid_name(&metadata.name).is_err()
            || metadata.entry_count > MAX_COLLECTION_ENTRIES
            || !source_valid
        {
            return Err(DictionaryCollectionsError::Corrupt);
        }
    }
    Ok(())
}

fn validate_collection(metadata: &CollectionMetadata, file: &CollectionFile) -> Result<()> {
    if file.id != metadata.id
        || file.kind != metadata.kind
        || file.entries.len() != metadata.entry_count
        || file.entries.len() > MAX_COLLECTION_ENTRIES
    {
        return Err(DictionaryCollectionsError::Corrupt);
    }
    let mut identities = HashSet::with_capacity(file.entries.len());
    if file
        .entries
        .iter()
        .any(|word| !valid_stored_word(word, metadata.kind) || !identities.insert(word.identity()))
    {
        return Err(DictionaryCollectionsError::Corrupt);
    }
    Ok(())
}

/// 待发送队列：按词条身份唯一，保持排队顺序。
#[derive(Default)]
struct Outbox {
    operations: Vec<Option<OutboxOperation>>,
    positions: HashMap<String, usize>,
}

impl Outbox {
    fn from_file(file: OutboxFile) -> Result<Self> {
        let mut outbox = Self::default();
        for operation in file.operations {
            if operation.word.validate().is_err() || operation.collection.is_nil() {
                return Err(DictionaryCollectionsError::Corrupt);
            }
            let identity = operation.word.identity();
            if outbox.positions.contains_key(&identity) {
                return Err(DictionaryCollectionsError::Corrupt);
            }
            outbox.positions.insert(identity, outbox.operations.len());
            outbox.operations.push(Some(operation));
        }
        Ok(outbox)
    }

    fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    fn operations(&self) -> impl Iterator<Item = &OutboxOperation> {
        self.operations.iter().flatten()
    }

    fn get(&self, identity: &str) -> Option<&OutboxOperation> {
        self.positions
            .get(identity)
            .and_then(|index| self.operations[*index].as_ref())
    }

    fn take(&mut self, identity: &str) -> Option<OutboxOperation> {
        let index = self.positions.remove(identity)?;
        self.operations[index].take()
    }

    /// 排一个操作。同一个词已经排了相反的操作时，两者都还没送出，直接抵消；排了相同的操作时用新的替换（例如换了权重）。
    fn queue(&mut self, op: OutboxKind, collection: Uuid, word: PersonalWord) {
        let identity = word.identity();
        if let Some(index) = self.positions.get(&identity).copied() {
            let cancels = self.operations[index]
                .as_ref()
                .is_some_and(|existing| existing.op != op);
            if cancels {
                self.take(&identity);
            } else {
                self.operations[index] = Some(OutboxOperation {
                    op,
                    collection,
                    word,
                });
            }
            return;
        }
        self.positions.insert(identity, self.operations.len());
        self.operations.push(Some(OutboxOperation {
            op,
            collection,
            word,
        }));
    }

    fn pending(&self) -> BTreeMap<Uuid, usize> {
        let mut counts = BTreeMap::new();
        for operation in self.operations() {
            *counts.entry(operation.collection).or_default() += 1;
        }
        counts
    }
}

/// 锁内读到的全部状态，以及这次改了哪些集合文件。
struct State {
    index: CollectionIndex,
    entries: HashMap<Uuid, Vec<PersonalWord>>,
    outbox: Outbox,
    dirty: HashSet<Uuid>,
    deleted: Vec<Uuid>,
}

impl State {
    fn view(&self) -> DictionaryCollectionsView {
        let pending = self.outbox.pending();
        DictionaryCollectionsView {
            collections: self
                .index
                .collections
                .iter()
                .map(|metadata| CollectionSummary {
                    id: metadata.id,
                    name: metadata.name.clone(),
                    kind: metadata.kind,
                    source: metadata.source.clone(),
                    enabled: metadata.enabled,
                    entry_count: metadata.entry_count,
                    pending: pending.get(&metadata.id).copied().unwrap_or(0),
                })
                .collect(),
            formats: IMPORT_FORMATS,
            import: None,
            sent: None,
        }
    }

    fn position(&self, id: Uuid) -> Result<usize> {
        self.index
            .collections
            .iter()
            .position(|metadata| metadata.id == id)
            .ok_or(DictionaryCollectionsError::NotFound)
    }

    fn metadata(&self, id: Uuid) -> Result<&CollectionMetadata> {
        let index = self.position(id)?;
        Ok(&self.index.collections[index])
    }

    fn metadata_mut(&mut self, id: Uuid) -> Result<&mut CollectionMetadata> {
        let index = self.position(id)?;
        Ok(&mut self.index.collections[index])
    }

    fn entries(&self, id: Uuid) -> &[PersonalWord] {
        self.entries.get(&id).map(Vec::as_slice).unwrap_or_default()
    }

    /// 除 `id` 以外所有已启用集合里的词条身份。
    fn identities_elsewhere(&self, id: Uuid) -> HashSet<String> {
        self.index
            .collections
            .iter()
            .filter(|metadata| metadata.enabled && metadata.id != id)
            .flat_map(|metadata| self.entries(metadata.id))
            .map(PersonalWord::identity)
            .collect()
    }

    /// 新建一个启用的集合。
    fn insert(
        &mut self,
        name: &str,
        kind: PersonalWordKind,
        source: CollectionSource,
        words: Vec<PersonalWord>,
    ) -> Result<()> {
        if self.index.collections.len() >= MAX_COLLECTIONS {
            return Err(DictionaryCollectionsError::Limit);
        }
        let id = Uuid::new_v4();
        self.index.collections.push(CollectionMetadata {
            id,
            name: name.to_owned(),
            kind,
            source,
            enabled: true,
            entry_count: 0,
        });
        self.entries.insert(id, Vec::new());
        self.dirty.insert(id);
        self.append(id, words)
    }

    /// 把还没有的词加进集合（已有的同一个词跳过）；集合启用时把不在其他已启用集合里的新词排进待发送队列。
    fn append(&mut self, id: Uuid, words: Vec<PersonalWord>) -> Result<()> {
        let enabled = self.metadata(id)?.enabled;
        let elsewhere = if enabled {
            self.identities_elsewhere(id)
        } else {
            HashSet::new()
        };
        let entries = self.entries.entry(id).or_default();
        let mut identities: HashSet<String> = entries.iter().map(PersonalWord::identity).collect();
        for word in words {
            if !identities.insert(word.identity()) {
                continue;
            }
            if entries.len() >= MAX_COLLECTION_ENTRIES {
                return Err(DictionaryCollectionsError::Limit);
            }
            if enabled && !elsewhere.contains(&word.identity()) {
                self.outbox.queue(OutboxKind::Add, id, word.clone());
            }
            entries.push(word);
        }
        let count = entries.len();
        self.metadata_mut(id)?.entry_count = count;
        self.dirty.insert(id);
        Ok(())
    }

    fn remove_identities(&mut self, id: Uuid, identities: &HashSet<String>) -> Result<()> {
        let enabled = self.metadata(id)?.enabled;
        let elsewhere = if enabled {
            self.identities_elsewhere(id)
        } else {
            HashSet::new()
        };
        let entries = self.entries.entry(id).or_default();
        let mut removed = Vec::new();
        entries.retain(|word| {
            if identities.contains(&word.identity()) {
                removed.push(word.clone());
                false
            } else {
                true
            }
        });
        let count = entries.len();
        if enabled {
            for word in removed {
                if !elsewhere.contains(&word.identity()) {
                    self.outbox.queue(OutboxKind::Remove, id, word);
                }
            }
        }
        self.metadata_mut(id)?.entry_count = count;
        self.dirty.insert(id);
        Ok(())
    }

    /// 启用：集合里不在其他已启用集合中的词排「加入」，已排的「删除」随之抵消。
    fn enable(&mut self, id: Uuid) -> Result<()> {
        let elsewhere = self.identities_elsewhere(id);
        let words: Vec<PersonalWord> = self.entries(id).to_vec();
        for word in words {
            if !elsewhere.contains(&word.identity()) {
                self.outbox.queue(OutboxKind::Add, id, word);
            }
        }
        Ok(())
    }

    /// 停用：只删不属于其他已启用集合的词，还没送出的「加入」随之抵消。
    fn disable(&mut self, id: Uuid) -> Result<()> {
        let elsewhere = self.identities_elsewhere(id);
        let words: Vec<PersonalWord> = self.entries(id).to_vec();
        for word in words {
            if !elsewhere.contains(&word.identity()) {
                self.outbox.queue(OutboxKind::Remove, id, word);
            }
        }
        Ok(())
    }
}

/// 解析一份导入文本，最多收 `room` 条，跳过 `existing` 里已有的和文件里重复的词。
fn parse_import(
    kind: PersonalWordKind,
    format: &str,
    text: &str,
    readings: Option<&dyn HanReadings>,
    existing: &HashSet<String>,
    room: usize,
) -> Result<(Vec<PersonalWord>, CollectionImportReport)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if text.is_empty() {
        return Err(DictionaryCollectionsError::Import(ImportError::Empty));
    }
    if text.len() > MAX_IMPORT_TEXT_BYTES {
        return Err(DictionaryCollectionsError::Import(ImportError::TooLarge));
    }
    if crate::text::has_disallowed_control(text) {
        return Err(DictionaryCollectionsError::Import(
            ImportError::ControlCharacters,
        ));
    }
    if format == "hans" {
        if kind != PersonalWordKind::Pinyin {
            return Err(DictionaryCollectionsError::Invalid);
        }
        let readings = readings.ok_or(DictionaryCollectionsError::UnsupportedFormat)?;
        return Ok(parse_hans(text, readings, existing, room));
    }
    let requested = match format {
        "txt" | "standard" => ImportFormat::Standard,
        "windows" => ImportFormat::Windows,
        "rime" => ImportFormat::Rime,
        _ => return Err(DictionaryCollectionsError::UnsupportedFormat),
    };
    // 读出了词条、全是已有的词或者集合已满，都说明列的顺序是对的。
    let usable = |parsed: &(Vec<PersonalWord>, CollectionImportReport)| {
        !parsed.0.is_empty() || parsed.1.duplicates > 0 || parsed.1.truncated
    };
    let parsed = parse_rows(kind, requested, text, existing, room);
    if usable(&parsed) {
        return Ok(parsed);
    }
    // 按要求的列顺序一条也读不出时换另一种顺序再读一次，见 `import::ImportReport::swapped`。
    let flipped = match requested {
        ImportFormat::Standard => Some(ImportFormat::Windows),
        ImportFormat::Windows => Some(ImportFormat::Standard),
        ImportFormat::Rime => None,
    };
    if let Some(other) = flipped {
        let mut retry = parse_rows(kind, other, text, existing, room);
        if usable(&retry) {
            retry.1.swapped = true;
            return Ok(retry);
        }
    }
    if parsed.1.failed == 0 {
        return Err(DictionaryCollectionsError::Import(
            ImportError::NoUsableRows,
        ));
    }
    Ok(parsed)
}

fn record_failure(report: &mut CollectionImportReport, failure: ImportFailure) {
    report.failed += 1;
    if report.first_failures.len() < import::REPORTED_FAILURES {
        report.first_failures.push(failure);
    }
}

/// 逐行交给导入解析器，这样一份文件不受它每次 1000 行的上限限制，行号也从整份文本开头数起。
fn parse_rows(
    kind: PersonalWordKind,
    format: ImportFormat,
    text: &str,
    existing: &HashSet<String>,
    room: usize,
) -> (Vec<PersonalWord>, CollectionImportReport) {
    let mut words = Vec::new();
    let mut report = CollectionImportReport::default();
    let mut seen = HashSet::new();
    let mut in_yaml_header = false;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if format == ImportFormat::Rime {
            // Rime 的 `dict.yaml` 以 `---` 开始、`...` 结束的头部不是词条，和导入解析器的规则相同。
            if trimmed == "---" {
                in_yaml_header = true;
                continue;
            }
            if trimmed == "..." {
                in_yaml_header = false;
                continue;
            }
            if in_yaml_header {
                continue;
            }
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parsed = import::parse_rows(import_kind(kind), format, line);
        for failure in parsed.first_failures {
            record_failure(
                &mut report,
                ImportFailure {
                    line: index + 1,
                    issue: failure.issue,
                },
            );
        }
        for entry in parsed.entries {
            let word = PersonalWord {
                kind,
                key: entry.key,
                value: entry.value,
                weight: entry.weight,
            };
            if word.validate_new().is_err() || word.value.len() > MAX_VALUE_BYTES {
                record_failure(
                    &mut report,
                    ImportFailure {
                        line: index + 1,
                        issue: import::ImportIssue::Rejected,
                    },
                );
                continue;
            }
            let identity = word.identity();
            if existing.contains(&identity) || !seen.insert(identity) {
                report.duplicates += 1;
                continue;
            }
            if words.len() >= room {
                report.truncated = true;
                return (words, report);
            }
            words.push(word);
        }
    }
    (words, report)
}

fn parse_hans(
    text: &str,
    readings: &dyn HanReadings,
    existing: &HashSet<String>,
    room: usize,
) -> (Vec<PersonalWord>, CollectionImportReport) {
    let mut words = Vec::new();
    let mut report = CollectionImportReport::default();
    let mut seen = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let value = line.trim();
        if value.is_empty() || value.starts_with('#') {
            continue;
        }
        let failure = |issue| ImportFailure {
            line: index + 1,
            issue,
        };
        if value.len() > MAX_VALUE_BYTES || !value.chars().all(super::is_han_character) {
            record_failure(&mut report, failure(import::ImportIssue::ValueTooLong));
            continue;
        }
        let Some(key) = readings.pinyin(value) else {
            record_failure(&mut report, failure(import::ImportIssue::Pinyin));
            continue;
        };
        let word = PersonalWord {
            kind: PersonalWordKind::Pinyin,
            key,
            value: value.to_owned(),
            weight: 10_000,
        };
        if word.validate_new().is_err() {
            record_failure(&mut report, failure(import::ImportIssue::Rejected));
            continue;
        }
        let identity = word.identity();
        if existing.contains(&identity) || !seen.insert(identity) {
            report.duplicates += 1;
            continue;
        }
        if words.len() >= room {
            report.truncated = true;
            break;
        }
        words.push(word);
    }
    (words, report)
}

#[cfg(test)]
mod tests;
