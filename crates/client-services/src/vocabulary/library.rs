//! The wordbooks available to review, on disk.
//!
//! One file per book under `<directory>/vocabulary-wordbooks/`: a book runs to a few megabytes, and importing one must not rewrite the others.
//!
//! Beside them is a small `index.json` naming each book and its size. Listing the library is what
//! the settings page does on open, and reading five multi-megabyte documents to count their rows
//! would make that the slowest thing on the page. The index is written inside the same lock as the
//! book it describes, and [`WordbookLibrary::list`] drops any entry whose file has gone — the
//! index is a cache of the directory, never the authority on it.
//!
//! 生词本（[`PERSONAL_ID`]）也放在这里，是同一目录、同一把锁下的一本书，区别是它可以逐词写入：打字时在候选窗里把一条释义加进来，或者从导出的文本导回来。它只存在本机，不进任何同步。

use super::import::{self as wordbook_import, WordbookImportFailure};
use super::wordbook::{self, Wordbook, WordbookEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The most books one library may hold.
const MAX_BOOKS: usize = 32;
/// The largest single book that will be read. Twenty thousand entries with a long gloss each.
pub(crate) const MAX_BOOK_BYTES: u64 = 8 * 1024 * 1024;
/// The largest index that will be read. Thirty-two short records.
const MAX_INDEX_BYTES: u64 = 64 * 1024;
/// The directory, relative to the host-supplied application data directory.
const DIRECTORY: &str = "vocabulary-wordbooks";

/// 生词本的固定 id。导入的书是 `user-…`、内置书是 [`super::builtin::ORDER`] 里的名字、插件词书是 `pack-…`，都不会和它重名。
pub const PERSONAL_ID: &str = "personal";
/// 生词本在书目里显示的名字。
pub const PERSONAL_NAME: &str = "生词本";
/// 生词本最多收多少词。
///
/// 每加一个词都要在锁内重写整本书，而这件事发生在打字的时候。五千条每个字段都写满、每字按三个字节算约 6 MB，仍在单本书的上限 `MAX_BOOK_BYTES` 之内；实际的释义远短于上限，五千条通常只有几百 KB，宿主一次写入在毫秒级。真想背更多的人可以把导出的文本当成普通词书导入，那条路的上限是 [`wordbook::MAX_ENTRIES`]。
pub const MAX_PERSONAL_ENTRIES: usize = 5_000;
/// 导入生词本时文本的字节上限，和导入普通词书相同。
pub const MAX_PERSONAL_IMPORT_BYTES: usize = 8 * 1024 * 1024;
/// 同一个词合并几条中文释义时用的分隔符，和词书里常见的写法一致。
const MEANING_SEPARATOR: char = '；';

#[derive(Debug, thiserror::Error)]
pub enum WordbookLibraryError {
    #[error("vocabulary wordbook storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid vocabulary wordbook document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("vocabulary wordbook is invalid")]
    InvalidWordbook,
    #[error("vocabulary wordbook is not in the library")]
    UnknownWordbook,
    #[error("the vocabulary library is full")]
    LibraryFull,
    /// 要加进生词本的词或释义不符合词书的格式（空的、太长、带控制字符）。
    #[error("the personal wordbook entry is invalid")]
    InvalidEntry,
    /// 生词本已经有 [`MAX_PERSONAL_ENTRIES`] 个词。
    #[error("the personal wordbook is full")]
    PersonalFull,
    #[error("{0}")]
    Import(#[from] wordbook_import::WordbookImportError),
}

/// What the picker needs to draw one row, without reading the book itself.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbookSummary {
    pub id: String,
    pub name: String,
    pub total: usize,
    /// A bundled book cannot be deleted. Every book is currently imported, so this is currently
    /// always false; it is the field the picker's delete affordance keys off, and bundled books
    /// will set it when they are shipped.
    pub builtin: bool,
    /// 来自单词本插件（id 是 `pack-<插件 id>`）：在插件页卸载，不能在背单词里删除。导入的书永远是 false，`index.json` 因此不变。
    #[serde(default, skip_serializing_if = "is_false")]
    pub pack: bool,
    /// 生词本（id 是 [`PERSONAL_ID`]）：宿主据此显示「移出生词本」和导出。其他书永远是 false，`index.json` 里的旧记录因此不变。
    #[serde(default, skip_serializing_if = "is_false")]
    pub personal: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryIndex {
    #[serde(default)]
    books: Vec<WordbookSummary>,
}

impl LibraryIndex {
    /// 导入的书有几本，不算生词本。
    fn imported_count(&self) -> usize {
        self.books.iter().filter(|book| !book.personal).count()
    }
}

/// 往生词本加一个词的结果。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonalAddOutcome {
    /// 新词，加在最后。
    Added,
    /// 词已经在了，这次的中文释义并进了它原有的释义。
    Merged,
    /// 词和这条释义都已经在了，什么也没改。
    AlreadyPresent,
}

/// 把一段文本导入生词本的结果。不回显任何一行内容，和导入普通词书相同。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalImportReport {
    /// 新加进来的词数。
    pub added: usize,
    /// 原来就有、并进了新释义的词数。
    pub merged: usize,
    /// 原来就有、释义也相同的词数。
    pub unchanged: usize,
    /// 生词本满了没能加进来的词数。
    pub over_limit: usize,
    /// 格式不对、被跳过的行数。
    pub failed: usize,
    pub first_failures: Vec<WordbookImportFailure>,
    /// 超出导入行数上限的部分没有读。
    pub truncated: bool,
}

/// 候选窗里一条候选的释义变成生词本里的一张卡：外文义项是词头，中文候选是释义。
///
/// `gloss` 是宿主显示的那段释义：可能有几行（每种目标语言一行，换行或 U+2028 分隔），一行里可能用 `;` 或 `；` 分出几个义项。词头取第一行的第一个义项，和单义项时 Ctrl+Enter 上屏的那一个相同；其余义项是同一个中文词的别的译法，不是这个外文词的意思，不收进卡片。`pronunciation` 是宿主随释义拿到的读音，可以为空。不符合词书格式时返回 `None`，例如 AI 译出的一整句超过了词头的长度上限。
pub fn personal_entry_from_gloss(
    candidate: &str,
    gloss: &str,
    pronunciation: &str,
) -> Option<WordbookEntry> {
    // 宿主各自的分行符：macOS 和 Windows 交给会话的释义用 U+2028 分隔各语言行，其他宿主用换行。
    let word = gloss
        .split(['\n', '\r', '\u{2028}'])
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .split([';', '；'])
        .map(str::trim)
        .find(|sense| !sense.is_empty())?;
    let entry = WordbookEntry {
        word: word.to_owned(),
        phonetic: pronunciation.trim().to_owned(),
        meaning: candidate.trim().to_owned(),
    };
    if entry.is_valid() {
        return Some(entry);
    }
    // 读音太长或者格式不对时不连累词本身，去掉读音再试一次。
    let entry = WordbookEntry {
        phonetic: String::new(),
        ..entry
    };
    entry.is_valid().then_some(entry)
}

/// 把 `incoming` 并进同一个词已有的 `existing`：释义按 [`MEANING_SEPARATOR`] 分成几项，新的一项不在里面就接在最后；原来没有读音时补上。合并后超过释义上限就只补读音。返回是否改动了什么。
fn merge_personal_entry(existing: &mut WordbookEntry, incoming: &WordbookEntry) -> bool {
    let mut changed = false;
    if existing.phonetic.is_empty() && !incoming.phonetic.is_empty() {
        existing.phonetic = incoming.phonetic.clone();
        changed = true;
    }
    let known = existing
        .meaning
        .split([MEANING_SEPARATOR, ';'])
        .map(str::trim)
        .collect::<Vec<_>>();
    let fresh = incoming
        .meaning
        .split([MEANING_SEPARATOR, ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty() && !known.contains(part))
        .collect::<Vec<_>>();
    if !fresh.is_empty() {
        let mut meaning = existing.meaning.clone();
        for part in fresh {
            meaning.push(MEANING_SEPARATOR);
            meaning.push_str(part);
        }
        if meaning.chars().count() <= wordbook::MAX_MEANING_CHARS {
            existing.meaning = meaning;
            changed = true;
        }
    }
    changed
}

/// 生词本导出成什么样的文本：一行一个词，三列 `词,读音,释义`，正是 [`wordbook_import::parse`] 读的格式，所以导出的文件既能导回生词本，也能当普通词书导入。
fn export_personal_text(book: &Wordbook) -> String {
    fn field(value: &str, out: &mut String) {
        // 含逗号、引号、首尾空白，或者以 `#` 开头（导入时会被当成注释行）的字段加引号，引号本身写两遍。
        let quoted = value.contains([',', '"']) || value.starts_with('#') || value.trim() != value;
        if quoted {
            out.push('"');
            out.push_str(&value.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(value);
        }
    }
    let mut out = String::with_capacity(32 + book.entries.len() * 32);
    out.push_str("# 水杉输入法生词本：词,读音,释义\n");
    for entry in &book.entries {
        field(&entry.word, &mut out);
        out.push(',');
        field(&entry.phonetic, &mut out);
        out.push(',');
        field(&entry.meaning, &mut out);
        out.push('\n');
    }
    out
}

/// The imported wordbooks in a host-supplied directory.
#[derive(Clone, Debug)]
pub struct WordbookLibrary {
    directory: PathBuf,
}

struct LibraryLock {
    directory: crate::file_lock::PrivateDirectory,
    _lock: crate::file_lock::ScopedFileLock,
}

impl WordbookLibrary {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into().join(DIRECTORY),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    #[cfg(all(test, unix))]
    fn book_path(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.json"))
    }

    #[cfg(test)]
    fn index_path(&self) -> PathBuf {
        self.directory.join("index.json")
    }

    #[cfg(test)]
    fn book_path_exists(&self, id: &str) -> bool {
        self.directory.join(format!("{id}.json")).exists()
    }

    fn lock(&self) -> Result<LibraryLock, WordbookLibraryError> {
        if let Some(parent) = self.directory.parent() {
            crate::storage::reject_symlink(parent)?;
        }
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(WordbookLibraryError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vocabulary wordbook directory is not a real directory",
            )));
        }
        let directory = crate::file_lock::open_private_directory(&self.directory)?;
        let lock =
            crate::file_lock::open_private_lock_file_at(&directory, OsStr::new("wordbooks.lock"))?;
        let lock = crate::file_lock::ScopedFileLock::exclusive(lock)?;
        Ok(LibraryLock {
            directory,
            _lock: lock,
        })
    }

    fn read_index_locked(&self, lock: &LibraryLock) -> Result<LibraryIndex, WordbookLibraryError> {
        let file =
            match crate::file_lock::open_private_file_at(&lock.directory, OsStr::new("index.json"))
            {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(LibraryIndex::default());
                }
                Err(_) => return Err(WordbookLibraryError::InvalidWordbook),
            };
        let bytes = crate::bounded_io::read_bounded_file(file, MAX_INDEX_BYTES, || {
            WordbookLibraryError::InvalidWordbook
        })?;
        let index: LibraryIndex = serde_json::from_slice(&bytes)?;
        // 生词本不占导入书的名额，所以索引最多比 [`MAX_BOOKS`] 多一行。
        if index.books.len() > MAX_BOOKS + 1
            || index.imported_count() > MAX_BOOKS
            || !index.books.iter().all(|book| {
                wordbook::id_is_well_formed(&book.id)
                    && !book.name.is_empty()
                    && crate::text::is_bounded_chars(&book.name, wordbook::MAX_NAME_CHARS)
                    && book.total > 0
                    && book.total <= wordbook::MAX_ENTRIES
                    && book.personal == (book.id == PERSONAL_ID)
                    && (!book.personal || book.total <= MAX_PERSONAL_ENTRIES)
            })
        {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        Ok(index)
    }

    fn write_atomically(
        &self,
        lock: &LibraryLock,
        name: &OsStr,
        bytes: &[u8],
    ) -> Result<(), WordbookLibraryError> {
        crate::file_lock::write_private_file_at(&lock.directory, name, bytes)
            .map_err(WordbookLibraryError::Io)
    }

    /// Every book the library holds, in the order they were imported.
    ///
    /// An index entry whose file has gone is dropped rather than reported: the book is genuinely
    /// not there, and a picker row that fails to open is worse than a row that is absent.
    pub fn list(&self) -> Result<Vec<WordbookSummary>, WordbookLibraryError> {
        let lock = self.lock()?;
        let index = self.read_index_locked(&lock)?;
        let mut books = Vec::with_capacity(index.books.len());
        books.extend(index.books.into_iter().filter(|book| {
            let name = format!("{}.json", book.id);
            crate::file_lock::open_private_file_at(&lock.directory, OsStr::new(&name)).is_ok()
        }));
        Ok(books)
    }

    /// One book in full. `Ok(None)` when the library does not have it.
    pub fn load(&self, id: &str) -> Result<Option<Wordbook>, WordbookLibraryError> {
        if !wordbook::id_is_well_formed(id) {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        let lock = self.lock()?;
        self.load_locked(&lock, id)
    }

    /// [`Self::load`]，调用方已经拿着锁。
    fn load_locked(
        &self,
        lock: &LibraryLock,
        id: &str,
    ) -> Result<Option<Wordbook>, WordbookLibraryError> {
        let name = format!("{id}.json");
        let file = match crate::file_lock::open_private_file_at(&lock.directory, OsStr::new(&name))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(WordbookLibraryError::InvalidWordbook),
        };
        let bytes = crate::bounded_io::read_bounded_file(file, MAX_BOOK_BYTES, || {
            WordbookLibraryError::InvalidWordbook
        })?;
        let book: Wordbook = serde_json::from_slice(&bytes)?;
        // A book that does not validate is reported, never silently skipped: the user imported it
        // and would otherwise see it vanish from the picker with no explanation.
        if !book.is_valid() || book.id != id {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        Ok(Some(book))
    }

    /// Store `entries` as the book `id`, called `name`. Importing an existing id replaces it.
    ///
    /// `id` is the caller's and must never be taken from the imported file. A book keys the review
    /// progress, so a file that named its own id could silently inherit — or destroy — the
    /// schedule of a book the user imported earlier. It is a parameter rather than generated here
    /// so that the schedule of a replaced book is deliberately preserved, and so these tests do
    /// not have to assert against a fresh random value.
    pub fn import(
        &self,
        name: &str,
        entries: Vec<WordbookEntry>,
        id: &str,
    ) -> Result<Wordbook, WordbookLibraryError> {
        let book = Wordbook {
            id: id.to_owned(),
            name: name.to_owned(),
            entries,
        };
        // 生词本只能逐词写入，整本替换会把它变成一本普通的书。
        if !book.is_valid() || book.id == PERSONAL_ID {
            return Err(WordbookLibraryError::InvalidWordbook);
        }

        let lock = self.lock()?;
        let mut index = self.read_index_locked(&lock)?;
        if !index.books.iter().any(|entry| entry.id == book.id)
            && index.imported_count() >= MAX_BOOKS
        {
            return Err(WordbookLibraryError::LibraryFull);
        }

        let bytes = serde_json::to_vec(&book)?;
        if bytes.len() as u64 > MAX_BOOK_BYTES {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        // The book first, then the index. A crash between the two leaves a file no index names,
        // which is invisible and harmless; the other order would leave a picker row that opens
        // nothing.
        let book_name = format!("{}.json", book.id);
        self.write_atomically(&lock, OsStr::new(&book_name), &bytes)?;

        let summary = WordbookSummary {
            id: book.id.clone(),
            name: book.name.clone(),
            total: book.entries.len(),
            builtin: false,
            pack: false,
            personal: false,
        };
        match index.books.iter_mut().find(|entry| entry.id == summary.id) {
            Some(existing) => *existing = summary,
            None => index.books.push(summary),
        }
        self.write_atomically(
            &lock,
            OsStr::new("index.json"),
            &serde_json::to_vec(&index)?,
        )?;
        Ok(book)
    }

    /// Forget one book. The caller is responsible for its review progress.
    pub fn remove(&self, id: &str) -> Result<(), WordbookLibraryError> {
        if !wordbook::id_is_well_formed(id) {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        let lock = self.lock()?;
        let mut index = self.read_index_locked(&lock)?;
        let before = index.books.len();
        index.books.retain(|entry| entry.id != id);
        if index.books.len() == before
            && crate::file_lock::open_private_file_at(
                &lock.directory,
                OsStr::new(&format!("{id}.json")),
            )
            .is_err()
        {
            return Err(WordbookLibraryError::UnknownWordbook);
        }
        // The index first this time, so a crash between the two leaves an orphan file rather than
        // a row pointing at a deleted book.
        self.write_atomically(
            &lock,
            OsStr::new("index.json"),
            &serde_json::to_vec(&index)?,
        )?;
        let name = format!("{id}.json");
        let removed = crate::file_lock::remove_private_file_at(&lock.directory, OsStr::new(&name));
        match removed {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// 生词本现在的词，按加入的先后。还没有生词本时是空的。
    fn personal_entries_locked(
        &self,
        lock: &LibraryLock,
    ) -> Result<Vec<WordbookEntry>, WordbookLibraryError> {
        Ok(self
            .load_locked(lock, PERSONAL_ID)?
            .map(|book| book.entries)
            .unwrap_or_default())
    }

    /// 把生词本写成 `entries`。一个词都不剩时连文件带索引那一行一起删掉：词书至少要有一个词，空的生词本就是没有生词本。
    fn store_personal_locked(
        &self,
        lock: &LibraryLock,
        entries: Vec<WordbookEntry>,
    ) -> Result<(), WordbookLibraryError> {
        let mut index = self.read_index_locked(lock)?;
        let file_name = format!("{PERSONAL_ID}.json");
        if entries.is_empty() {
            // 和 `remove` 一样先写索引，中途崩溃只留下一个没有索引指向的文件。
            index.books.retain(|book| book.id != PERSONAL_ID);
            self.write_atomically(lock, OsStr::new("index.json"), &serde_json::to_vec(&index)?)?;
            return match crate::file_lock::remove_private_file_at(
                &lock.directory,
                OsStr::new(&file_name),
            ) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
                _ => Ok(()),
            };
        }
        let book = Wordbook {
            id: PERSONAL_ID.to_owned(),
            name: PERSONAL_NAME.to_owned(),
            entries,
        };
        if !book.is_valid() || book.entries.len() > MAX_PERSONAL_ENTRIES {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        let bytes = serde_json::to_vec(&book)?;
        if bytes.len() as u64 > MAX_BOOK_BYTES {
            return Err(WordbookLibraryError::InvalidWordbook);
        }
        // 先写书再写索引，理由同 `import`。
        self.write_atomically(lock, OsStr::new(&file_name), &bytes)?;
        let summary = WordbookSummary {
            id: book.id,
            name: book.name,
            total: book.entries.len(),
            builtin: false,
            pack: false,
            personal: true,
        };
        match index.books.iter_mut().find(|entry| entry.personal) {
            Some(existing) => *existing = summary,
            // 第一次建生词本时排在导入的书前面：它是天天往里加词的那一本，不该被几十本导入的书压到最后。
            None => index.books.insert(0, summary),
        }
        self.write_atomically(lock, OsStr::new("index.json"), &serde_json::to_vec(&index)?)
    }

    /// 把一个词加进生词本，生词本不存在时建出来。
    ///
    /// 同一个词已经在了就把释义并进去，不另起一行：卡片按词头记复习进度，同一个词两行就是一张卡两份释义。读、改、写都在词书目录的锁里，候选窗和背单词页同时写也不会互相覆盖。
    pub fn add_personal(
        &self,
        entry: WordbookEntry,
    ) -> Result<PersonalAddOutcome, WordbookLibraryError> {
        if !entry.is_valid() {
            return Err(WordbookLibraryError::InvalidEntry);
        }
        let lock = self.lock()?;
        let mut entries = self.personal_entries_locked(&lock)?;
        let outcome = match entries
            .iter()
            .position(|existing| existing.word == entry.word)
        {
            Some(position) => {
                if !merge_personal_entry(&mut entries[position], &entry) {
                    return Ok(PersonalAddOutcome::AlreadyPresent);
                }
                PersonalAddOutcome::Merged
            }
            None if entries.len() >= MAX_PERSONAL_ENTRIES => {
                return Err(WordbookLibraryError::PersonalFull);
            }
            None => {
                entries.push(entry);
                PersonalAddOutcome::Added
            }
        };
        self.store_personal_locked(&lock, entries)?;
        Ok(outcome)
    }

    /// 把 `word` 移出生词本。词本来就不在时返回 `false`。移走最后一个词时生词本随之删除。
    ///
    /// 这个词的复习进度留着：同一个词以后再加回来，接着原来的排程，不必从头背。
    pub fn remove_personal_entry(&self, word: &str) -> Result<bool, WordbookLibraryError> {
        if !wordbook::word_is_valid(word) {
            return Err(WordbookLibraryError::InvalidEntry);
        }
        let lock = self.lock()?;
        let mut entries = self.personal_entries_locked(&lock)?;
        let before = entries.len();
        entries.retain(|entry| entry.word != word);
        if entries.len() == before {
            return Ok(false);
        }
        self.store_personal_locked(&lock, entries)?;
        Ok(true)
    }

    /// 把与 `drained` 里某一条完全相同的词条移出生词本，返回移走了几条。词头相同但释义或读音已经变了的词条留着：那是交出去以后又改过的，还没有交出去。移走最后一个词时生词本随之删除。
    pub fn remove_personal_entries(
        &self,
        drained: &[WordbookEntry],
    ) -> Result<usize, WordbookLibraryError> {
        let lock = self.lock()?;
        let mut entries = self.personal_entries_locked(&lock)?;
        let before = entries.len();
        entries.retain(|entry| !drained.contains(entry));
        let removed = before - entries.len();
        if removed > 0 {
            self.store_personal_locked(&lock, entries)?;
        }
        Ok(removed)
    }

    /// 生词本导出成的文本，格式见 [`export_personal_text`]。还没有生词本时是 `None`。
    pub fn export_personal(&self) -> Result<Option<String>, WordbookLibraryError> {
        let lock = self.lock()?;
        Ok(self
            .load_locked(&lock, PERSONAL_ID)?
            .map(|book| export_personal_text(&book)))
    }

    /// 把一段词表文本并进生词本：新词接在最后，已有的词按 [`Self::add_personal`] 的规则合并释义，生词本满了以后的新词只计数不加。整份文本只写一次。
    ///
    /// 文本的格式和导入普通词书相同（CSV 或 TSV，两列或三列），[`Self::export_personal`] 导出的文件可以原样导回来。没有一行可用时返回错误，生词本不变。
    pub fn import_personal(
        &self,
        text: &str,
    ) -> Result<PersonalImportReport, WordbookLibraryError> {
        let parsed = wordbook_import::parse(text, MAX_PERSONAL_IMPORT_BYTES)?;
        let report = PersonalImportReport {
            failed: parsed.failed,
            first_failures: parsed.first_failures,
            truncated: parsed.truncated,
            ..PersonalImportReport::default()
        };
        self.merge_personal(parsed.entries, report)
    }

    /// 把已经校验过的词条逐个并进生词本，规则同 [`Self::import_personal`]，整批只写一次。`report` 里已有的格式错误计数原样带回。
    pub fn merge_personal(
        &self,
        incoming: Vec<WordbookEntry>,
        mut report: PersonalImportReport,
    ) -> Result<PersonalImportReport, WordbookLibraryError> {
        let lock = self.lock()?;
        let mut entries = self.personal_entries_locked(&lock)?;
        let mut positions: HashMap<String, usize> = entries
            .iter()
            .enumerate()
            .map(|(position, entry)| (entry.word.clone(), position))
            .collect();
        for entry in incoming {
            if !entry.is_valid() {
                report.failed += 1;
                continue;
            }
            match positions.get(&entry.word) {
                Some(&position) => {
                    if merge_personal_entry(&mut entries[position], &entry) {
                        report.merged += 1;
                    } else {
                        report.unchanged += 1;
                    }
                }
                None if entries.len() >= MAX_PERSONAL_ENTRIES => report.over_limit += 1,
                None => {
                    positions.insert(entry.word.clone(), entries.len());
                    entries.push(entry);
                    report.added += 1;
                }
            }
        }
        if report.added > 0 || report.merged > 0 {
            self.store_personal_locked(&lock, entries)?;
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn finished_scope_releases_inherited_lock() {
        let root = tempfile::tempdir().unwrap();
        let store = WordbookLibrary::new(root.path());
        let held = store.lock().unwrap();
        let inherited = crate::file_lock::testing::duplicate(&held._lock);
        crate::file_lock::testing::assert_finished_scope_releases_lock(
            held,
            inherited,
            &store.directory.join("wordbooks.lock"),
        );
    }

    use std::fs;

    fn entries(words: &[&str]) -> Vec<WordbookEntry> {
        words
            .iter()
            .map(|word| WordbookEntry {
                word: (*word).to_owned(),
                phonetic: String::new(),
                meaning: "adj. 合成释义".to_owned(),
            })
            .collect()
    }

    fn library() -> (tempfile::TempDir, WordbookLibrary) {
        let directory = tempfile::tempdir().unwrap();
        let library = WordbookLibrary::new(directory.path());
        (directory, library)
    }

    #[test]
    fn an_empty_library_lists_nothing() {
        let (_directory, library) = library();
        assert!(library.list().unwrap().is_empty());
        assert_eq!(library.load("user-1").unwrap(), None);
    }

    #[test]
    fn an_imported_book_is_listed_and_can_be_read_back() {
        let (_directory, library) = library();
        let book = library
            .import("我的词表", entries(&["alpha", "beta"]), "user-1")
            .unwrap();
        assert_eq!(book.entries.len(), 2);

        assert_eq!(
            library.list().unwrap(),
            vec![WordbookSummary {
                id: "user-1".to_owned(),
                name: "我的词表".to_owned(),
                total: 2,
                builtin: false,
                pack: false,
                personal: false,
            }]
        );
        assert_eq!(library.load("user-1").unwrap().as_ref(), Some(&book));
    }

    #[test]
    fn importing_the_same_id_replaces_the_book_without_a_second_row() {
        let (_directory, library) = library();
        library
            .import("第一版", entries(&["alpha"]), "user-1")
            .unwrap();
        library
            .import("第二版", entries(&["alpha", "beta", "gamma"]), "user-1")
            .unwrap();

        let listed = library.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "第二版");
        assert_eq!(listed[0].total, 3);
    }

    #[test]
    fn several_books_keep_their_import_order() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        library.import("乙", entries(&["b"]), "user-2").unwrap();
        library.import("丙", entries(&["c"]), "user-3").unwrap();
        assert_eq!(
            library
                .list()
                .unwrap()
                .iter()
                .map(|book| book.id.as_str())
                .collect::<Vec<_>>(),
            vec!["user-1", "user-2", "user-3"]
        );
    }

    #[test]
    fn removing_a_book_drops_the_row_and_the_file() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        library.import("乙", entries(&["b"]), "user-2").unwrap();

        library.remove("user-1").unwrap();
        assert_eq!(
            library
                .list()
                .unwrap()
                .iter()
                .map(|book| book.id.as_str())
                .collect::<Vec<_>>(),
            vec!["user-2"]
        );
        assert_eq!(library.load("user-1").unwrap(), None);
        assert!(!library.directory().join("user-1.json").exists());

        assert!(matches!(
            library.remove("user-1"),
            Err(WordbookLibraryError::UnknownWordbook)
        ));
    }

    #[test]
    fn an_index_row_whose_file_is_gone_is_dropped_from_the_listing() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        library.import("乙", entries(&["b"]), "user-2").unwrap();
        fs::remove_file(library.directory().join("user-1.json")).unwrap();

        // A picker row that fails to open is worse than a row that is not there.
        assert_eq!(
            library
                .list()
                .unwrap()
                .iter()
                .map(|book| book.id.as_str())
                .collect::<Vec<_>>(),
            vec!["user-2"]
        );
    }

    #[test]
    fn a_damaged_book_is_reported_rather_than_silently_skipped() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        fs::write(
            library.directory().join("user-1.json"),
            b"{\"id\":\"user-1\"}",
        )
        .unwrap();

        // The user imported it; it must not vanish from the picker with no explanation.
        assert!(library.load("user-1").is_err());
    }

    #[test]
    fn an_oversized_book_is_rejected_before_loading() {
        let (_directory, library) = library();
        fs::create_dir_all(library.directory()).unwrap();
        std::fs::File::create(library.directory().join("user-1.json"))
            .unwrap()
            .set_len(MAX_BOOK_BYTES + 1)
            .unwrap();
        assert!(matches!(
            library.load("user-1"),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }

    #[test]
    fn a_book_whose_stored_id_disagrees_with_its_file_is_refused() {
        let (_directory, library) = library();
        let book = Wordbook {
            id: "user-2".to_owned(),
            name: "错位".to_owned(),
            entries: entries(&["a"]),
        };
        fs::create_dir_all(library.directory()).unwrap();
        fs::write(
            library.directory().join("user-1.json"),
            serde_json::to_vec(&book).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            library.load("user-1"),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }

    #[test]
    fn an_invalid_book_is_refused_before_anything_is_written() {
        let (_directory, library) = library();
        assert!(matches!(
            library.import("空的", Vec::new(), "user-1"),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
        assert!(matches!(
            library.import("坏 id", entries(&["a"]), "User 1"),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
        assert!(library.list().unwrap().is_empty());
    }

    #[test]
    fn the_library_refuses_a_thirty_third_book() {
        let (_directory, library) = library();
        for index in 0..MAX_BOOKS {
            library
                .import("书", entries(&["a"]), &format!("user-{index}"))
                .unwrap();
        }
        assert!(matches!(
            library.import("再来一本", entries(&["a"]), "user-extra"),
            Err(WordbookLibraryError::LibraryFull)
        ));
        // An existing book may still be replaced when the library is full.
        assert!(library.import("书", entries(&["a", "b"]), "user-0").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_library_storage_and_book() {
        use std::os::unix::fs::symlink;

        let target = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked_root = parent.path().join("user-data");
        symlink(target.path(), &linked_root).unwrap();
        let linked_library = WordbookLibrary::new(&linked_root);
        assert!(matches!(
            linked_library.list(),
            Err(WordbookLibraryError::Io(_))
        ));
        assert!(!target.path().join(DIRECTORY).exists());

        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_book = outside.path().join("book.json");
        fs::write(
            &outside_book,
            fs::read(library.book_path("user-1")).unwrap(),
        )
        .unwrap();
        fs::remove_file(library.book_path("user-1")).unwrap();
        symlink(&outside_book, library.book_path("user-1")).unwrap();
        assert!(matches!(
            library.load("user-1"),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
        assert!(library
            .list()
            .unwrap()
            .iter()
            .all(|summary| summary.id != "user-1"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_hard_linked_book_outside_the_library() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_book = outside.path().join("book.json");
        fs::write(
            &outside_book,
            fs::read(library.book_path("user-1")).unwrap(),
        )
        .unwrap();
        fs::remove_file(library.book_path("user-1")).unwrap();
        fs::hard_link(&outside_book, library.book_path("user-1")).unwrap();
        msime_path_trust::open_to_other_users(library.book_path("user-1").parent().unwrap())
            .unwrap();

        assert!(matches!(
            library.load("user-1"),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn writes_stay_bound_to_the_locked_library_directory_after_replacement() {
        let root = tempfile::tempdir().unwrap();
        let library = WordbookLibrary::new(root.path());
        fs::create_dir_all(library.directory()).unwrap();
        let lock = library.lock().unwrap();
        let moved = root.path().join("vocabulary-wordbooks-moved");
        fs::rename(library.directory(), &moved).unwrap();
        fs::create_dir(library.directory()).unwrap();

        library
            .write_atomically(&lock, std::ffi::OsStr::new("index.json"), b"{}")
            .unwrap();

        assert!(moved.join("index.json").exists());
        assert!(!library.directory().join("index.json").exists());
        fs::remove_dir_all(moved).unwrap();
    }

    #[test]
    fn rejects_malformed_index_summaries() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        fs::write(
            library.index_path(),
            br#"{"books":[{"id":"user-1","name":"","total":20001,"builtin":false}]}"#,
        )
        .unwrap();
        assert!(matches!(
            library.list(),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }

    fn personal(word: &str, meaning: &str) -> WordbookEntry {
        WordbookEntry {
            word: word.to_owned(),
            phonetic: String::new(),
            meaning: meaning.to_owned(),
        }
    }

    #[test]
    fn a_gloss_becomes_a_card_headed_by_its_first_sense() {
        let entry =
            personal_entry_from_gloss(" 面试 ", "interview; audition\nmensetsu", "/ˈɪntəvjuː/")
                .unwrap();
        assert_eq!(entry.word, "interview");
        assert_eq!(entry.meaning, "面试");
        assert_eq!(entry.phonetic, "/ˈɪntəvjuː/");
        // 全角分号和前导空行一样认。
        assert_eq!(
            personal_entry_from_gloss("苹果", "\n ；apple；fruit", "")
                .unwrap()
                .word,
            "apple"
        );
        assert_eq!(
            personal_entry_from_gloss("面试", "interview\u{2028}mensetsu", "")
                .unwrap()
                .word,
            "interview"
        );
        assert_eq!(personal_entry_from_gloss("苹果", "", ""), None);
        assert_eq!(personal_entry_from_gloss("苹果", " ; ；", ""), None);
        assert_eq!(personal_entry_from_gloss("", "apple", ""), None);
        assert_eq!(
            personal_entry_from_gloss("一句话", &"word ".repeat(20), ""),
            None,
            "a sentence translation is longer than any headword"
        );
        let long_reading = personal_entry_from_gloss("苹果", "apple", &"ə".repeat(65)).unwrap();
        assert_eq!(long_reading.word, "apple");
        assert_eq!(
            long_reading.phonetic, "",
            "an unusable reading does not lose the word"
        );
    }

    #[test]
    fn adding_creates_the_personal_book_ahead_of_imported_ones() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        assert_eq!(
            library.add_personal(personal("interview", "面试")).unwrap(),
            PersonalAddOutcome::Added
        );
        let listed = library.list().unwrap();
        assert_eq!(
            listed[0],
            WordbookSummary {
                id: PERSONAL_ID.to_owned(),
                name: PERSONAL_NAME.to_owned(),
                total: 1,
                builtin: false,
                pack: false,
                personal: true,
            }
        );
        assert_eq!(listed[1].id, "user-1");
        let book = library.load(PERSONAL_ID).unwrap().unwrap();
        assert_eq!(book.entries, vec![personal("interview", "面试")]);
    }

    #[test]
    fn adding_a_known_word_merges_its_meaning_instead_of_a_second_row() {
        let (_directory, library) = library();
        library.add_personal(personal("interview", "面试")).unwrap();
        assert_eq!(
            library.add_personal(personal("interview", "面试")).unwrap(),
            PersonalAddOutcome::AlreadyPresent
        );
        assert_eq!(
            library
                .add_personal(WordbookEntry {
                    phonetic: "/ˈɪntəvjuː/".to_owned(),
                    ..personal("interview", "面谈")
                })
                .unwrap(),
            PersonalAddOutcome::Merged
        );
        let book = library.load(PERSONAL_ID).unwrap().unwrap();
        assert_eq!(book.entries.len(), 1);
        assert_eq!(book.entries[0].meaning, "面试；面谈");
        assert_eq!(book.entries[0].phonetic, "/ˈɪntəvjuː/");
        assert_eq!(
            library.add_personal(personal("interview", "面谈")).unwrap(),
            PersonalAddOutcome::AlreadyPresent
        );
    }

    #[test]
    fn a_merge_that_would_overflow_the_meaning_keeps_the_old_one() {
        let (_directory, library) = library();
        let full = "词".repeat(wordbook::MAX_MEANING_CHARS);
        library.add_personal(personal("word", &full)).unwrap();
        assert_eq!(
            library.add_personal(personal("word", "另一个")).unwrap(),
            PersonalAddOutcome::AlreadyPresent
        );
        assert_eq!(
            library.load(PERSONAL_ID).unwrap().unwrap().entries[0].meaning,
            full
        );
    }

    #[test]
    fn an_invalid_entry_is_refused_without_creating_the_book() {
        let (_directory, library) = library();
        assert!(matches!(
            library.add_personal(personal("", "面试")),
            Err(WordbookLibraryError::InvalidEntry)
        ));
        assert!(matches!(
            library.add_personal(personal("interview", "")),
            Err(WordbookLibraryError::InvalidEntry)
        ));
        assert!(library.list().unwrap().is_empty());
    }

    fn numbered(count: usize) -> String {
        (0..count)
            .map(|index| format!("w{index},义{index}\n"))
            .collect()
    }

    #[test]
    fn the_personal_book_stops_at_its_limit_but_still_merges() {
        let (_directory, library) = library();
        let report = library
            .import_personal(&numbered(MAX_PERSONAL_ENTRIES + 3))
            .unwrap();
        assert_eq!(report.added, MAX_PERSONAL_ENTRIES);
        assert_eq!(report.over_limit, 3);
        assert_eq!(library.list().unwrap()[0].total, MAX_PERSONAL_ENTRIES);

        assert!(matches!(
            library.add_personal(personal("fresh", "新词")),
            Err(WordbookLibraryError::PersonalFull)
        ));
        assert_eq!(
            library.add_personal(personal("w0", "另义")).unwrap(),
            PersonalAddOutcome::Merged
        );
    }

    #[test]
    fn the_personal_book_does_not_take_an_imported_books_place() {
        let (_directory, library) = library();
        for index in 0..MAX_BOOKS {
            library
                .import("书", entries(&["a"]), &format!("user-{index}"))
                .unwrap();
        }
        library.add_personal(personal("interview", "面试")).unwrap();
        assert_eq!(library.list().unwrap().len(), MAX_BOOKS + 1);
        assert!(matches!(
            library.import("再来一本", entries(&["a"]), "user-extra"),
            Err(WordbookLibraryError::LibraryFull)
        ));
        // 生词本只能逐词写入，不能被整本替换。
        assert!(matches!(
            library.import("冒名", entries(&["a"]), PERSONAL_ID),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }

    #[test]
    fn removing_the_last_word_removes_the_personal_book() {
        let (_directory, library) = library();
        library.add_personal(personal("alpha", "甲")).unwrap();
        library.add_personal(personal("beta", "乙")).unwrap();
        assert!(!library.remove_personal_entry("gamma").unwrap());
        assert!(library.remove_personal_entry("alpha").unwrap());
        assert_eq!(library.list().unwrap()[0].total, 1);
        assert!(library.remove_personal_entry("beta").unwrap());
        assert!(library.list().unwrap().is_empty());
        assert!(!library.book_path_exists(PERSONAL_ID));
        assert_eq!(library.export_personal().unwrap(), None);
        // 删空以后照样能重新建。
        library.add_personal(personal("alpha", "甲")).unwrap();
        assert_eq!(library.list().unwrap().len(), 1);
    }

    #[test]
    fn an_export_reads_back_as_the_same_book() {
        let (_directory, library) = library();
        for entry in [
            WordbookEntry {
                word: "interview".to_owned(),
                phonetic: "/ˈɪntəvjuː/".to_owned(),
                meaning: "面试；面谈".to_owned(),
            },
            personal("#hashtag", "话题标签"),
            personal("say \"hi\"", "打招呼, 问好"),
            personal("rock,paper", "石头剪刀"),
        ] {
            library.add_personal(entry).unwrap();
        }
        let original = library.load(PERSONAL_ID).unwrap().unwrap();
        let text = library.export_personal().unwrap().unwrap();

        let parsed = wordbook_import::parse(&text, MAX_PERSONAL_IMPORT_BYTES).unwrap();
        assert_eq!(parsed.failed, 0);
        assert_eq!(parsed.entries, original.entries);

        let (_other, fresh) = super::tests::library();
        let report = fresh.import_personal(&text).unwrap();
        assert_eq!(report.added, original.entries.len());
        assert_eq!(
            fresh.load(PERSONAL_ID).unwrap().unwrap().entries,
            original.entries
        );

        // 导回同一个生词本什么也不改。
        let again = library.import_personal(&text).unwrap();
        assert_eq!(again.unchanged, original.entries.len());
        assert_eq!(again.added + again.merged, 0);
    }

    #[test]
    fn an_import_reports_what_it_did_without_echoing_rows() {
        let (_directory, library) = library();
        library.add_personal(personal("alpha", "甲")).unwrap();
        library.add_personal(personal("beta", "乙")).unwrap();
        let report = library
            .import_personal("alpha,甲\nbeta,二\ngamma,丙\nbroken\n")
            .unwrap();
        assert_eq!(report.added, 1);
        assert_eq!(report.merged, 1);
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.failed, 1);
        assert_eq!(report.first_failures[0].line, 4);
        let book = library.load(PERSONAL_ID).unwrap().unwrap();
        assert_eq!(
            book.words().collect::<Vec<_>>(),
            vec!["alpha", "beta", "gamma"]
        );
        assert_eq!(book.entries[1].meaning, "乙；二");

        assert!(matches!(
            library.import_personal("broken\n"),
            Err(WordbookLibraryError::Import(_))
        ));
        assert_eq!(library.load(PERSONAL_ID).unwrap().unwrap(), book);
    }

    #[test]
    fn concurrent_writers_do_not_lose_each_others_words() {
        let root = tempfile::tempdir().unwrap();
        let threads: Vec<_> = (0..8)
            .map(|writer| {
                let path = root.path().to_owned();
                std::thread::spawn(move || {
                    // 每个线程自己建一份句柄，和两个进程各开一份一样，只靠文件锁互斥。
                    let library = WordbookLibrary::new(path);
                    for word in 0..10 {
                        library
                            .add_personal(personal(&format!("w{writer}-{word}"), "义"))
                            .unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let library = WordbookLibrary::new(root.path());
        let book = library.load(PERSONAL_ID).unwrap().unwrap();
        assert_eq!(book.entries.len(), 80);
        assert_eq!(library.list().unwrap()[0].total, 80);
    }

    #[test]
    fn an_index_that_marks_the_wrong_book_personal_is_refused() {
        let (_directory, library) = library();
        library.import("甲", entries(&["a"]), "user-1").unwrap();
        fs::write(
            library.index_path(),
            br#"{"books":[{"id":"user-1","name":"a","total":1,"builtin":false,"personal":true}]}"#,
        )
        .unwrap();
        assert!(matches!(
            library.list(),
            Err(WordbookLibraryError::InvalidWordbook)
        ));
    }
}
