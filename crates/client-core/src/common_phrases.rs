//! 无编码常用语：键盘「常用语」面板里点一下就发送的多行文本。
//!
//! 它不复用快捷短语（quick_phrase）：快捷短语的每一条都必须带 1–32 个小写字母的编码，K 模式短语表只能是单行，而这里的常用语没有编码、可以多行。硬塞一个自动生成的编码，会让 `cyy1` 这类编码冒进 K 模式和快捷短语的候选。
//!
//! 文件是 `<preferences_directory>/CommonPhrases.json`，旁边的 `CommonPhrases.json.lock` 是它的文件锁。宿主进程和 Android 的 `:ime` 进程共用这个目录（和 `CommunityLibrary.json` 一样），所以每次操作都在锁里读、改、原子写回，并返回整份文档。文件损坏时如实报错，绝不悄悄清空。

use crate::community::resource::{validate_resource, CommunityResource, CommunityResourceKind};
use crate::file_lock;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// 常用语文件在偏好目录里的文件名。
pub const FILE_NAME: &str = "CommonPhrases.json";
/// 用户自己添加的常用语最多的条数。
pub const MAX_OWN_PHRASES: usize = 200;
/// 每条常用语最多的 UTF-16 单元数（编辑器和 Java 都按 UTF-16 计长度）。
pub const MAX_PHRASE_UTF16: usize = 1_000;
/// 最多安装的社区短语包个数。
pub const MAX_PACKS: usize = 16;
/// 每个社区短语包最多保留的条数。
pub const MAX_PACK_PHRASES: usize = 200;
/// 整个文件的字节上限。
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// 一条常用语。`pack` 为空表示用户自己添加的，否则是它来自的社区短语包的 id。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommonPhrase {
    pub id: Uuid,
    pub text: String,
    pub pack: Option<Uuid>,
}

/// 一个已安装的社区短语包：id 就是社区资源的 id，`revision` 是安装时的资源版本。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommonPhrasePack {
    pub id: Uuid,
    pub name: String,
    pub revision: u32,
}

/// 整份常用语文档，`phrases` 的顺序就是面板里的顺序。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommonPhrases {
    pub phrases: Vec<CommonPhrase>,
    pub packs: Vec<CommonPhrasePack>,
}

/// 一次操作，形状与 C ABI 的 `action` 相同（`operation` 区分种类）。
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommonPhrasesAction {
    Load,
    Add { text: String },
    Remove { id: Uuid },
    Replace { id: Uuid, text: String },
    Move { id: Uuid, index: usize },
    InstallPack { resource: Box<CommunityResource> },
    RemovePack { id: Uuid },
}

/// 一次操作的结果：整份文档，以及安装短语包时因为超过本地长度上限或与包内已有文本重复而没有收下的条数。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CommonPhrasesOutcome {
    #[serde(flatten)]
    pub document: CommonPhrases,
    #[serde(skip_serializing_if = "is_zero")]
    pub skipped: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[derive(Debug, Error)]
pub enum CommonPhrasesError {
    #[error("common phrases storage failed")]
    Io(#[from] std::io::Error),
    #[error("common phrases file is malformed; it was left unchanged")]
    Corrupt,
    #[error("common phrase is invalid")]
    Invalid,
    #[error("common phrase already exists in the same source")]
    Duplicate,
    #[error("common phrase limit reached")]
    Limit,
    #[error("common phrases file would exceed its size limit")]
    TooLarge,
    #[error("common phrase or pack was not found")]
    NotFound,
}

impl CommonPhrasesError {
    /// 交给宿主的稳定错误码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "common_phrases_io",
            Self::Corrupt => "common_phrases_corrupt",
            Self::Invalid => "common_phrases_invalid",
            Self::Duplicate => "common_phrases_duplicate",
            Self::Limit => "common_phrases_limit",
            Self::TooLarge => "common_phrases_too_large",
            Self::NotFound => "common_phrases_not_found",
        }
    }
}

/// 常用语正文是否可以保存：1–1000 个 UTF-16 单元，不全是空白，除换行 `\n` 以外不含控制字符。
pub fn valid_phrase_text(text: &str) -> bool {
    !text.trim().is_empty()
        && crate::text::is_bounded_utf16(text, MAX_PHRASE_UTF16)
        && !crate::has_disallowed_control_with_allowed(text, &['\n'])
}

/// 社区短语的换行可能是 `\r\n` 或 `\r`；本地只存 `\n`。
fn normalize_line_breaks(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[derive(Clone, Debug)]
pub struct CommonPhrasesStore {
    file: PathBuf,
}

impl CommonPhrasesStore {
    /// `directory` 是偏好目录，文件放在它下面的 [`FILE_NAME`]。
    pub fn new(directory: impl AsRef<Path>) -> Self {
        Self {
            file: directory.as_ref().join(FILE_NAME),
        }
    }

    pub fn perform(
        &self,
        action: CommonPhrasesAction,
    ) -> Result<CommonPhrasesOutcome, CommonPhrasesError> {
        match action {
            CommonPhrasesAction::Load => self.load().map(Self::outcome),
            CommonPhrasesAction::Add { text } => self.add(&text).map(Self::outcome),
            CommonPhrasesAction::Remove { id } => self.remove(id).map(Self::outcome),
            CommonPhrasesAction::Replace { id, text } => self.replace(id, &text).map(Self::outcome),
            CommonPhrasesAction::Move { id, index } => self.move_to(id, index).map(Self::outcome),
            CommonPhrasesAction::InstallPack { resource } => self.install_pack(&resource),
            CommonPhrasesAction::RemovePack { id } => self.remove_pack(id).map(Self::outcome),
        }
    }

    fn outcome(document: CommonPhrases) -> CommonPhrasesOutcome {
        CommonPhrasesOutcome {
            document,
            skipped: 0,
        }
    }

    /// 首次运行时文件不存在，返回空文档。
    pub fn load(&self) -> Result<CommonPhrases, CommonPhrasesError> {
        let _lock = self.lock()?;
        self.read_locked()
    }

    /// 在末尾加一条自己的常用语。与自己已有的某条完全相同时拒绝。
    pub fn add(&self, text: &str) -> Result<CommonPhrases, CommonPhrasesError> {
        if !valid_phrase_text(text) {
            return Err(CommonPhrasesError::Invalid);
        }
        self.update(|document| {
            let own = document
                .phrases
                .iter()
                .filter(|phrase| phrase.pack.is_none());
            let mut count = 0;
            for phrase in own {
                if phrase.text == text {
                    return Err(CommonPhrasesError::Duplicate);
                }
                count += 1;
            }
            if count >= MAX_OWN_PHRASES {
                return Err(CommonPhrasesError::Limit);
            }
            document.phrases.push(CommonPhrase {
                id: Uuid::new_v4(),
                text: text.to_owned(),
                pack: None,
            });
            Ok(())
        })
    }

    /// 删除一条，不论它是自己加的还是来自短语包。
    pub fn remove(&self, id: Uuid) -> Result<CommonPhrases, CommonPhrasesError> {
        self.update(|document| {
            let index = position(document, id)?;
            document.phrases.remove(index);
            Ok(())
        })
    }

    /// 改写一条的正文，它仍属于原来的来源；与同一来源里的另一条相同时拒绝。
    pub fn replace(&self, id: Uuid, text: &str) -> Result<CommonPhrases, CommonPhrasesError> {
        if !valid_phrase_text(text) {
            return Err(CommonPhrasesError::Invalid);
        }
        self.update(|document| {
            let index = position(document, id)?;
            let pack = document.phrases[index].pack;
            let existing: HashSet<(Option<Uuid>, &str)> = document
                .phrases
                .iter()
                .filter(|phrase| phrase.id != id)
                .map(|phrase| (phrase.pack, phrase.text.as_str()))
                .collect();
            if existing.contains(&(pack, text)) {
                return Err(CommonPhrasesError::Duplicate);
            }
            document.phrases[index].text = text.to_owned();
            Ok(())
        })
    }

    /// 把一条移到 `index`（移动之后它在整份列表里的位置）。
    pub fn move_to(&self, id: Uuid, index: usize) -> Result<CommonPhrases, CommonPhrasesError> {
        self.update(|document| {
            let from = position(document, id)?;
            if index >= document.phrases.len() {
                return Err(CommonPhrasesError::Invalid);
            }
            let phrase = document.phrases.remove(from);
            document.phrases.insert(index, phrase);
            Ok(())
        })
    }

    /// 安装或更新一个社区短语包。同一个包再装一次时，用新内容替换它原有的全部条目，位置保持在原来第一条所在处；新包追加在末尾。超过本地 1000 个 UTF-16 单元的条目和包内重复的文本不收，计入 `skipped`；一条都收不下时拒绝。
    pub fn install_pack(
        &self,
        resource: &CommunityResource,
    ) -> Result<CommonPhrasesOutcome, CommonPhrasesError> {
        if resource.kind != CommunityResourceKind::Phrase || validate_resource(resource).is_err() {
            return Err(CommonPhrasesError::Invalid);
        }
        let mut skipped = 0;
        let mut normalized = Vec::with_capacity(resource.content.phrases.len());
        for phrase in &resource.content.phrases {
            let text = normalize_line_breaks(&phrase.text);
            if !valid_phrase_text(&text) {
                skipped += 1;
                continue;
            }
            normalized.push(text);
        }
        // Borrow normalized text while finding first occurrences, then move only the accepted
        // strings into the pack after releasing the set.
        let mut seen = HashSet::with_capacity(normalized.len());
        let unique = normalized
            .iter()
            .map(|text| seen.insert(text.as_str()))
            .collect::<Vec<_>>();
        drop(seen);
        let mut texts = Vec::with_capacity(normalized.len().min(MAX_PACK_PHRASES));
        for (text, unique) in normalized.into_iter().zip(unique) {
            if !unique || texts.len() >= MAX_PACK_PHRASES {
                skipped += 1;
            } else {
                texts.push(text);
            }
        }
        if texts.is_empty() {
            return Err(CommonPhrasesError::Invalid);
        }
        let pack = CommonPhrasePack {
            id: resource.id,
            name: resource.name.clone(),
            revision: resource.revision,
        };
        let document = self.update(|document| {
            let existing = document.packs.iter().position(|item| item.id == pack.id);
            let insert_at = match existing {
                Some(index) => {
                    document.packs[index] = pack.clone();
                    let first = document
                        .phrases
                        .iter()
                        .position(|phrase| phrase.pack == Some(pack.id));
                    document
                        .phrases
                        .retain(|phrase| phrase.pack != Some(pack.id));
                    first.unwrap_or(document.phrases.len())
                }
                None => {
                    if document.packs.len() >= MAX_PACKS {
                        return Err(CommonPhrasesError::Limit);
                    }
                    document.packs.push(pack.clone());
                    document.phrases.len()
                }
            };
            let entries = texts.iter().map(|text| CommonPhrase {
                id: Uuid::new_v4(),
                text: text.clone(),
                pack: Some(pack.id),
            });
            document.phrases.splice(insert_at..insert_at, entries);
            Ok(())
        })?;
        Ok(CommonPhrasesOutcome { document, skipped })
    }

    /// 卸载一个短语包，连同来自它的所有条目。
    pub fn remove_pack(&self, id: Uuid) -> Result<CommonPhrases, CommonPhrasesError> {
        self.update(|document| {
            let index = document
                .packs
                .iter()
                .position(|pack| pack.id == id)
                .ok_or(CommonPhrasesError::NotFound)?;
            document.packs.remove(index);
            document.phrases.retain(|phrase| phrase.pack != Some(id));
            Ok(())
        })
    }

    fn update(
        &self,
        change: impl FnOnce(&mut CommonPhrases) -> Result<(), CommonPhrasesError>,
    ) -> Result<CommonPhrases, CommonPhrasesError> {
        let _lock = self.lock()?;
        let mut document = self.read_locked()?;
        change(&mut document)?;
        validate(&document).map_err(|_| CommonPhrasesError::Invalid)?;
        self.write_locked(&document)?;
        Ok(document)
    }

    fn parent(&self) -> Result<&Path, CommonPhrasesError> {
        let parent = self.file.parent().ok_or(CommonPhrasesError::Invalid)?;
        if !crate::storage::create_directory_and_check(parent)? {
            return Err(CommonPhrasesError::Invalid);
        }
        Ok(parent)
    }

    fn lock(&self) -> Result<File, CommonPhrasesError> {
        self.parent()?;
        let lock = file_lock::open_lock_file(self.file.with_extension("json.lock"))?;
        file_lock::exclusive(&lock)?;
        Ok(lock)
    }

    fn read_locked(&self) -> Result<CommonPhrases, CommonPhrasesError> {
        let metadata = match fs::symlink_metadata(&self.file) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CommonPhrases::default())
            }
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() || metadata.len() > MAX_FILE_BYTES {
            return Err(CommonPhrasesError::Corrupt);
        }
        let bytes =
            crate::bounded_io::read_bounded_file(File::open(&self.file)?, MAX_FILE_BYTES, || {
                CommonPhrasesError::Corrupt
            })?;
        let document: CommonPhrases =
            serde_json::from_slice(&bytes).map_err(|_| CommonPhrasesError::Corrupt)?;
        validate(&document).map_err(|_| CommonPhrasesError::Corrupt)?;
        Ok(document)
    }

    fn write_locked(&self, document: &CommonPhrases) -> Result<(), CommonPhrasesError> {
        let parent = self.parent()?;
        let bytes = serde_json::to_vec(document).map_err(|_| CommonPhrasesError::Invalid)?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(CommonPhrasesError::TooLarge);
        }
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.file).map_err(|error| error.error)?;
        Ok(())
    }
}

fn position(document: &CommonPhrases, id: Uuid) -> Result<usize, CommonPhrasesError> {
    document
        .phrases
        .iter()
        .position(|phrase| phrase.id == id)
        .ok_or(CommonPhrasesError::NotFound)
}

/// 整份文档的不变量：id 唯一且非空、正文合法、每条的包都已安装、各来源的条数在上限内且同一来源内没有重复文本。读文件和写文件之前都检查一遍。
fn validate(document: &CommonPhrases) -> Result<(), ()> {
    if document.packs.len() > MAX_PACKS {
        return Err(());
    }
    let mut pack_ids = HashSet::with_capacity(document.packs.len());
    for pack in &document.packs {
        if pack.id.is_nil()
            || pack.revision == 0
            || !crate::community::valid_name(&pack.name)
            || !pack_ids.insert(pack.id)
        {
            return Err(());
        }
    }
    let mut ids = HashSet::with_capacity(document.phrases.len());
    let mut texts = HashSet::with_capacity(document.phrases.len());
    let mut own = 0;
    let mut per_pack = std::collections::BTreeMap::<Uuid, usize>::new();
    for phrase in &document.phrases {
        if phrase.id.is_nil()
            || !ids.insert(phrase.id)
            || !valid_phrase_text(&phrase.text)
            || !texts.insert((phrase.pack, phrase.text.as_str()))
        {
            return Err(());
        }
        match phrase.pack {
            None => own += 1,
            Some(pack) => {
                if !pack_ids.contains(&pack) {
                    return Err(());
                }
                *per_pack.entry(pack).or_default() += 1;
            }
        }
    }
    if own > MAX_OWN_PHRASES || per_pack.values().any(|count| *count > MAX_PACK_PHRASES) {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
