//! The personal dictionary API (user-dictionary.md §15.1-§15.3, `include/metasequoia/personal_dictionary.h`). Error texts are the reference's; copy them from J:1738-2318 and PD.
//!
//! Validation failures are `InvalidArgument` (the bridge threw `invalid_argument` for them); every failure of an edit, page or lookup is `Failed` carrying the reference's `result.error`, which the bridge threw as `runtime_error`.

use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior};

use crate::assets;
use crate::dictionary::english::ensure_english_schema;
use crate::error::{EngineError, Result};
use crate::lattice::personal::PersonalTransition;
use crate::paths::RuntimePaths;
use crate::pinyin::syllables::intact_pinyin_set;
use crate::types::{PersonalDictionaryEntry, PersonalDictionaryKind};
use crate::user_dictionary::journal::{ensure_user_database, open_database, pinyin_table};
use crate::user_dictionary::ngram_store::{
    delete_personal_ngram_word, flush_journal, forget_journal_rows,
};
use crate::user_dictionary::replay::{
    apply_english, apply_pinyin, apply_simple, attach, open_without_main_dictionary,
};

pub const MAX_PAGE_LIMIT: usize = 1_000;
pub const MAX_PAGE_OFFSET: usize = 1_000_000;
pub const MAX_REQUEST_ID_LENGTH: usize = 128;

pub const MAX_ENTRY_WEIGHT: i64 = 100_000_000;
const MAX_KEY_BYTES: usize = 512;
const MAX_VALUE_BYTES: usize = 4_096;
const MAX_PINYIN_SYLLABLES: usize = 64;

// ---- Validation (PD:6-70) ----
pub const WEIGHT_OUT_OF_RANGE: &str = "Weight must be between 1 and 100000000";
pub const UNBOUNDED_ENTRY: &str = "A valid, bounded UTF-8 word and input code are required";
pub const CONTROL_CHARACTER: &str = "The word contains an unsupported control character";
pub const INCOMPLETE_PINYIN: &str =
    "Use complete pinyin syllables separated by apostrophes or spaces";
pub const SYLLABLE_COUNT_MISMATCH: &str =
    "Each character must have one pinyin syllable (maximum 64)";
pub const INVALID_WUBI_CODE: &str = "Wubi codes contain one to four letters";
pub const INVALID_QUICK_PHRASE_CODE: &str =
    "Quick-phrase codes contain one to 32 letters or digits";
pub const INVALID_ENGLISH_CODE: &str = "English codes contain letters, hyphens and apostrophes";
pub const INVALID_CANTONESE_CODE: &str =
    "Use Jyutping syllables separated by spaces or apostrophes";
pub const CANTONESE_SYLLABLE_COUNT_MISMATCH: &str =
    "Each character must have one Jyutping syllable (maximum 64)";
/// 最长的粤拼音节（`gwaang`）六个字母；多留两个，只挡明显不是音节的长串。
const MAX_JYUTPING_SYLLABLE_LETTERS: usize = 8;

// ---- Edits, pages and lookups (J:1792-2317); `bundled` shares them ----
pub(super) const INVALID_REQUEST_ID: &str = "Invalid personal dictionary request ID";
pub(super) const ENTRY_REQUIRED: &str = "An entry is required";
pub(super) const STORAGE_NOT_PREPARED: &str = "Cannot prepare personal dictionary storage";
pub(super) const DICTIONARY_NOT_OPENED: &str = "Cannot open dictionary";
pub(super) const STORAGE_NOT_ATTACHED: &str = "Cannot attach personal dictionary storage";
pub(super) const CONTEXT_NOT_WRITTEN: &str = "Cannot write personal context";
pub(super) const EDIT_NOT_BEGUN: &str = "Cannot begin personal dictionary edit";
pub(super) const RECEIPT_NOT_READ: &str = "Cannot read edit receipt";
pub(super) const RECEIPT_REUSED: &str = "Request ID was already used for another edit";
pub(super) const RETRY_NOT_FINISHED: &str = "Cannot finish edit retry";
pub(super) const RECEIPT_READ_UNFINISHED: &str = "Cannot finish reading edit receipt";
pub(super) const ENTRY_CHANGED: &str = "The entry changed; reload it before editing";
pub(super) const PREVIOUS_NOT_REMOVED: &str = "Cannot remove previous personal entry";
pub(super) const ENTRY_NOT_SAVED: &str = "Cannot save personal entry";
pub(super) const CONTEXT_NOT_REMOVED: &str = "Cannot remove personal context";
pub(super) const RECEIPT_NOT_SAVED: &str = "Cannot save edit receipt";
pub(super) const EDIT_NOT_COMMITTED: &str = "Cannot commit personal dictionary edit";
pub(super) const STORAGE_UNAVAILABLE: &str = "Cannot access personal dictionary storage";
pub(super) const INVALID_PAGE: &str = "Invalid personal dictionary page";
pub(super) const DICTIONARY_NOT_READ: &str = "Cannot read personal dictionary";
pub(super) const PAGE_READ_UNFINISHED: &str = "Cannot finish reading personal dictionary";
/// 没有 `msime-pinyin.db` 的代次（方案集合不读它）只收英文词和粤语词：拼音、五笔和快捷短语都写在 `msime-pinyin.db` 里。
pub(super) const NO_CHINESE_DICTIONARY: &str =
    "This input method has no Chinese dictionary; only English words can be edited";
/// 既没有 `msime-pinyin.db` 也没有 `msime-english.db` 的代次（方案集合两者都不要求，资源目录也没带英文库）什么词也存不下。
pub(super) const NO_ENGLISH_DICTIONARY: &str =
    "This input method has no English dictionary; no words can be edited";

/// 代次里有没有 `msime-pinyin.db`。不分方案集合的入口（[`edit_personal_dictionary`]、[`personal_dictionary_entries`]、`edit_bundled_dictionary_entry`）按代次实际准备出的内容决定：方案集合不读主库时代次里没有它（`prepare_runtime_paths_for`），这时只有英文词和粤语词能编辑、列出。
pub(super) fn generation_has_main_dictionary(paths: &RuntimePaths) -> bool {
    paths.dictionary(assets::MAIN_DICTIONARY).is_file()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersonalDictionaryPage {
    pub entries: Vec<PersonalDictionaryEntry>,
    pub has_more: bool,
}

/// Normalise and check an entry: pinyin `"NI HAO"` becomes `"ni'hao"`; English keys may differ from their display (`dont` / `don't`).
pub fn validate_personal_dictionary_entry(
    entry: &PersonalDictionaryEntry,
) -> Result<PersonalDictionaryEntry> {
    validate_entry(entry, 1..=MAX_ENTRY_WEIGHT)
}

/// The same checks for a row being edited or removed. It is one the list returned and is matched exactly against the journal, so its weight is held only to what the store can hold, not to the range a new entry must fall in. English frequency learning lifts a user's own word past `MAX_ENTRY_WEIGHT` without a ceiling (J:1027-1031), and a wubi word derived from the user's pinyin word ranks below the rows already on its code and can be stored at weight 0 (`wubi_learning::learn`); no writer stores a negative weight. The reference refused such rows (personal_dictionary.cpp:11-12), which left them impossible to edit or remove.
pub(crate) fn validate_existing_entry(
    entry: &PersonalDictionaryEntry,
) -> Result<PersonalDictionaryEntry> {
    validate_entry(entry, 0..=i64::MAX)
}

fn validate_entry(
    entry: &PersonalDictionaryEntry,
    weights: std::ops::RangeInclusive<i64>,
) -> Result<PersonalDictionaryEntry> {
    let invalid = |message: &str| Err(EngineError::invalid(message));
    if !weights.contains(&entry.weight) {
        return invalid(WEIGHT_OUT_OF_RANGE);
    }
    if entry.key.is_empty()
        || entry.key.len() > MAX_KEY_BYTES
        || entry.value.is_empty()
        || entry.value.len() > MAX_VALUE_BYTES
    {
        return invalid(UNBOUNDED_ENTRY);
    }
    let quick = entry.kind == PersonalDictionaryKind::QuickPhrase;
    // A quick phrase may span lines and columns; no other word may carry a control character.
    if entry
        .value
        .bytes()
        .any(|byte| (byte < 32 && !(quick && (byte == b'\n' || byte == b'\t'))) || byte == 127)
    {
        return invalid(CONTROL_CHARACTER);
    }
    let mut key = entry.key.to_ascii_lowercase();
    let letters = |byte: u8| byte.is_ascii_lowercase();
    match entry.kind {
        PersonalDictionaryKind::Pinyin => {
            // Require explicit syllables; guessing a split here could store a different pronunciation.
            key = key.replace(' ', "'");
            let mut count = 0;
            for syllable in key.split('\'') {
                if !intact_pinyin_set().contains(syllable) {
                    return invalid(INCOMPLETE_PINYIN);
                }
                count += 1;
            }
            if count > MAX_PINYIN_SYLLABLES || count != entry.value.chars().count() {
                return invalid(SYLLABLE_COUNT_MISMATCH);
            }
        }
        PersonalDictionaryKind::Wubi | PersonalDictionaryKind::Wubi98 => {
            if key.len() > 4 || !key.bytes().all(letters) {
                return invalid(INVALID_WUBI_CODE);
            }
        }
        PersonalDictionaryKind::QuickPhrase => {
            if key.len() > 32
                || !key
                    .bytes()
                    .all(|byte| letters(byte) || byte.is_ascii_digit())
            {
                return invalid(INVALID_QUICK_PHRASE_CODE);
            }
        }
        PersonalDictionaryKind::English => {
            // The code is what the user types and the word what it types out, and the two need not be the same text: `dont` typing out `don't` is the case that matters (overlay english_display).
            if key.len() > 64
                || !key
                    .bytes()
                    .all(|byte| letters(byte) || byte == b'-' || byte == b'\'')
            {
                return invalid(INVALID_ENGLISH_CODE);
            }
        }
        PersonalDictionaryKind::Cantonese => {
            let Some(syllables) = jyutping_key(&key) else {
                return invalid(INVALID_CANTONESE_CODE);
            };
            let count = syllables.split(' ').count();
            if count > MAX_PINYIN_SYLLABLES || count != entry.value.chars().count() {
                return invalid(CANTONESE_SYLLABLE_COUNT_MISMATCH);
            }
            key = syllables;
        }
    }
    Ok(PersonalDictionaryEntry {
        kind: entry.kind,
        key,
        value: entry.value.clone(),
        weight: entry.weight,
    })
}

/// 粤语编码收成 `msime-cantonese.db` 的键：音节之间的空格或 `'`（连续几个算一个）换成单个空格，每个音节末尾的声调数字 1–6 去掉（`nei5 hou2` 存成 `nei hou`），粤语方案的编码本来就不带声调。音节只能是小写字母；首尾的分隔符忽略。这里只查写法，不查音节表：音节表在 `msime-cantonese.db` 里，没装粤语词库时也要能加词。
fn jyutping_key(key: &str) -> Option<String> {
    let mut normalized = String::with_capacity(key.len());
    for part in key.split([' ', '\'']).filter(|part| !part.is_empty()) {
        let letters = match part.as_bytes() {
            [rest @ .., b'1'..=b'6'] => rest,
            bytes => bytes,
        };
        if letters.is_empty()
            || letters.len() > MAX_JYUTPING_SYLLABLE_LETTERS
            || !letters.iter().all(u8::is_ascii_lowercase)
        {
            return None;
        }
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        // 上面只放过 ASCII 小写字母。
        normalized.push_str(std::str::from_utf8(letters).ok()?);
    }
    (!normalized.is_empty()).then_some(normalized)
}

/// Add (`previous = None`), remove (`replacement = None`) or replace, in one attached transaction with its receipt. `previous` must still be user-inserted with the same weight. A request id (1..=128 of `[A-Za-z0-9_-]`) makes a retry of the same edit a no-op and a reuse for different content a failure.
///
/// An empty request id edits without a receipt, as the reference allowed.
///
/// 自己取独占的维护租约（`DictionaryAccess::try_maintenance`）：还有会话或别的维护持着租约时以 `Busy`（`DICTIONARY_MAINTENANCE_BUSY`）失败，什么也不写。不合法的改动在取租约之前就报出来。代次里没有 `msime-pinyin.db`（方案集合不读它）时只能编辑英文词和粤语词，规则同 `edit_personal_dictionary_with`。
pub fn edit_personal_dictionary(
    paths: &RuntimePaths,
    previous: Option<&PersonalDictionaryEntry>,
    replacement: Option<&PersonalDictionaryEntry>,
    request_id: &str,
) -> Result<()> {
    let main_dictionary = generation_has_main_dictionary(paths);
    normalize_edit(main_dictionary, previous, replacement, request_id)?;
    let _access = maintenance(paths)?;
    edit_personal_dictionary_with(paths, main_dictionary, previous, replacement, request_id)
}

/// 引擎层编辑接口取维护租约；取不到之外的失败（目录打不开、锁文件不对）按本模块的说法报存储不可用。
pub(super) fn maintenance(paths: &RuntimePaths) -> Result<crate::access::DictionaryAccess> {
    crate::access::maintenance(paths).map_err(|error| match error {
        EngineError::Busy(_) => error,
        _ => failed(STORAGE_UNAVAILABLE),
    })
}

/// [`edit_personal_dictionary`]，`main_dictionary` 说明代次里有没有 `msime-pinyin.db`（`SchemeSet::reads_main_dictionary`）。没有时只能编辑英文词和粤语词，其余种类的增删改都以 `NO_CHINESE_DICTIONARY` 失败，什么也不写；代次里连 `msime-english.db` 也没有时，编辑连接建不起来，英文词和粤语词都以 `NO_ENGLISH_DICTIONARY` 失败。不取维护租约，调用方（host 门面背后的 host-api）已经持有。
pub fn edit_personal_dictionary_with(
    paths: &RuntimePaths,
    main_dictionary: bool,
    previous: Option<&PersonalDictionaryEntry>,
    replacement: Option<&PersonalDictionaryEntry>,
    request_id: &str,
) -> Result<()> {
    let (old, new) = normalize_edit(main_dictionary, previous, replacement, request_id)?;
    paths.validate().map_err(|_| failed(STORAGE_UNAVAILABLE))?;
    let journal = paths.user(assets::USER_JOURNAL);
    let mut connection = open_edit_connection(paths, main_dictionary)?;
    // 先刷新内存中的上下文，避免删除词条时遗漏尚未落盘的记录。
    flush_journal(&journal).map_err(|_| failed(CONTEXT_NOT_WRITTEN))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed(EDIT_NOT_BEGUN))?;
    let applied =
        apply_normalized_edit(&transaction, old.as_ref(), new.as_ref(), request_id, false)?;
    transaction
        .commit()
        .map_err(|_| failed(EDIT_NOT_COMMITTED))?;
    if let Some(removed) = applied.forgotten {
        forget_journal_rows(&journal, &removed);
    }
    Ok(())
}

/// 一批最多 128 项，和跨进程个人词库队列的活动请求上限一致。
pub const MAX_EDIT_BATCH: usize = 128;
/// 集合直达 Engine 的专用追加导入上限；普通编辑仍使用 128 项。
pub const MAX_COLLECTION_IMPORT_BATCH: usize = 4096;

/// 一次编辑的借用参数；请求回执格式与单项入口相同。
#[derive(Clone, Copy)]
pub struct PersonalDictionaryEdit<'a> {
    pub previous: Option<&'a PersonalDictionaryEntry>,
    pub replacement: Option<&'a PersonalDictionaryEntry>,
    pub request_id: &'a str,
    /// 集合新增不能覆盖用户原有词；先核对本请求的回执，再判断既有行。
    pub preserve_existing: bool,
}

/// 批量编辑对一项的处理结果，既有用户词归集合管理层记录保护。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonalDictionaryApplied {
    Written,
    AlreadyPresent,
}

struct AppliedEdit {
    outcome: PersonalDictionaryApplied,
    forgotten: Option<Vec<PersonalTransition>>,
}

/// 一次准备连接并提交有界批次；保存点隔离条目失败，外层失败则整批不生效。
pub fn edit_personal_dictionary_batch_with(
    paths: &RuntimePaths,
    main_dictionary: bool,
    edits: &[PersonalDictionaryEdit<'_>],
) -> Result<Vec<Result<PersonalDictionaryApplied>>> {
    edit_personal_dictionary_batch_with_limit(paths, main_dictionary, edits, MAX_EDIT_BATCH)
}

/// 集合导入使用的专用批量入口，不改变普通编辑的 128 项上限。
pub fn edit_personal_dictionary_collection_batch_with(
    paths: &RuntimePaths,
    main_dictionary: bool,
    edits: &[PersonalDictionaryEdit<'_>],
) -> Result<Vec<Result<PersonalDictionaryApplied>>> {
    edit_personal_dictionary_batch_with_limit(
        paths,
        main_dictionary,
        edits,
        MAX_COLLECTION_IMPORT_BATCH,
    )
}

fn edit_personal_dictionary_batch_with_limit(
    paths: &RuntimePaths,
    main_dictionary: bool,
    edits: &[PersonalDictionaryEdit<'_>],
    max_batch: usize,
) -> Result<Vec<Result<PersonalDictionaryApplied>>> {
    if edits.len() > max_batch {
        return Err(failed("Personal dictionary edit batch is too large"));
    }
    if edits.is_empty() {
        return Ok(Vec::new());
    }
    paths.validate().map_err(|_| failed(STORAGE_UNAVAILABLE))?;
    let journal = paths.user(assets::USER_JOURNAL);
    let mut connection = open_edit_connection(paths, main_dictionary)?;
    flush_journal(&journal).map_err(|_| failed(CONTEXT_NOT_WRITTEN))?;
    let mut transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| failed(EDIT_NOT_BEGUN))?;
    let mut outcomes = Vec::with_capacity(edits.len());
    let mut forgotten = Vec::new();
    for edit in edits {
        let (old, new) = match normalize_edit(
            main_dictionary,
            edit.previous,
            edit.replacement,
            edit.request_id,
        ) {
            Ok(entries) => entries,
            Err(error) => {
                outcomes.push(Err(error));
                continue;
            }
        };
        let mut savepoint = transaction
            .savepoint()
            .map_err(|_| failed(EDIT_NOT_BEGUN))?;
        let result = apply_normalized_edit(
            &savepoint,
            old.as_ref(),
            new.as_ref(),
            edit.request_id,
            edit.preserve_existing,
        );
        // RAISE(ROLLBACK) 或存储故障可能已经撤销外层事务，不能再开启保存点并误报前面的项成功。
        if savepoint.is_autocommit() {
            return Err(failed(EDIT_NOT_COMMITTED));
        }
        match result {
            Ok(applied) => {
                savepoint.commit().map_err(|_| failed(EDIT_NOT_COMMITTED))?;
                if let Some(removed) = applied.forgotten {
                    forgotten.extend(removed);
                }
                outcomes.push(Ok(applied.outcome));
            }
            Err(error) => {
                savepoint
                    .rollback()
                    .map_err(|_| failed(EDIT_NOT_COMMITTED))?;
                savepoint.commit().map_err(|_| failed(EDIT_NOT_COMMITTED))?;
                outcomes.push(Err(error));
            }
        }
    }
    transaction
        .commit()
        .map_err(|_| failed(EDIT_NOT_COMMITTED))?;
    if !forgotten.is_empty() {
        forget_journal_rows(&journal, &forgotten);
    }
    Ok(outcomes)
}

fn normalize_edit(
    main_dictionary: bool,
    previous: Option<&PersonalDictionaryEntry>,
    replacement: Option<&PersonalDictionaryEntry>,
    request_id: &str,
) -> Result<(
    Option<PersonalDictionaryEntry>,
    Option<PersonalDictionaryEntry>,
)> {
    if !valid_request_id(request_id) {
        return Err(failed(INVALID_REQUEST_ID));
    }
    if previous.is_none() && replacement.is_none() {
        return Err(failed(ENTRY_REQUIRED));
    }
    let old = previous
        .map(validate_existing_entry)
        .transpose()
        .map_err(as_failure)?;
    let new = replacement
        .map(validate_personal_dictionary_entry)
        .transpose()
        .map_err(as_failure)?;
    if !main_dictionary
        && [old.as_ref(), new.as_ref()]
            .into_iter()
            .flatten()
            .any(|entry| !entry.kind.stored_without_main_dictionary())
    {
        return Err(failed(NO_CHINESE_DICTIONARY));
    }

    Ok((old, new))
}

fn apply_normalized_edit(
    transaction: &Connection,
    old: Option<&PersonalDictionaryEntry>,
    new: Option<&PersonalDictionaryEntry>,
    request_id: &str,
    preserve_existing: bool,
) -> Result<AppliedEdit> {
    let payload = if request_id.is_empty() {
        String::new()
    } else {
        let mut payload = String::new();
        append_payload(&mut payload, old);
        append_payload(&mut payload, new);
        match check_receipt(transaction, request_id, &payload)? {
            Receipt::Fresh => {}
            Receipt::Retry => {
                return Ok(AppliedEdit {
                    outcome: PersonalDictionaryApplied::Written,
                    forgotten: None,
                });
            }
            Receipt::AlreadyPresent => {
                return Ok(AppliedEdit {
                    outcome: PersonalDictionaryApplied::AlreadyPresent,
                    forgotten: None,
                });
            }
        }
        payload
    };
    if preserve_existing && old.is_none() {
        if let Some(new) = new {
            let present = transaction.query_row(
                "SELECT 1 FROM personal_journal.user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 AND operation='upsert' AND user_inserted=1",
                params![new.kind.journal_name(), new.key, new.value], |_| Ok(()),
            ).optional().map_err(|_| failed(DICTIONARY_NOT_READ))?;
            if present.is_some() || bundled_entry_present(transaction, new)? {
                if !request_id.is_empty() {
                    let receipt_payload = format!("{ALREADY_PRESENT_RECEIPT_PREFIX}{payload}");
                    save_receipt(transaction, request_id, &receipt_payload)?;
                }
                return Ok(AppliedEdit {
                    outcome: PersonalDictionaryApplied::AlreadyPresent,
                    forgotten: None,
                });
            }
        }
    }
    if let Some(old) = old {
        let current = transaction
            .query_row(
                "SELECT 1 FROM personal_journal.user_dictionary_operations WHERE dictionary=?1 AND key=?2 AND value=?3 AND weight=?4 AND operation='upsert' AND user_inserted=1",
                params![old.kind.journal_name(), old.key, old.value, old.weight],
                |_| Ok(()),
            )
            .optional();
        if !matches!(current, Ok(Some(()))) {
            return Err(failed(ENTRY_CHANGED));
        }
        if !matches!(apply_personal_edit(transaction, old, true), Ok(true)) {
            return Err(failed(PREVIOUS_NOT_REMOVED));
        }
    }
    if let Some(new) = new {
        if !matches!(apply_personal_edit(transaction, new, false), Ok(true)) {
            return Err(failed(ENTRY_NOT_SAVED));
        }
    }
    // 词条离开词库才删除输入上下文；只调整权重时保留。
    let forgotten = match old {
        Some(old)
            if old.kind == PersonalDictionaryKind::Pinyin
                && !new.is_some_and(|new| {
                    new.kind == PersonalDictionaryKind::Pinyin && new.value == old.value
                }) =>
        {
            Some(
                delete_personal_ngram_word(transaction, "personal_journal", &old.value)
                    .map_err(|_| failed(CONTEXT_NOT_REMOVED))?,
            )
        }
        _ => None,
    };
    if !request_id.is_empty() {
        save_receipt(transaction, request_id, &payload)?;
    }
    Ok(AppliedEdit {
        outcome: PersonalDictionaryApplied::Written,
        forgotten,
    })
}

/// 集合新增不能覆盖当前代次里已经存在的发布词条；集合与用户词库共用同一工作库连接，所以这里直接检查实际表。
fn bundled_entry_present(connection: &Connection, entry: &PersonalDictionaryEntry) -> Result<bool> {
    let (schema, table, key_column, value_column) = match entry.kind {
        PersonalDictionaryKind::Pinyin => {
            let Some(table) = pinyin_table(&entry.key) else {
                return Ok(false);
            };
            ("main", table, "key", "value")
        }
        PersonalDictionaryKind::Wubi | PersonalDictionaryKind::Wubi98 => {
            let Some(table) = entry.kind.wubi_table() else {
                return Ok(false);
            };
            ("main", table.to_owned(), "key", "value")
        }
        PersonalDictionaryKind::QuickPhrase => ("main", "quick_parases".to_owned(), "key", "value"),
        PersonalDictionaryKind::English => (
            "replay_english",
            "english_words".to_owned(),
            "word",
            "display",
        ),
        // 粤语的发布词在只读的 `msime-cantonese.db` 里，编辑连接上没有它；同一个词再加一遍只是多一行，方案出候选时按词去重。
        PersonalDictionaryKind::Cantonese => return Ok(false),
    };
    let table_exists = connection
        .query_row(
            &format!("SELECT 1 FROM {schema}.sqlite_master WHERE type='table' AND name=?1"),
            [&table],
            |_| Ok(()),
        )
        .optional()
        .map_err(|_| failed(DICTIONARY_NOT_READ))?
        .is_some();
    if !table_exists {
        return Ok(false);
    }
    connection
        .query_row(
            &format!(
                "SELECT 1 FROM {schema}.\"{table}\" WHERE \"{key_column}\"=?1 AND \"{value_column}\"=?2 LIMIT 1"
            ),
            params![entry.key, entry.value],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(|_| failed(DICTIONARY_NOT_READ))
}

/// User-inserted entries (auto-created words included) in kind/key/value order; with `include_learned_pinyin`, also learned or edited pinyin weights of shipped multi-character words.
///
/// Rows of a dictionary this engine does not know are left out (J:1974-1982). Text that is not UTF-8 is shown with replacement characters so one damaged row cannot hide the rest of the list; editing such a row fails its stale check and changes nothing.
pub fn personal_dictionary_entries(
    paths: &RuntimePaths,
    offset: usize,
    limit: usize,
    include_learned_pinyin: bool,
) -> Result<PersonalDictionaryPage> {
    personal_dictionary_entries_with(
        paths,
        generation_has_main_dictionary(paths),
        offset,
        limit,
        include_learned_pinyin,
    )
}

/// [`personal_dictionary_entries`]，`main_dictionary` 为假（代次里没有 `msime-pinyin.db`）时只列英文词和粤语词：别的种类的行（例如从别的版本恢复来的快照）在这里既用不上也改不了，所以不列出。
pub fn personal_dictionary_entries_with(
    paths: &RuntimePaths,
    main_dictionary: bool,
    offset: usize,
    limit: usize,
    include_learned_pinyin: bool,
) -> Result<PersonalDictionaryPage> {
    read_personal_dictionary_page(
        paths,
        main_dictionary,
        offset,
        limit,
        include_learned_pinyin,
        true,
        None,
    )
}

/// 指定词库先筛选再分页；拼音带学习权重且排除单字，其他词库只导出用户词条。
pub fn personal_dictionary_export_entries_for_kind(
    paths: &RuntimePaths,
    main_dictionary: bool,
    kind: PersonalDictionaryKind,
    offset: usize,
    limit: usize,
) -> Result<PersonalDictionaryPage> {
    read_personal_dictionary_page(
        paths,
        main_dictionary,
        offset,
        limit,
        kind == PersonalDictionaryKind::Pinyin,
        true,
        Some(kind),
    )
}

/// 只查询指定词库的用户词条；列表保留拼音单字，不带学习权重。
pub fn personal_dictionary_entries_for_kind(
    paths: &RuntimePaths,
    main_dictionary: bool,
    kind: PersonalDictionaryKind,
    offset: usize,
    limit: usize,
) -> Result<PersonalDictionaryPage> {
    read_personal_dictionary_page(
        paths,
        main_dictionary,
        offset,
        limit,
        false,
        false,
        Some(kind),
    )
}

fn read_personal_dictionary_page(
    paths: &RuntimePaths,
    main_dictionary: bool,
    offset: usize,
    limit: usize,
    include_learned_pinyin: bool,
    exclude_single_pinyin: bool,
    kind: Option<PersonalDictionaryKind>,
) -> Result<PersonalDictionaryPage> {
    if limit == 0 || limit > MAX_PAGE_LIMIT || offset > MAX_PAGE_OFFSET {
        return Err(failed(INVALID_PAGE));
    }
    paths.validate().map_err(|_| failed(STORAGE_UNAVAILABLE))?;
    let journal = paths.user(assets::USER_JOURNAL);
    // 列表读取不创建日志（J:1939）。
    if !journal.exists() {
        return Ok(PersonalDictionaryPage::default());
    }
    let connection = open_database(&journal, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| failed(DICTIONARY_NOT_READ))?;
    let sql = if kind.is_some() {
        "SELECT dictionary,key,value,weight FROM user_dictionary_operations WHERE operation='upsert' AND (user_inserted=1 OR (?3 AND dictionary='pinyin')) AND NOT (?3 AND ?5 AND dictionary='pinyin' AND length(value)<=1) AND (?4 OR dictionary IN ('english','cantonese')) AND dictionary=?6 ORDER BY dictionary,key,value LIMIT ?1 OFFSET ?2"
    } else {
        "SELECT dictionary,key,value,weight FROM user_dictionary_operations WHERE operation='upsert' AND (user_inserted=1 OR (?3 AND dictionary='pinyin')) AND NOT (?3 AND ?5 AND dictionary='pinyin' AND length(value)<=1) AND (?4 OR dictionary IN ('english','cantonese')) ORDER BY dictionary,key,value LIMIT ?1 OFFSET ?2"
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(|_| failed(DICTIONARY_NOT_READ))?;
    let mut page = PersonalDictionaryPage::default();
    let read = (|| -> rusqlite::Result<()> {
        let mut rows = if let Some(kind) = kind {
            statement.query(params![
                page_bind(limit + 1),
                page_bind(offset),
                i64::from(include_learned_pinyin),
                i64::from(main_dictionary),
                i64::from(exclude_single_pinyin),
                kind.journal_name()
            ])?
        } else {
            statement.query(params![
                page_bind(limit + 1),
                page_bind(offset),
                i64::from(include_learned_pinyin),
                i64::from(main_dictionary),
                i64::from(exclude_single_pinyin)
            ])?
        };
        while let Some(row) = rows.next()? {
            if page.entries.len() == limit {
                page.has_more = true;
                break;
            }
            let Some(kind) = PersonalDictionaryKind::from_journal_name(&column_text(row, 0)?)
            else {
                continue;
            };
            page.entries.push(PersonalDictionaryEntry {
                kind,
                key: column_text(row, 1)?,
                value: column_text(row, 2)?,
                weight: column_i64(row, 3)?,
            });
        }
        Ok(())
    })();
    if read.is_err() {
        return Err(failed(PAGE_READ_UNFINISHED));
    }
    Ok(page)
}

/// The main dictionary opened for writing with the journal attached as `personal_journal` and the English dictionary as `replay_english`, both prepared first (J:1822-1836). `main_dictionary` 为假（代次里没有 `msime-pinyin.db`）时以一个内存库代替它，只有 attach 的日志和英文词库可写；这时代次里也没有 `msime-english.db` 就以 `NO_ENGLISH_DICTIONARY` 失败，不在代次里建一个空的英文库。
///
/// 日志是 WAL 时（iOS 以外）编辑事务只在每个文件内原子：崩溃可能留下日志已记下、词库工作副本回滚，下一次回放补齐，见 `journal::prefer_write_ahead_log`。
pub(super) fn open_edit_connection(
    paths: &RuntimePaths,
    main_dictionary: bool,
) -> Result<Connection> {
    let journal = paths.user(assets::USER_JOURNAL);
    let english = paths.dictionary(assets::ENGLISH_DICTIONARY);
    if !main_dictionary && !english.is_file() {
        return Err(failed(NO_ENGLISH_DICTIONARY));
    }
    if ensure_user_database(&journal).is_err() || ensure_english_schema(&english).is_err() {
        return Err(failed(STORAGE_NOT_PREPARED));
    }
    let connection = if main_dictionary {
        open_database(
            &paths.dictionary(assets::MAIN_DICTIONARY),
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .map_err(|_| failed(DICTIONARY_NOT_OPENED))?
    } else {
        open_without_main_dictionary().map_err(|_| failed(DICTIONARY_NOT_OPENED))?
    };
    for (schema, path) in [("personal_journal", &journal), ("replay_english", &english)] {
        attach(&connection, path, schema).map_err(|_| failed(STORAGE_NOT_ATTACHED))?;
    }
    Ok(connection)
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Receipt {
    /// 还没有编辑使用这个请求 id。
    Fresh,
    /// 相同编辑已经用这个 id 提交；提交并报告成功。
    Retry,
    /// 相同的保留已有词条编辑已经用这个 id 提交；提交并报告已存在。
    AlreadyPresent,
}

const ALREADY_PRESENT_RECEIPT_PREFIX: &str = "already_present;";

/// Idempotency by request id: the stored payload must match exactly (J:1858-1880).
pub(super) fn check_receipt(
    transaction: &Connection,
    request_id: &str,
    payload: &str,
) -> Result<Receipt> {
    let mut statement = transaction
        .prepare(
            "SELECT payload FROM personal_journal.personal_dictionary_receipts WHERE request_id=?1",
        )
        .map_err(|_| failed(RECEIPT_NOT_READ))?;
    let stored = statement
        .query_row([request_id], |row| row.get::<_, Option<String>>(0))
        .optional()
        .map_err(|_| failed(RECEIPT_READ_UNFINISHED))?;
    match stored {
        None => Ok(Receipt::Fresh),
        Some(stored) if stored.as_deref() == Some(payload) => Ok(Receipt::Retry),
        Some(stored)
            if stored
                .as_deref()
                .and_then(|stored| stored.strip_prefix(ALREADY_PRESENT_RECEIPT_PREFIX))
                == Some(payload) =>
        {
            Ok(Receipt::AlreadyPresent)
        }
        Some(_) => Err(failed(RECEIPT_REUSED)),
    }
}

pub(super) fn save_receipt(
    transaction: &Connection,
    request_id: &str,
    payload: &str,
) -> Result<()> {
    transaction
        .execute(
            "INSERT INTO personal_journal.personal_dictionary_receipts(request_id,payload) VALUES(?1,?2)",
            params![request_id, payload],
        )
        .map_err(|_| failed(RECEIPT_NOT_SAVED))?;
    Ok(())
}

/// `none;` or `<kind ordinal>;<weight>;<len>:<key><len>:<value>` with byte lengths (J:1847-1857). Stored receipts compare against this text, so it must not change.
fn append_payload(payload: &mut String, entry: Option<&PersonalDictionaryEntry>) {
    let Some(entry) = entry else {
        payload.push_str("none;");
        return;
    };
    payload.push_str(&format!("{};{};", entry.kind as u8, entry.weight));
    for text in [&entry.key, &entry.value] {
        payload.push_str(&format!("{}:{}", text.len(), text));
    }
}

/// Apply to the working table, then journal it as the user's own row: a removal is a tombstone with `user_inserted = 1`, so replay also removes a same-named shipped row (J:1757-1790).
fn apply_personal_edit(
    connection: &Connection,
    entry: &PersonalDictionaryEntry,
    remove: bool,
) -> rusqlite::Result<bool> {
    let (key, value, weight) = (entry.key.as_str(), entry.value.as_str(), entry.weight);
    let (key_bytes, value_bytes) = (key.as_bytes(), value.as_bytes());
    let applied = match entry.kind {
        PersonalDictionaryKind::Pinyin => {
            apply_pinyin(connection, key_bytes, value_bytes, remove, weight)?
        }
        PersonalDictionaryKind::Wubi | PersonalDictionaryKind::Wubi98 => apply_simple(
            connection,
            entry.kind.wubi_table().unwrap_or_default(),
            key_bytes,
            value_bytes,
            remove,
            weight,
        )?,
        PersonalDictionaryKind::QuickPhrase => apply_simple(
            connection,
            "quick_parases",
            key_bytes,
            value_bytes,
            remove,
            weight,
        )?,
        PersonalDictionaryKind::English => apply_english(
            connection,
            key_bytes,
            value_bytes,
            remove,
            weight,
            value_bytes,
        )?,
        // 粤语词只记在日志里，下面那一行就是它的全部存储。
        PersonalDictionaryKind::Cantonese => true,
    };
    if !applied {
        return Ok(false);
    }
    let display = if entry.kind == PersonalDictionaryKind::English {
        value
    } else {
        ""
    };
    connection.execute(
        "INSERT INTO personal_journal.user_dictionary_operations(dictionary,key,value,operation,weight,display,user_inserted) VALUES(?1,?2,?3,?4,?5,?6,1) ON CONFLICT(dictionary,key,value) DO UPDATE SET operation=excluded.operation, weight=excluded.weight,display=excluded.display,user_inserted=1,updated_at=unixepoch()",
        params![
            entry.kind.journal_name(),
            key,
            value,
            if remove { "delete" } else { "upsert" },
            if remove { 0 } else { weight },
            display
        ],
    )?;
    Ok(true)
}

/// Empty, or at most 128 of `[A-Za-z0-9_-]` (J:2020-2026).
pub(super) fn valid_request_id(request_id: &str) -> bool {
    request_id.len() <= MAX_REQUEST_ID_LENGTH
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(super) fn failed(message: &str) -> EngineError {
    EngineError::failed(message)
}

/// An edit reports a validation failure as its own failure, as `result.error` did.
fn as_failure(error: EngineError) -> EngineError {
    match error {
        EngineError::InvalidArgument(message) => EngineError::Failed(message),
        other => other,
    }
}

/// `LIMIT` and `OFFSET` binds; both are bounded by the page checks above.
pub(super) fn page_bind(value: usize) -> i64 {
    value as i64
}

/// `sqlite3_column_text` as the reference's pages read it: NULL is empty, and bytes that are not UTF-8 are replaced for display.
pub(super) fn column_text(row: &Row<'_>, index: usize) -> rusqlite::Result<String> {
    Ok(match row.get_ref(index)? {
        ValueRef::Null => String::new(),
        ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
            String::from_utf8_lossy(bytes).into_owned()
        }
        ValueRef::Integer(number) => number.to_string(),
        ValueRef::Real(number) => number.to_string(),
    })
}

/// `sqlite3_column_int64`: NULL reads as 0.
pub(super) fn column_i64(row: &Row<'_>, index: usize) -> rusqlite::Result<i64> {
    Ok(row.get::<_, Option<i64>>(index)?.unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_dictionary::generation::prepare_runtime_paths;
    use crate::user_dictionary::replay::replay;
    use crate::user_dictionary::replay::tests::{sql, weight};
    use std::path::Path;

    fn entry(
        kind: PersonalDictionaryKind,
        key: &str,
        value: &str,
        weight: i64,
    ) -> PersonalDictionaryEntry {
        PersonalDictionaryEntry {
            kind,
            key: key.into(),
            value: value.into(),
            weight,
        }
    }

    /// test_personal_dictionary.cpp:53-60.
    fn fixture(root: &Path) -> RuntimePaths {
        let resources = root.join("resources");
        std::fs::create_dir_all(&resources).unwrap();
        sql(
            &resources.join(assets::MAIN_DICTIONARY),
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             CREATE TABLE tbl_7_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             CREATE TABLE tbl_others_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);",
        );
        ensure_english_schema(&resources.join(assets::ENGLISH_DICTIONARY)).unwrap();
        prepare_runtime_paths(&resources, &root.join("user"), &root.join("cache"), "first").unwrap()
    }

    /// What a session would find: the row in the working table the kind lives in.
    fn has(paths: &RuntimePaths, entry: &PersonalDictionaryEntry) -> bool {
        let (database, query) = match entry.kind {
            PersonalDictionaryKind::English => (
                assets::ENGLISH_DICTIONARY,
                "SELECT count(*) FROM english_words WHERE word=?1 AND display=?2".to_owned(),
            ),
            PersonalDictionaryKind::Wubi | PersonalDictionaryKind::Wubi98 => (
                assets::MAIN_DICTIONARY,
                format!(
                    "SELECT count(*) FROM {} WHERE key=?1 AND value=?2",
                    entry.kind.wubi_table().unwrap()
                ),
            ),
            PersonalDictionaryKind::QuickPhrase => (
                assets::MAIN_DICTIONARY,
                "SELECT count(*) FROM quick_parases WHERE key=?1 AND value=?2".to_owned(),
            ),
            PersonalDictionaryKind::Pinyin => (
                assets::MAIN_DICTIONARY,
                format!(
                    "SELECT count(*) FROM \"{}\" WHERE key=?1 AND value=?2",
                    crate::user_dictionary::journal::pinyin_table(&entry.key).unwrap()
                ),
            ),
            // 粤语词没有工作表，粤语方案从日志读它。
            PersonalDictionaryKind::Cantonese => (
                assets::USER_JOURNAL,
                "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='cantonese' AND key=?1 AND value=?2 AND operation='upsert'".to_owned(),
            ),
        };
        let file = if entry.kind == PersonalDictionaryKind::Cantonese {
            paths.user(database)
        } else {
            paths.dictionary(database)
        };
        let count: i64 = Connection::open(file)
            .unwrap()
            .query_row(&query, params![entry.key, entry.value], |row| row.get(0))
            .unwrap();
        count > 0
    }

    #[test]
    fn preserved_import_receipt_survives_removal_before_retry() {
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let original = entry(PersonalDictionaryKind::Pinyin, "ni'hao", "合成", 50_000);
        edit_personal_dictionary(&paths, None, Some(&original), "fixture-owner").unwrap();
        let imported = PersonalDictionaryEntry {
            weight: 10_000,
            ..original.clone()
        };
        let batch = [PersonalDictionaryEdit {
            previous: None,
            replacement: Some(&imported),
            request_id: "collections-preserved-retry",
            preserve_existing: true,
        }];
        let first = edit_personal_dictionary_batch_with(&paths, true, &batch).unwrap();
        assert_eq!(
            first[0].as_ref().unwrap(),
            &PersonalDictionaryApplied::AlreadyPresent
        );
        edit_personal_dictionary(&paths, Some(&original), None, "fixture-owner-removes").unwrap();
        assert!(!has(&paths, &original));
        let retried = edit_personal_dictionary_batch_with(&paths, true, &batch).unwrap();
        assert_eq!(
            retried[0].as_ref().unwrap(),
            &PersonalDictionaryApplied::AlreadyPresent
        );
        assert!(!has(&paths, &imported));
    }

    #[test]
    fn preserved_import_receipt_rejects_a_changed_payload() {
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let original = entry(PersonalDictionaryKind::Pinyin, "ni'hao", "合成", 50_000);
        edit_personal_dictionary(&paths, None, Some(&original), "fixture-owner").unwrap();
        let imported = PersonalDictionaryEntry {
            weight: 10_000,
            ..original
        };
        let attempt = |replacement| {
            edit_personal_dictionary_batch_with(
                &paths,
                true,
                &[PersonalDictionaryEdit {
                    previous: None,
                    replacement: Some(replacement),
                    request_id: "collections-preserved-payload",
                    preserve_existing: true,
                }],
            )
            .unwrap()
            .remove(0)
        };
        assert_eq!(
            attempt(&imported).unwrap(),
            PersonalDictionaryApplied::AlreadyPresent
        );
        let changed = PersonalDictionaryEntry {
            weight: 10_001,
            ..imported.clone()
        };
        assert_eq!(attempt(&changed).unwrap_err().to_string(), RECEIPT_REUSED);
    }

    #[test]
    fn preserved_import_receipt_failure_is_not_reported_as_success() {
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let original = entry(PersonalDictionaryKind::Pinyin, "ni'hao", "合成", 50_000);
        edit_personal_dictionary(&paths, None, Some(&original), "fixture-owner").unwrap();
        let journal = paths.user(assets::USER_JOURNAL);
        sql(&journal, "CREATE TRIGGER reject_preserved_receipt BEFORE INSERT ON personal_dictionary_receipts WHEN NEW.request_id='collections-preserved-failure' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;");
        let imported = PersonalDictionaryEntry {
            weight: 10_000,
            ..original.clone()
        };
        let result = edit_personal_dictionary_batch_with(
            &paths,
            true,
            &[PersonalDictionaryEdit {
                previous: None,
                replacement: Some(&imported),
                request_id: "collections-preserved-failure",
                preserve_existing: true,
            }],
        )
        .unwrap()
        .remove(0);
        assert_eq!(result.unwrap_err().to_string(), RECEIPT_NOT_SAVED);
        assert!(has(&paths, &original));
        let receipts: i64 = Connection::open(journal).unwrap().query_row(
            "SELECT count(*) FROM personal_dictionary_receipts WHERE request_id='collections-preserved-failure'",
            [], |row| row.get(0)
        ).unwrap();
        assert_eq!(receipts, 0);
    }

    // engine-bridge tests.rs `validates_and_normalizes_personal_dictionary_entries` and test_personal_dictionary.cpp:63-76 with the overlay's English rule.
    #[test]
    fn validation_normalises_and_rejects_with_the_reference_messages() {
        use PersonalDictionaryKind::*;
        let normalized =
            validate_personal_dictionary_entry(&entry(Pinyin, "NI HAO", "拟好", 12345)).unwrap();
        assert_eq!(normalized, entry(Pinyin, "ni'hao", "拟好", 12345));
        assert_eq!(
            validate_personal_dictionary_entry(&entry(English, "dont", "don't", 100_000)).unwrap(),
            entry(English, "dont", "don't", 100_000)
        );
        assert_eq!(
            validate_personal_dictionary_entry(&entry(English, "hello", "different", 100_000))
                .unwrap(),
            entry(English, "hello", "different", 100_000)
        );
        // 粤语：空格、`'` 和连续的分隔都收成单个空格，音节末尾的声调数字去掉。
        for typed in [
            "nei hou",
            "Nei5 Hou2",
            "nei'hou",
            "  nei''hou ",
            "nei2 'hou",
        ] {
            assert_eq!(
                validate_personal_dictionary_entry(&entry(Cantonese, typed, "你好", 100_000))
                    .unwrap(),
                entry(Cantonese, "nei hou", "你好", 100_000),
                "{typed:?}"
            );
        }
        assert_eq!(
            validate_personal_dictionary_entry(&entry(
                QuickPhrase,
                "TEST1",
                "fixture\nsecond\tline",
                1
            ))
            .unwrap()
            .key,
            "test1"
        );
        for (invalid, message) in [
            (entry(Pinyin, "nihao", "你好", 1), INCOMPLETE_PINYIN),
            (entry(Pinyin, "ni''hao", "你好", 1), INCOMPLETE_PINYIN),
            (entry(Pinyin, "ni'hao", "你", 1), SYLLABLE_COUNT_MISMATCH),
            (
                entry(Pinyin, &vec!["ni"; 65].join("'"), &"你".repeat(65), 1),
                SYLLABLE_COUNT_MISMATCH,
            ),
            (entry(Wubi, "abcde", "词", 1), INVALID_WUBI_CODE),
            (entry(Wubi, "ab1", "词", 1), INVALID_WUBI_CODE),
            (entry(Wubi98, "abcde", "词", 1), INVALID_WUBI_CODE),
            (
                entry(Cantonese, "neihou", "你好", 1),
                CANTONESE_SYLLABLE_COUNT_MISMATCH,
            ),
            (
                entry(Cantonese, "nei", "你好", 1),
                CANTONESE_SYLLABLE_COUNT_MISMATCH,
            ),
            (
                entry(Cantonese, "nei7 hou", "你好", 1),
                INVALID_CANTONESE_CODE,
            ),
            (entry(Cantonese, "nei 1", "你好", 1), INVALID_CANTONESE_CODE),
            (
                entry(Cantonese, "nei-hou", "你好", 1),
                INVALID_CANTONESE_CODE,
            ),
            (entry(Cantonese, " ' ", "你", 1), INVALID_CANTONESE_CODE),
            (
                entry(QuickPhrase, "bad;code", "text", 1),
                INVALID_QUICK_PHRASE_CODE,
            ),
            (
                entry(QuickPhrase, &"a".repeat(33), "text", 1),
                INVALID_QUICK_PHRASE_CODE,
            ),
            (
                entry(English, "wrong_code", "Word", 1),
                INVALID_ENGLISH_CODE,
            ),
            (entry(English, "hello", "a\0b", 1), CONTROL_CHARACTER),
            (entry(Pinyin, "ni", "你\u{7f}", 1), CONTROL_CHARACTER),
            (entry(Wubi, "ab", "词\n", 1), CONTROL_CHARACTER),
            (entry(Pinyin, "ni", "你", 0), WEIGHT_OUT_OF_RANGE),
            (entry(Pinyin, "ni", "你", 100_000_001), WEIGHT_OUT_OF_RANGE),
            (entry(Pinyin, "", "你", 1), UNBOUNDED_ENTRY),
            (entry(Pinyin, "ni", "", 1), UNBOUNDED_ENTRY),
            (
                entry(QuickPhrase, "a", &"x".repeat(4097), 1),
                UNBOUNDED_ENTRY,
            ),
        ] {
            let error = validate_personal_dictionary_entry(&invalid).unwrap_err();
            assert!(
                matches!(error, EngineError::InvalidArgument(_)),
                "{invalid:?}"
            );
            assert_eq!(error.to_string(), message, "{invalid:?}");
        }
    }

    // test_personal_dictionary.cpp:61-163, probing the working tables where the reference typed into a session.
    /// 引擎层的编辑接口自己取维护租约：还有会话持共享租约时报 `Busy`、什么也不写；不合法的改动照旧先报出来；会话结束后同一改动照常生效。
    #[test]
    fn engine_level_edits_take_the_maintenance_lease() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let pinyin =
            validate_personal_dictionary_entry(&entry(Pinyin, "NI HAO", "拟好", 12345)).unwrap();
        let session = crate::DictionaryAccess::try_session(&paths.user_data, &paths.dictionaries)
            .unwrap()
            .unwrap();

        let error = edit_personal_dictionary(&paths, None, Some(&pinyin), "busy-1").unwrap_err();
        assert_eq!(error.code(), crate::ErrorCode::Busy);
        assert_eq!(
            error.to_string(),
            crate::diagnostics::DICTIONARY_MAINTENANCE_BUSY
        );
        assert!(!has(&paths, &pinyin));
        let invalid = entry(Pinyin, "nihao", "你好", 1);
        let error = edit_personal_dictionary(&paths, None, Some(&invalid), "").unwrap_err();
        assert_eq!(error.code(), crate::ErrorCode::Failed);
        // 门面的 `_with` 版本假定调用方已持有租约，不再取锁。
        drop(session);
        let maintenance =
            crate::DictionaryAccess::try_maintenance(&paths.user_data, &paths.dictionaries)
                .unwrap()
                .unwrap();
        edit_personal_dictionary_with(&paths, true, None, Some(&pinyin), "busy-1").unwrap();
        assert!(has(&paths, &pinyin));
        drop(maintenance);
        edit_personal_dictionary(&paths, Some(&pinyin), None, "busy-2").unwrap();
        assert!(!has(&paths, &pinyin));
    }

    #[test]
    fn edits_replace_page_refuse_stale_and_survive_an_upgrade() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        assert_eq!(
            personal_dictionary_entries(&paths, 0, 100, false).unwrap(),
            PersonalDictionaryPage::default()
        );
        let journal = paths.user(assets::USER_JOURNAL);

        for invalid in [
            entry(Pinyin, "nihao", "你好", 1),
            entry(Wubi, "abcde", "词", 1),
            entry(English, "hello", "a\0b", 1),
        ] {
            let error = edit_personal_dictionary(&paths, None, Some(&invalid), "").unwrap_err();
            assert!(matches!(error, EngineError::Failed(_)), "{error}");
        }
        assert_eq!(
            edit_personal_dictionary(&paths, None, None, "")
                .unwrap_err()
                .to_string(),
            ENTRY_REQUIRED
        );

        let pinyin =
            validate_personal_dictionary_entry(&entry(Pinyin, "NI HAO", "拟好", 12345)).unwrap();
        edit_personal_dictionary(&paths, None, Some(&pinyin), "").unwrap();
        assert!(has(&paths, &pinyin));
        assert_eq!(
            weight(
                &paths.dictionary(assets::MAIN_DICTIONARY),
                "SELECT count(*) FROM tbl_2_n WHERE jp='nh' AND weight=12345"
            ),
            Some(1)
        );
        let wubi = entry(Wubi, "wq", "拟好", 12345);
        let english = entry(English, "metasequoia", "Metasequoia", 12345);
        let quick = entry(QuickPhrase, "test1", "fixture\nsecond line", 12345);
        for added in [&wubi, &english, &quick] {
            edit_personal_dictionary(&paths, None, Some(added), "").unwrap();
            assert!(has(&paths, added), "{added:?}");
        }
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM user_dictionary_operations WHERE dictionary='english' AND display='Metasequoia' AND user_inserted=1"),
            Some(1)
        );

        // Moving between dictionaries commits the old tombstone and the new row together.
        let moved = entry(English, "fixtureword", "FixtureWord", quick.weight);
        edit_personal_dictionary(&paths, Some(&quick), Some(&moved), "").unwrap();
        assert!(has(&paths, &moved) && !has(&paths, &quick));
        edit_personal_dictionary(&paths, Some(&moved), Some(&quick), "").unwrap();
        assert!(!has(&paths, &moved) && has(&paths, &quick));

        let first = personal_dictionary_entries(&paths, 0, 2, false).unwrap();
        assert_eq!(first.entries.len(), 2);
        assert!(first.has_more);
        assert_eq!(first.entries[0], english);
        let rest = personal_dictionary_entries(&paths, 2, 2, false).unwrap();
        assert_eq!(rest.entries.len(), 2);
        assert!(!rest.has_more);
        assert_eq!(rest.entries, vec![quick.clone(), wubi.clone()]);
        for (offset, limit) in [(0, 0), (0, 1001), (1_000_001, 1)] {
            assert_eq!(
                personal_dictionary_entries(&paths, offset, limit, false)
                    .unwrap_err()
                    .to_string(),
                INVALID_PAGE
            );
        }

        // Seven syllables use a numbered table; eight and nine use `tbl_others_n` (the format contract).
        for count in [7, 8, 9] {
            let long = entry(
                Pinyin,
                &vec!["ni"; count].join("'"),
                &"你".repeat(count),
                100_000,
            );
            edit_personal_dictionary(&paths, None, Some(&long), "").unwrap();
            assert!(has(&paths, &long), "{count}");
        }

        let changed = PersonalDictionaryEntry {
            value: "你好".into(),
            ..pinyin.clone()
        };
        let stale = PersonalDictionaryEntry {
            weight: pinyin.weight - 1,
            ..pinyin.clone()
        };
        assert_eq!(
            edit_personal_dictionary(&paths, Some(&stale), Some(&changed), "")
                .unwrap_err()
                .to_string(),
            ENTRY_CHANGED
        );
        assert!(has(&paths, &pinyin));
        // `bu'hao` has no table in the fixture, so the replacement fails and the removal of `previous` rolls back with it.
        let missing_table = entry(Pinyin, "bu'hao", "补好", pinyin.weight);
        assert_eq!(
            edit_personal_dictionary(&paths, Some(&pinyin), Some(&missing_table), "")
                .unwrap_err()
                .to_string(),
            ENTRY_NOT_SAVED
        );
        assert!(has(&paths, &pinyin));

        edit_personal_dictionary(&paths, Some(&pinyin), Some(&changed), "edit-1").unwrap();
        assert!(!has(&paths, &pinyin) && has(&paths, &changed));
        edit_personal_dictionary(&paths, Some(&pinyin), Some(&changed), "edit-1").unwrap();
        assert_eq!(
            edit_personal_dictionary(&paths, Some(&changed), Some(&pinyin), "edit-1")
                .unwrap_err()
                .to_string(),
            RECEIPT_REUSED
        );
        assert!(has(&paths, &changed));
        for bad in ["bad id", "é", &"a".repeat(129)] {
            assert_eq!(
                edit_personal_dictionary(&paths, Some(&changed), Some(&pinyin), bad)
                    .unwrap_err()
                    .to_string(),
                INVALID_REQUEST_ID
            );
        }

        // A journal write failure after the candidate update rolls both back and leaves no receipt.
        sql(&journal, "CREATE TRIGGER reject_edit BEFORE INSERT ON user_dictionary_operations BEGIN SELECT RAISE(ABORT,'fixture failure'); END;");
        assert!(
            edit_personal_dictionary(&paths, Some(&changed), Some(&pinyin), "failure-retry")
                .is_err()
        );
        assert!(has(&paths, &changed) && !has(&paths, &pinyin));
        sql(&journal, "DROP TRIGGER reject_edit;");
        edit_personal_dictionary(&paths, Some(&changed), Some(&pinyin), "failure-retry").unwrap();
        edit_personal_dictionary(&paths, Some(&pinyin), Some(&changed), "").unwrap();
        edit_personal_dictionary(&paths, Some(&changed), None, "").unwrap();
        assert!(!has(&paths, &changed));
        assert_eq!(
            weight(&journal, "SELECT user_inserted FROM user_dictionary_operations WHERE value='你好' AND operation='delete' AND weight=0"),
            Some(1)
        );

        let upgraded =
            prepare_runtime_paths(&paths.resources, &paths.user_data, &paths.cache, "second")
                .unwrap();
        assert!(!has(&upgraded, &changed) && !has(&upgraded, &pinyin));
        assert!(has(&upgraded, &wubi) && has(&upgraded, &english) && has(&upgraded, &quick));
        // A retry of an edit committed before later changes stays a no-op instead of resurrecting the removed word.
        edit_personal_dictionary(&upgraded, Some(&pinyin), Some(&changed), "edit-1").unwrap();
        assert!(!has(&upgraded, &changed));
        let entries = personal_dictionary_entries(&upgraded, 0, 100, false).unwrap();
        assert_eq!(entries.entries.len(), 6);
    }

    #[test]
    fn removing_a_pinyin_word_drops_its_personal_context_and_reweighting_keeps_it() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        let word = entry(Pinyin, "ni'hao", "你好", 100);
        edit_personal_dictionary(&paths, None, Some(&word), "").unwrap();
        sql(&journal, "INSERT INTO personal_bigram VALUES('\u{1}','你好',3),('你好','世界',2); INSERT INTO personal_trigram VALUES('我','你好','世界',1);");
        let reweighted = PersonalDictionaryEntry {
            weight: 200,
            ..word.clone()
        };
        edit_personal_dictionary(&paths, Some(&word), Some(&reweighted), "").unwrap();
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM personal_bigram"),
            Some(2)
        );
        edit_personal_dictionary(&paths, Some(&reweighted), None, "").unwrap();
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM personal_bigram"),
            Some(0)
        );
        assert_eq!(
            weight(&journal, "SELECT count(*) FROM personal_trigram"),
            Some(0)
        );
    }

    #[test]
    fn learned_pinyin_is_exported_without_single_characters_and_unknown_rows_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        ensure_user_database(&journal).unwrap();
        sql(
            &journal,
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted) VALUES
               ('pinyin','ni''hao','你好','upsert',1,0),
               ('pinyin','ni','你','upsert',2,0),
               ('pinyin','ni''men','你们','delete',0,0),
               ('wubi','wqvb','你好','upsert',3,0),
               ('japanese','ni','你','upsert',4,1),
               ('quick','dh',CAST(x'e794b5ff' AS TEXT),'upsert',5,1);",
        );
        let user_only = personal_dictionary_entries(&paths, 0, 100, false).unwrap();
        assert_eq!(
            user_only.entries,
            vec![entry(
                PersonalDictionaryKind::QuickPhrase,
                "dh",
                "电\u{fffd}",
                5
            )]
        );
        let exported = personal_dictionary_entries(&paths, 0, 100, true).unwrap();
        assert_eq!(
            exported.entries,
            vec![
                entry(PersonalDictionaryKind::Pinyin, "ni'hao", "你好", 1),
                entry(PersonalDictionaryKind::QuickPhrase, "dh", "电\u{fffd}", 5),
            ]
        );
    }

    #[test]
    fn kind_export_filters_before_offset_and_keeps_export_rules() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        ensure_user_database(&journal).unwrap();
        let connection = open_database(&journal, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
        for kind in [Pinyin, Wubi, QuickPhrase, English, Wubi98, Cantonese] {
            for (key, value, weight) in [
                ("a", "合成甲", 11),
                ("a", "合成乙", 12),
                ("b", "合成丙", 13),
            ] {
                connection.execute(
                    "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted) VALUES(?1,?2,?3,'upsert',?4,1)",
                    params![kind.journal_name(), key, value, weight],
                ).unwrap();
            }
            connection.execute(
                "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted) VALUES(?1,'x','合成学习','upsert',21,0), (?1,'y','合成删除','delete',0,1)",
                [kind.journal_name()],
            ).unwrap();
        }
        connection.execute_batch(
            "INSERT INTO user_dictionary_operations(dictionary,key,value,operation,weight,user_inserted) VALUES
             ('pinyin','c','合','upsert',22,1),
             ('japanese','a','合成未知','upsert',23,1);",
        ).unwrap();
        drop(connection);
        for kind in [Pinyin, Wubi, QuickPhrase, English, Wubi98, Cantonese] {
            let mut expected = vec![
                entry(kind, "a", "合成乙", 12),
                entry(kind, "a", "合成甲", 11),
                entry(kind, "b", "合成丙", 13),
            ];
            if kind == Pinyin {
                expected.push(entry(kind, "x", "合成学习", 21));
            }
            let first =
                personal_dictionary_export_entries_for_kind(&paths, true, kind, 0, 2).unwrap();
            assert_eq!(first.entries, expected[..2]);
            assert!(first.has_more);
            let last =
                personal_dictionary_export_entries_for_kind(&paths, true, kind, 2, 2).unwrap();
            assert_eq!(last.entries, expected[2..]);
            assert!(!last.has_more);
            assert_eq!(
                personal_dictionary_export_entries_for_kind(&paths, true, kind, expected.len(), 1)
                    .unwrap(),
                PersonalDictionaryPage::default()
            );
            let restricted =
                personal_dictionary_export_entries_for_kind(&paths, false, kind, 0, 1000).unwrap();
            assert_eq!(
                restricted.entries,
                if kind.stored_without_main_dictionary() {
                    expected
                } else {
                    Vec::new()
                }
            );
            assert!(!restricted.has_more);
        }
    }

    /// 粤语词只记在日志里：增、改权重、删都只动日志那一行，列表照常列出；回放新代次时跳过它，不会因为不认识而让整次回放回滚。
    #[test]
    fn cantonese_words_live_in_the_journal_and_survive_replay() {
        use PersonalDictionaryKind::*;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let word = entry(Cantonese, "Nei5 Hou2", "你好", 100_000);
        edit_personal_dictionary(&paths, None, Some(&word), "cantonese-add").unwrap();
        let stored = entry(Cantonese, "nei hou", "你好", 100_000);
        assert!(has(&paths, &stored));
        assert_eq!(
            personal_dictionary_entries_for_kind(&paths, true, Cantonese, 0, 10)
                .unwrap()
                .entries,
            vec![stored.clone()]
        );
        // 没有主词库的代次（例如只有粤语的方案集合）同样能列出、编辑粤语词。
        assert_eq!(
            personal_dictionary_entries_with(&paths, false, 0, 10, false)
                .unwrap()
                .entries,
            vec![stored.clone()]
        );
        let heavier = entry(Cantonese, "nei hou", "你好", 200_000);
        edit_personal_dictionary_with(&paths, false, Some(&stored), Some(&heavier), "").unwrap();
        assert!(has(&paths, &heavier));

        let result = replay(
            &paths.user(assets::USER_JOURNAL),
            &paths.dictionary(assets::MAIN_DICTIONARY),
            &paths.dictionary(assets::ENGLISH_DICTIONARY),
        );
        assert!(result.error.is_empty(), "{}", result.error);
        assert_eq!((result.failed, result.skipped), (0, 1));
        assert!(has(&paths, &heavier));

        edit_personal_dictionary(&paths, Some(&heavier), None, "").unwrap();
        assert!(!has(&paths, &heavier));
        assert!(
            personal_dictionary_entries_for_kind(&paths, true, Cantonese, 0, 10)
                .unwrap()
                .entries
                .is_empty()
        );
    }

    #[test]
    fn kind_export_validates_pages_and_reads_without_creating_a_journal() {
        use PersonalDictionaryKind::English;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let journal = paths.user(assets::USER_JOURNAL);
        assert_eq!(
            personal_dictionary_export_entries_for_kind(&paths, true, English, 1_000_000, 1000)
                .unwrap(),
            PersonalDictionaryPage::default()
        );
        assert!(!journal.exists());
        for (offset, limit) in [(0, 0), (0, 1001), (1_000_001, 1)] {
            assert_eq!(
                personal_dictionary_export_entries_for_kind(&paths, true, English, offset, limit)
                    .unwrap_err()
                    .to_string(),
                INVALID_PAGE
            );
        }
        let mut relative = paths.clone();
        relative.user_data = "relative".into();
        assert_eq!(
            personal_dictionary_export_entries_for_kind(&relative, true, English, 0, 1)
                .unwrap_err()
                .to_string(),
            STORAGE_UNAVAILABLE
        );
        std::fs::write(&journal, b"synthetic invalid database").unwrap();
        assert_eq!(
            personal_dictionary_export_entries_for_kind(&paths, true, English, 0, 1)
                .unwrap_err()
                .to_string(),
            DICTIONARY_NOT_READ
        );
    }

    /// English frequency learning lifts a user's own word by `max(listed)+1000` with no ceiling (ranking.rs, J:1027-1031) and keeps it user-inserted, so the listed row can carry a weight above `MAX_ENTRY_WEIGHT`. That row must still be editable and removable: only the replacement is held to the ceiling.
    #[test]
    fn a_learned_english_word_above_the_ceiling_stays_editable() {
        use PersonalDictionaryKind::English;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        let word = entry(English, "foo", "foo", 10);
        edit_personal_dictionary(&paths, None, Some(&word), "").unwrap();
        let learned = 20_000_001_000_i64;
        let journal = open_database(
            &paths.user(assets::USER_JOURNAL),
            OpenFlags::SQLITE_OPEN_READ_WRITE,
        )
        .unwrap();
        journal
            .execute(
                crate::user_dictionary::journal::UPSERT_JOURNAL_SQL,
                params!["english", "foo", "foo", learned, "foo"],
            )
            .unwrap();
        drop(journal);
        let listed = personal_dictionary_entries(&paths, 0, 10, false)
            .unwrap()
            .entries
            .into_iter()
            .find(|entry| entry.key == "foo")
            .unwrap();
        assert_eq!(listed.weight, learned);
        let reweighted = entry(English, "foo", "foo", 500);
        edit_personal_dictionary(&paths, Some(&listed), Some(&reweighted), "").unwrap();
        let listed = personal_dictionary_entries(&paths, 0, 10, false)
            .unwrap()
            .entries;
        assert_eq!(listed, vec![reweighted.clone()]);
        edit_personal_dictionary(&paths, Some(&reweighted), None, "").unwrap();
        assert!(personal_dictionary_entries(&paths, 0, 10, false)
            .unwrap()
            .entries
            .is_empty());
        // The replacement keeps the ceiling.
        let too_heavy = entry(English, "bar", "bar", MAX_ENTRY_WEIGHT + 1);
        assert!(edit_personal_dictionary(&paths, None, Some(&too_heavy), "").is_err());
    }

    /// 由拼音自造词派生的五笔词排在同码已有词之后（`wubi_learning::learn` 取同码最小权重减一，最低到 0），所以列表会返回权重为 0 的用户词。这一行必须能删除、能调权重；新写入的词仍不得低于 1。
    #[test]
    fn a_derived_wubi_word_with_weight_zero_stays_editable() {
        use PersonalDictionaryKind::Wubi;
        let root = tempfile::tempdir().unwrap();
        let paths = fixture(root.path());
        sql(
            &paths.dictionary(assets::MAIN_DICTIONARY),
            "INSERT INTO wubi86 VALUES('abcd','甲',100),('efgh','乙',100),('abef','占位',1);",
        );
        assert!(crate::user_dictionary::wubi_learning::learn(&paths, "甲乙").unwrap());
        let listed = || {
            personal_dictionary_entries(&paths, 0, 10, false)
                .unwrap()
                .entries
        };
        let derived = entry(Wubi, "abef", "甲乙", 0);
        assert_eq!(listed(), vec![derived.clone()]);

        let reweighted = entry(Wubi, "abef", "甲乙", 5);
        edit_personal_dictionary(&paths, Some(&derived), Some(&reweighted), "reweight").unwrap();
        assert_eq!(listed(), vec![reweighted.clone()]);
        edit_personal_dictionary(&paths, Some(&reweighted), Some(&derived), "")
            .expect_err("a new weight stays at 1 or above");

        // 回到权重 0 的状态，再按列表原样删除。
        sql(
            &paths.dictionary(assets::MAIN_DICTIONARY),
            "DELETE FROM wubi86 WHERE value='甲乙'",
        );
        sql(
            &paths.user(assets::USER_JOURNAL),
            "DELETE FROM user_dictionary_operations",
        );
        assert!(crate::user_dictionary::wubi_learning::learn(&paths, "甲乙").unwrap());
        assert_eq!(listed(), vec![derived.clone()]);
        edit_personal_dictionary(&paths, Some(&derived), None, "remove").unwrap();
        assert!(listed().is_empty());
        assert!(!has(&paths, &derived));

        // 存储里没有负权重；这样的“已有行”仍按越界拒绝。
        assert!(validate_existing_entry(&entry(Wubi, "abef", "甲乙", -1)).is_err());
    }

    #[test]
    fn receipt_payloads_keep_the_reference_text() {
        let mut payload = String::new();
        append_payload(&mut payload, None);
        append_payload(
            &mut payload,
            Some(&entry(PersonalDictionaryKind::English, "dont", "don't", 7)),
        );
        assert_eq!(payload, "none;3;7;4:dont5:don't");
        let mut chinese = String::new();
        append_payload(
            &mut chinese,
            Some(&entry(
                PersonalDictionaryKind::Pinyin,
                "ni'hao",
                "你好",
                12345,
            )),
        );
        assert_eq!(chinese, "0;12345;6:ni'hao6:你好");
    }
}
