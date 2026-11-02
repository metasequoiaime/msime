//! Native management requests. The native caller owns and authorizes all paths.

use super::{
    edit_personal_dictionary, invalid_dictionary_entry, require_dictionary_kind, response,
    DictionaryAccess, HostOptions, DICTIONARY_REQUEST_LIMIT,
};
use msime_client_services::dictionary::import::{dictionary_row_matches, PageSelector};
use msime_client_services::dictionary::is_han_character;
use msime_client_services::dictionary::personal::{
    PersonalDictionaryError, PersonalDictionaryStore, PersonalWord, PersonalWordApplied,
    PersonalWordKind, PersonalWordRequestStatus,
};
use msime_engine::host::{DictionaryEntry, DictionaryKind};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::ffi::c_char;
use std::path::Path;

#[derive(Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Pinyin,
    Wubi,
    QuickPhrase,
    English,
    Wubi98,
    /// 用户自己加的粤语词，编码是粤拼音节。只在本机，不进云词库和词库快照。
    Cantonese,
}

/// 浏览类型独立于个人词库类型，资源词条不能流入编辑、导入或云同步协议。`cantonese` 两边都有，浏览时一直指发布的粤语词库：资源排在前面，untagged 按顺序先试它。个人粤语词由 `list` 列出。
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(untagged)]
enum LookupKind {
    Resource(ResourceKind),
    Personal(Kind),
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum ResourceKind {
    Japanese,
    Cantonese,
    Zhuyin,
    Stroke,
    KoreanHanja,
}

impl From<ResourceKind> for msime_engine::host::dictionary_lookup::ResourceDictionaryKind {
    fn from(kind: ResourceKind) -> Self {
        match kind {
            ResourceKind::Japanese => Self::Japanese,
            ResourceKind::Cantonese => Self::Cantonese,
            ResourceKind::Zhuyin => Self::Zhuyin,
            ResourceKind::Stroke => Self::Stroke,
            ResourceKind::KoreanHanja => Self::KoreanHanja,
        }
    }
}

/// Where a listed row comes from. A bundled row shipped with the dictionary (or was learned from typing rather than added): its code and word are fixed, so it can only be given another weight or deleted.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Source {
    User,
    Bundled,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    kind: Kind,
    key: String,
    value: String,
    weight: i64,
    /// Set on every row a list returns, and sent back with it as `previous`. Absent means a user entry, which is what every caller predating bundled rows sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<Source>,
}

/// The refusal for an edit that would change what a bundled row types rather than its weight.
const BUNDLED_READ_ONLY: &str = "bundled dictionary entry is read-only";

impl Entry {
    fn is_bundled(&self) -> bool {
        self.source == Some(Source::Bundled)
    }

    fn with_source(mut self, source: Source) -> Self {
        self.source = Some(source);
        self
    }

    /// A new entry as the Engine will store it: the code lowercased, and a weight inside the range it accepts.
    ///
    /// Both are the Engine's rules (`validate_personal_dictionary_entry`), applied here so a value this layer would otherwise hand on and have refused - with an error naming neither the field nor the reason - becomes the entry the user meant. A weight of zero is the one the reference writes for an imported English row, and losing the row over a rank difference of one would be the worse trade.
    fn normalized_for_engine(self) -> Self {
        let mut entry = self.with_folded_code();
        entry.weight = entry.weight.max(MINIMUM_WEIGHT);
        entry
    }

    /// A stored row as the Engine looks it up: only the code is folded, as the Engine folds it. The weight is matched exactly, and 0 is one a stored row can carry (a wubi word derived from the user's pinyin word ranks below the rows already on its code), so lifting it to the new-entry floor would name a row that is not there.
    fn with_folded_code(mut self) -> Self {
        self.key = self.key.to_ascii_lowercase();
        self
    }

    /// A pinyin entry with its key cut into the syllables the Engine stores, the same way an imported row is.
    ///
    /// The Engine only accepts separated syllables (`ni'hao`), and the reference's settings page accepts `nihao` by normalizing it against the word's length first. Without this the add form refused the way almost everyone types pinyin, with a message that did not say why. A key that cannot be cut is passed on unchanged for the Engine to judge: its refusal names the rule, and a word containing a character outside the Han ranges counted here (`〇`) can still be saved with explicitly separated syllables, as before.
    fn with_full_pinyin(mut self) -> Self {
        if self.kind == Kind::Pinyin {
            if let Some(key) = full_pinyin_key(&self.key, &self.value) {
                self.key = key;
            }
        }
        self
    }

    /// 按种类筛选；非 ASCII 查询按词条包含匹配，其余保留编码前缀规则。
    fn matches(&self, kind: Option<Kind>, prefix: &str) -> bool {
        let same_kind = kind.map(|wanted| wanted == self.kind).unwrap_or(true);
        if !prefix.is_ascii() {
            let word = prefix.trim();
            return same_kind && !word.is_empty() && self.value.contains(word);
        }
        dictionary_row_matches(same_kind, (&self.kind).into(), &self.key, prefix)
    }
}

/// The smallest weight a new entry is stored at; a new entry below it is refused outright. A stored row may still carry 0 (`validate_previous_entry`).
const MINIMUM_WEIGHT: i64 = 1;

impl From<Entry> for DictionaryEntry {
    fn from(entry: Entry) -> Self {
        Self {
            kind: match entry.kind {
                Kind::Pinyin => DictionaryKind::Pinyin,
                Kind::Wubi => DictionaryKind::Wubi,
                Kind::QuickPhrase => DictionaryKind::QuickPhrase,
                Kind::English => DictionaryKind::English,
                Kind::Wubi98 => DictionaryKind::Wubi98,
                Kind::Cantonese => DictionaryKind::Cantonese,
            },
            key: entry.key,
            value: entry.value,
            weight: entry.weight,
        }
    }
}

impl From<Kind> for msime_engine::host::DictionaryKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Pinyin => Self::Pinyin,
            Kind::Wubi => Self::Wubi,
            Kind::QuickPhrase => Self::QuickPhrase,
            Kind::English => Self::English,
            Kind::Wubi98 => Self::Wubi98,
            Kind::Cantonese => Self::Cantonese,
        }
    }
}

impl TryFrom<DictionaryEntry> for Entry {
    type Error = &'static str;

    fn try_from(entry: DictionaryEntry) -> Result<Self, Self::Error> {
        let kind = match entry.kind {
            DictionaryKind::Pinyin => Kind::Pinyin,
            DictionaryKind::Wubi => Kind::Wubi,
            DictionaryKind::QuickPhrase => Kind::QuickPhrase,
            DictionaryKind::English => Kind::English,
            DictionaryKind::Wubi98 => Kind::Wubi98,
            DictionaryKind::Cantonese => Kind::Cantonese,
            _ => return Err("unsupported dictionary kind"),
        };
        Ok(Self {
            kind,
            key: entry.key,
            value: entry.value,
            weight: entry.weight,
            source: None,
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    /// 浏览个人词库与语言资源；空查询也包含内置词条，不改变旧 `list` 的个人导出语义。
    Browse {
        kind: LookupKind,
        #[serde(default)]
        query: String,
        offset: usize,
        limit: usize,
    },
    /// 官方词库独立更新，不需要输入会话释放个人词库。
    Updates {
        command: String,
        enabled: Option<bool>,
    },
    List {
        offset: usize,
        limit: usize,
        /// Restrict the page to one dictionary. Absent means every kind, which
        /// is what older callers sent.
        #[serde(default)]
        kind: Option<Kind>,
        /// ASCII 查询按编码前缀匹配，忽略大小写与拼音分隔符；非 ASCII 查询按词条字面包含匹配。指定种类时非空查询包含内置行，快捷短语空查询也列出内置行；其他空查询只列个人词条。
        #[serde(default)]
        query: Option<String>,
        /// Keep the page to the user's own words even with a kind and a prefix. The mobile personal dictionary queue sets this: its edits cannot carry a bundled row, which may only be re-weighted or deleted in place.
        #[serde(default)]
        user_only: bool,
    },
    Edit {
        previous: Option<Entry>,
        replacement: Option<Entry>,
        request_id: String,
    },
    QueueEdit {
        previous: Option<Entry>,
        replacement: Option<Entry>,
        request_id: String,
    },
    Import {
        kind: Kind,
        format: String,
        text: String,
        request_id: String,
    },
    ImportPersonal {
        text: String,
        request_id: String,
    },
    Reset,
    Export {
        kind: Kind,
        format: String,
        offset: usize,
        limit: usize,
    },
    Retry {
        request_id: String,
    },
    DismissFailure {
        request_id: String,
    },
    /// 词条数，只读。没有 `kind` 时数全部词库。`user_only` 为真时只数用户自己的词；为假（缺省）时拼音还计入学习过权重的内置词，与导出的口径相同。
    Count {
        #[serde(default)]
        kind: Option<Kind>,
        #[serde(default)]
        user_only: bool,
    },
    /// 把用户词库导出成与 `/v1/users/me/dictionary/snapshot` 相同的 NDJSON 文件，`destination` 是绝对路径。`include_learning` 为真时再写上输入记录（本地备份用；云同步不传，输出与原来逐字节相同）。
    ExportSnapshot {
        destination: String,
        #[serde(default)]
        include_learning: bool,
    },
    /// 本机输入记录的条数，只读（`dictionary_snapshot::learning_count`）。
    LearningCount,
    /// 把快照文件 `source` 里的输入记录存成待合并的文件，键盘收起后空闲时合并进本机（`dictionary_snapshot::queue_learning_merge`）。要求 `preferences_directory` 是绝对路径。
    QueueLearningMerge {
        source: String,
    },
    /// 合并 `QueueLearningMerge` 排下的输入记录和 `QueueHabitsMerge` 排下的输入习惯（`dictionary_snapshot::merge_pending_learning`），要独占维护权。要求 `preferences_directory` 是绝对路径。
    MergePendingLearning,
    /// 只读地完整校验一份词库快照文件 `source`（本地备份恢复前先查一遍），返回它的元数据（`dictionary_snapshot::inspect_snapshot`）。
    InspectSnapshot {
        source: String,
    },
    /// 把本机的输入习惯（整句联想、选词对、拼写纠错、自动纠错抑制、置顶）写成 `destination`（`dictionary_snapshot::habits::export_learning_habits`），本地备份用。
    ExportHabits {
        destination: String,
    },
    /// 只读地完整校验一份输入习惯文件 `source`，返回 `{habits}`。
    InspectHabits {
        source: String,
    },
    /// 把输入习惯文件 `source` 存成待合并的文件，键盘收起后空闲时与输入记录一起合并（`dictionary_snapshot::habits::queue_habits_merge`）。要求 `preferences_directory` 是绝对路径。
    QueueHabitsMerge {
        source: String,
    },
    /// 五笔查码，只读：`text` 里每个不同汉字（以及整段作为一个词）在 86 和 98 码表里的编码（[`wubi_codes_json`]）。
    WubiCodes {
        text: String,
    },
}

/// 五笔查码一次接受的文字上限（字节）。查码只列出前 `MAX_WUBI_LOOKUP_CHARACTERS` 个不同汉字，更长的粘贴没有意义。
const MAX_WUBI_LOOKUP_TEXT: usize = 4096;

/// `wubi_codes` 的回答：`{phrase, characters, truncated}`，`phrase` 和每个字都是 `{text, wubi86, wubi98}`，编码短码在前。
///
/// 只读五笔码表（会话用的同一份），不拿词库的维护租约：码表由 SQLite 只读打开，维护进行时读到的是替换前或替换后的完整文件，而一次查码不该让维护等它，也不该在 Android 设置进程里因为键盘进程持有维护而失败。移动端的个人词库队列也直接回答它，因为它不改任何东西。
fn wubi_codes_json(
    options: &msime_engine::host::EngineOptions,
    text: &str,
) -> Result<serde_json::Value, String> {
    if text.len() > MAX_WUBI_LOOKUP_TEXT {
        return Err("invalid wubi lookup".into());
    }
    let lookup = msime_engine::host::wubi_code_lookup(options, text);
    let item = |codes: &msime_engine::host::WubiTextCodes| json!({ "text": codes.text, "wubi86": codes.wubi86, "wubi98": codes.wubi98 });
    Ok(json!({
        "phrase": lookup.phrase.as_ref().map(item),
        "characters": lookup.characters.iter().map(item).collect::<Vec<_>>(),
        "truncated": lookup.truncated,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    options: HostOptions,
    action: Operation,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersonalDictionaryImport {
    format: String,
    version: u32,
    entries: Vec<PersonalWord>,
}

fn parse_personal_dictionary_import(text: &str) -> Result<Vec<PersonalWord>, String> {
    if text.len() > 1_048_576 {
        return Err("personal dictionary file is too large".into());
    }
    let file: PersonalDictionaryImport =
        serde_json::from_str(text).map_err(|_| "invalid personal dictionary file".to_owned())?;
    if file.format != "msime-personal-dictionary" || file.version != 1 {
        return Err("unsupported personal dictionary file".into());
    }
    if file.entries.is_empty() || file.entries.len() > 128 {
        return Err("invalid personal dictionary entry count".into());
    }
    let mut identities = std::collections::HashSet::with_capacity(file.entries.len());
    let mut entries = Vec::with_capacity(file.entries.len());
    for entry in file.entries {
        let entry = normalize_personal_word(entry)?;
        entry
            .validate_new()
            .map_err(|_| "invalid personal dictionary entry".to_owned())?;
        if !identities.insert(entry.identity()) {
            return Err("duplicate personal dictionary entry".into());
        }
        entries.push(entry);
    }
    Ok(entries)
}

/// 一个要排进个人词库导入队列的词，按 `import_personal` 导入文件的规则规范化并检查；`kind` 是导入文件的写法（`pinyin`、`quickPhrase` 等），不合规时为 `None`。云同步合并时用它算出与队列里一致的身份。
pub(crate) fn personal_import_word(
    kind: &str,
    key: &str,
    value: &str,
    weight: i64,
) -> Option<PersonalWord> {
    let kind: PersonalWordKind = serde_json::from_value(json!(kind)).ok()?;
    let word = normalize_personal_word(PersonalWord {
        kind,
        key: key.to_owned(),
        value: value.to_owned(),
        weight,
    })
    .ok()?;
    word.validate_new().ok()?;
    Some(word)
}

/// 一个已经存着的词在个人词库里的身份（权重不参与）：编码按 Engine 对已存词的规则规范（`dictionary_validate_previous`，权重不设上限，英文学习抬过上限的词也认得），规范不了时用原样的写法。云同步合并时用它比较云端的词和本机导出的词，与 [`personal_import_word`] 排进队列的词的身份一致。种类不认识时为 `None`。
pub(crate) fn personal_stored_identity(kind: &str, key: &str, value: &str) -> Option<String> {
    let kind: PersonalWordKind = serde_json::from_value(json!(kind)).ok()?;
    let word = PersonalWord {
        kind,
        key: key.to_owned(),
        value: value.to_owned(),
        weight: 1,
    };
    Some(
        match msime_engine::host::dictionary_validate_previous(&personal_engine_entry(&word)) {
            Ok(entry) => PersonalWord {
                kind: personal_kind(entry.kind),
                key: entry.key,
                value: entry.value,
                weight: 1,
            }
            .identity(),
            Err(_) => word.identity(),
        },
    )
}

/// Native management requests return only redacted errors.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        if request.is_null() || length > DICTIONARY_REQUEST_LIMIT {
            return Err("invalid dictionary buffer".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        dictionary_request_json(bytes)
    })
}

/// The largest `msime_client_settings_dictionary` request: an import carries the whole file, up to `settings_dictionary::MAX_IMPORT_TEXT_BYTES` of text, which JSON escaping can at most double, plus the host options.
const SETTINGS_DICTIONARY_REQUEST_LIMIT: usize =
    2 * crate::settings_dictionary::MAX_IMPORT_TEXT_BYTES + 1024 * 1024;

/// A settings-page dictionary request, `{options, action}` as `msime_client_dictionary` takes it, through `settings_dictionary::request`: an edit, import or reset first has the input hosts let go of the dictionaries, an import of a whole file is sent in batches and answered with one report, and a refusal comes back as the settings pages' code (`dictionary_busy`, `dictionary_invalid_entry`, ...). Blocks until the hosts have let go: call off the UI thread.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_settings_dictionary(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > SETTINGS_DICTIONARY_REQUEST_LIMIT {
            return Err("dictionary_too_large".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let document: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|_| "storage".to_owned())?;
        let (Some(options), Some(action)) = (document.get("options"), document.get("action"))
        else {
            return Err("storage".into());
        };
        crate::settings_dictionary::request(options, action, || {}).map_err(|code| code.to_owned())
    })
}

/// The queued personal dictionary, for a host that cannot take the Engine's
/// maintenance lock when the request arrives.
///
/// `msime_client_dictionary` edits the Engine dictionary directly, which needs
/// the maintenance lock and therefore needs the keyboard not to be holding a
/// session. That is the right route for an edit made in a settings window while
/// nothing is being typed, and the wrong one for importing a file: the user is
/// as likely to do it with the keyboard up, and "dictionary maintenance busy"
/// is not an answer to "please add these words".
///
/// The queue is the answer the Apple and Android hosts already give. Entries go
/// into `<preferences_directory>/PersonalDictionary` and the keyboard applies
/// them the next time it starts a session, which is the one moment it is
/// certain no session is open. This entry point is the same dispatcher those
/// hosts call as Rust, published for the hosts that reach this crate through
/// the C ABI.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_personal_dictionary_request(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // The Apple-compatible import file is bounded at 1 MiB and the request framing needs room
        // on top of it; `personal_dictionary_request_json` applies the same ceiling itself.
        if request.is_null() || length > DICTIONARY_REQUEST_LIMIT {
            return Err("invalid dictionary buffer".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        personal_dictionary_request_json(bytes)
    })
}

/// Validate and normalize one new entry without opening or changing dictionary state.
/// Engine diagnostics are redacted because they can contain submitted text.
/// New input follows the stricter rules: a quick phrase code must be letters only, as in `replacement_for_engine`. A stored row that only identifies what to edit or delete is not passed through here.
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_validate(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 65536 {
            return Err("invalid dictionary buffer".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let entry: Entry =
            serde_json::from_slice(bytes).map_err(|_| "invalid dictionary entry".to_owned())?;
        let normalized = msime_engine::host::dictionary_validate(&entry.into())
            .map_err(|_| "invalid dictionary entry".to_owned())?;
        let normalized = Entry::try_from(normalized)?;
        if matches!(normalized.kind, Kind::QuickPhrase)
            && !msime_client_services::dictionary::quick_phrase_code_is_well_formed(&normalized.key)
        {
            return Err(invalid_dictionary_entry(
                "code contains characters this dictionary does not accept",
            ));
        }
        Ok(json!(normalized))
    })
}

/// Read plain Chinese words, one per line, and answer the pinyin entries they would import as, without opening or changing any dictionary state.
///
/// For a host whose settings surface cannot take the Engine's maintenance lock or run `msime_client_prepare_host` while the keyboard may hold a session: iOS shows the result for confirmation and then queues it itself. The readings come from the packaged main dictionary under `resources`, opened read-only, and follow the same rules as the `hans` import format.
/// # Safety
/// Both pointers must reference readable buffers of the stated lengths. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_hans_entries(
    text: *const u8,
    text_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    response(|| {
        if text.is_null()
            || resources.is_null()
            || text_length > msime_client_core::cloud::dictionary::MAX_IMPORT_BYTES
            || resources_length > 4096
        {
            return Err("invalid dictionary buffer".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let text = std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, text_length) })
            .map_err(|_| "invalid dictionary import")?;
        // SAFETY: guaranteed by the caller contract above.
        let resources =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(resources, resources_length) })
                .map_err(|_| "resources path is not UTF-8")?;
        hans_entries_json(text, resources)
    })
}

fn hans_entries_json(text: &str, resources: &str) -> Result<serde_json::Value, String> {
    let options = read_only_options(resources)?;
    let entries = parse_hans_import(&Kind::Pinyin, text, &options.into_engine_options())?
        .into_iter()
        .map(Entry::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "entries": entries }))
}

/// `msime_client_dictionary_scel_text` 请求（`{bytes_base64}`）的字节上限：细胞词库上限的 base64 长度，再加 4 KiB 给 JSON 外壳。
const SCEL_TEXT_REQUEST_LIMIT: usize = scel_base64_limit() + 4096;

/// 细胞词库上限（`MAX_SCEL_BYTES`）经 base64 编码后的长度。
const fn scel_base64_limit() -> usize {
    msime_client_services::dictionary::scel::MAX_SCEL_BYTES.div_ceil(3) * 4
}

/// 把一个搜狗细胞词库（`.scel`）转成标准 TSV 文本，不碰任何词库状态，也不需要宿主选项。
///
/// 请求是 `{bytes_base64}`，回答 `{name, category, entries, text}`：`text` 每个词一行，读不出读音或词的那一行必定被导入拒绝，所以导入报告里的行号就是词序。宿主随后把 `text` 当作 `standard` 格式交给自己现有的导入（共享设置页、iOS、鸿蒙 2in1、Windows 原生设置），上限、去重、分批和报告都不变。失败时 `error` 是 `dictionary_scel_unsupported`（不是细胞词库或版本不支持）、`dictionary_scel_malformed`（不完整或已损坏）、`dictionary_scel_empty`、`dictionary_too_large` 或 `dictionary_scel_invalid`（请求或 base64 坏了）。
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_scel_text(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() {
            return Err("dictionary_scel_invalid".into());
        }
        if length > SCEL_TEXT_REQUEST_LIMIT {
            return Err("dictionary_too_large".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: ScelTextRequest =
            serde_json::from_slice(bytes).map_err(|_| "dictionary_scel_invalid".to_owned())?;
        scel_text_json(&request.bytes_base64).map_err(str::to_owned)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScelTextRequest {
    bytes_base64: String,
}

/// [`msime_client_dictionary_scel_text`] 的 Rust 入口，Tauri 外壳直接调用；错误是同一组稳定错误码。
pub fn scel_text_json(bytes_base64: &str) -> Result<serde_json::Value, &'static str> {
    use base64::Engine as _;
    use msime_client_services::dictionary::scel::{self, ScelError};
    if bytes_base64.len() > scel_base64_limit() {
        return Err("dictionary_too_large");
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bytes_base64)
        .map_err(|_| "dictionary_scel_invalid")?;
    let converted = scel::decode(&bytes).map_err(|error| match error {
        ScelError::NotScel | ScelError::Unsupported => "dictionary_scel_unsupported",
        ScelError::Malformed => "dictionary_scel_malformed",
        ScelError::Empty => "dictionary_scel_empty",
        ScelError::TooLarge => "dictionary_too_large",
    })?;
    Ok(json!({
        "name": converted.name,
        "category": converted.category,
        "entries": converted.entries,
        "text": converted.text,
    }))
}

/// Parse a dictionary file into the words the personal dictionary queue accepts, with the report the settings page shows for it, without opening or changing any dictionary state.
///
/// `request` is `{kind, format, text}` in the shape of the `import` dictionary action. For the iOS host, whose Tauri settings page queues the words itself in the App Group store the keyboard reads; the readings for `hans` come from the packaged main dictionary under `resources`, opened read-only.
/// # Safety
/// Both pointers must reference readable buffers of the stated lengths. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_import_entries(
    request: *const u8,
    request_length: usize,
    resources: *const u8,
    resources_length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null()
            || resources.is_null()
            || request_length > 1_200_000
            || resources_length > 4096
        {
            return Err("invalid dictionary buffer".into());
        }
        // SAFETY: guaranteed by the caller contract above.
        let bytes = unsafe { std::slice::from_raw_parts(request, request_length) };
        // SAFETY: guaranteed by the caller contract above.
        let resources =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(resources, resources_length) })
                .map_err(|_| "resources path is not UTF-8")?;
        import_entries_json(bytes, resources)
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportEntriesRequest {
    kind: Kind,
    format: String,
    text: String,
}

fn import_entries_json(bytes: &[u8], resources: &str) -> Result<serde_json::Value, String> {
    let request: ImportEntriesRequest =
        serde_json::from_slice(bytes).map_err(|_| "invalid dictionary import".to_owned())?;
    let options = read_only_options(resources)?;
    let (words, mut result) = queued_import(
        &request.kind,
        &request.format,
        &request.text,
        &options.into_engine_options(),
    )?;
    result["entries"] = json!(words
        .into_iter()
        .map(personal_to_entry)
        .collect::<Result<Vec<_>, _>>()?);
    Ok(result)
}

/// Host options that open nothing but the packaged, read-only main dictionary.
fn read_only_options(resources: &str) -> Result<HostOptions, String> {
    if !Path::new(resources).is_absolute() {
        return Err("resources path must be absolute".into());
    }
    // The packaged directory stands in for every runtime path: only the read-only main dictionary is opened.
    serde_json::from_value(json!({
        "api_version": 1,
        "resources": resources,
        "user_data": resources,
        "cache": resources,
        "dictionaries": resources,
        "preferences": msime_client_core::preferences::Preferences::default(),
    }))
    .map_err(|_| "invalid dictionary options".into())
}

/// The personal dictionary queue takes at most this many words per import.
const MAX_QUEUED_IMPORT: usize = 128;

/// 设置页导入一次交给 Engine 的行数，与 `msime_engine::host::dictionary_edit_batch` 的上限（`MAX_EDIT_BATCH`）相同。
const IMPORT_EDIT_BATCH: usize = 128;

/// 在一个事务里写入一块导入行，逐行返回是否写入（重复导入命中回执也算写入）。单行被拒由保存点隔离，不影响同块其他行；整块没能提交时（例如触发器 `RAISE(ROLLBACK)` 撤销了外层事务）这一块退回逐行写入，让一行异常只拒掉它自己，和分批之前一样。
fn import_chunk(
    options: &msime_engine::host::EngineOptions,
    entries: &[DictionaryEntry],
    receipts: &[String],
) -> Vec<bool> {
    let edits: Vec<_> = entries
        .iter()
        .zip(receipts)
        .map(|(entry, receipt)| msime_engine::host::DictionaryEdit {
            previous: None,
            replacement: Some(entry),
            request_id: receipt,
            preserve_existing: false,
        })
        .collect();
    match msime_engine::host::dictionary_edit_batch(options, &edits) {
        Ok(outcomes) => outcomes.iter().map(Result::is_ok).collect(),
        Err(_) => entries
            .iter()
            .zip(receipts)
            .map(|(entry, receipt)| {
                msime_engine::host::dictionary_edit(options, None, Some(entry), receipt).is_ok()
            })
            .collect(),
    }
}

/// A dictionary file as the words the mobile queue accepts, and the import report the settings page shows for it.
///
/// The queue refuses a whole batch for one invalid or repeated word, or for more than 128, where the desktop import applies what it can and names the rest. So rows the Engine would refuse are counted with their line, a repeated word is queued once, and rows past the queue's capacity are reported as truncated: a real file imports on a phone the way it does on a desktop.
fn queued_import(
    kind: &Kind,
    format: &str,
    text: &str,
    options: &msime_engine::host::EngineOptions,
) -> Result<(Vec<PersonalWord>, serde_json::Value), String> {
    let (entries, report) = if format == "hans" {
        (parse_hans_import(kind, text, options)?, None)
    } else {
        let (entries, report) = parse_import(kind, format, text, Some(options))?;
        (entries, Some(report))
    };
    // The engine entry carries no line number; the parsed report lists the same rows in the same order.
    let source_lines = report
        .as_ref()
        .map(|parsed| {
            let mut lines = Vec::with_capacity(parsed.entries.len());
            lines.extend(parsed.entries.iter().map(|entry| entry.line));
            lines
        })
        .unwrap_or_default();
    let mut report = report.unwrap_or(msime_client_services::dictionary::import::ImportReport {
        entries: Vec::new(),
        failed: 0,
        first_failures: Vec::new(),
        truncated: false,
        swapped: false,
    });
    let capacity = entries.len().min(MAX_QUEUED_IMPORT);
    let mut words = Vec::with_capacity(capacity);
    let mut identities = std::collections::HashSet::with_capacity(capacity);
    let mut rejected_lines = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let Ok(word) = normalize_personal_word(PersonalWord {
            kind: personal_kind(entry.kind),
            key: entry.key,
            value: entry.value,
            weight: entry.weight,
        }) else {
            rejected_lines.push(source_lines.get(index).copied().unwrap_or(0));
            continue;
        };
        if !identities.insert(word.identity()) {
            continue;
        }
        if words.len() == MAX_QUEUED_IMPORT {
            report.truncated = true;
            break;
        }
        words.push(word);
    }
    if words.is_empty() {
        return Err("dictionary import rejected".into());
    }
    report.record_rejected(&rejected_lines);
    let result = json!({
        "applied": words.len(),
        "failed": report.failed,
        "truncated": report.truncated,
        "swapped": report.swapped,
        "first_failures": serde_json::to_value(&report.first_failures).map_err(|error| error.to_string())?,
    });
    Ok((words, result))
}

pub fn dictionary_request_json(bytes: &[u8]) -> Result<serde_json::Value, String> {
    if bytes.len() > DICTIONARY_REQUEST_LIMIT {
        return Err("invalid dictionary buffer".into());
    }
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| "invalid dictionary request".to_owned())?;
    if request.options.api_version != 1 {
        return Err("unsupported host API version".into());
    }
    request
        .options
        .preferences
        .validate()
        .map_err(|_| "invalid dictionary options".to_owned())?;
    if let Operation::Updates { command, enabled } = &request.action {
        return crate::dictionary_updates::request(&request.options, command, *enabled);
    }

    // 只有 `queue_learning_merge` 和 `merge_pending_learning` 用到：待合并的输入记录放在它下面。
    let preferences_directory = request
        .options
        .preferences_directory
        .clone()
        .filter(|path| Path::new(path).is_absolute());
    let options = request.options.into_engine_options();
    match request.action {
        Operation::Browse {
            kind,
            query,
            offset,
            limit,
        } => {
            if !(1..=1000).contains(&limit) || offset > 1_000_000 || query.len() > 256 {
                return Err("invalid dictionary page".into());
            }
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            match kind {
                LookupKind::Personal(kind) => {
                    let (entries, has_more) =
                        table_entries_page(&options, kind, query.trim(), offset, limit)?;
                    Ok(json!({ "entries": entries, "has_more": has_more, "available": true }))
                }
                LookupKind::Resource(resource) => {
                    let page = msime_engine::host::dictionary_lookup::dictionary_resource_entries(
                        &options,
                        resource.into(),
                        &query,
                        offset,
                        limit,
                    )
                    .map_err(|_| "dictionary read rejected")?;
                    let entries: Vec<_> = page
                        .entries
                        .into_iter()
                        .map(|row| {
                            json!({
                                "kind": kind, "key": row.key, "value": row.value,
                                "weight": row.weight, "source": "bundled",
                            })
                        })
                        .collect();
                    Ok(
                        json!({ "entries": entries, "has_more": page.has_more, "available": page.available }),
                    )
                }
            }
        }
        Operation::Updates { .. } => Err("dictionary_update_invalid".into()),
        Operation::List {
            offset,
            limit,
            kind,
            query,
            user_only,
        } => {
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            if limit == 0 || limit > 1000 {
                return Err("invalid dictionary page".into());
            }
            let prefix = query.unwrap_or_default();
            if prefix.len() > 256 {
                return Err("invalid dictionary page".into());
            }
            // 非空查询在目标词库内筛选后分页，以便找到并调整内置行；快捷短语允许整表列出，其余空查询仅列个人词条。粤语的发布词在只读的语言词库里（由 `browse` 浏览），这里只列个人粤语词。
            if let Some(kind) = kind.filter(|kind| {
                !user_only
                    && *kind != Kind::Cantonese
                    && (*kind == Kind::QuickPhrase || names_a_code(*kind, &prefix))
            }) {
                let (entries, has_more) =
                    table_entries_page(&options, kind, prefix.trim(), offset, limit)?;
                return Ok(json!({ "entries": entries, "has_more": has_more }));
            }
            let (entries, has_more) = user_entries_page(&options, offset, limit, kind, &prefix)?;
            Ok(json!({ "entries": entries, "has_more": has_more }))
        }
        Operation::Edit {
            previous,
            replacement,
            request_id,
        } => {
            if let Some(previous) = previous.as_ref().filter(|entry| entry.is_bundled()) {
                let weight = bundled_weight(previous, replacement.as_ref())?;
                return edit_bundled_entry(&options, previous, weight, &request_id);
            }
            if replacement.as_ref().is_some_and(Entry::is_bundled) {
                return Err(BUNDLED_READ_ONLY.into());
            }
            // The code is case-insensitive, and the Engine says so by folding it
            // (`validate_personal_dictionary_entry` lowercases the key before it checks anything
            // else); the reference's settings page lowercases it at its own boundary for the same
            // reason. This layer used to reject an uppercase letter instead, so a quick phrase
            // typed as `QQ` came back as "invalid dictionary entry" while the very same code
            // typed in lower case was fine - and an entry edited after being imported from a file
            // that had it in upper case could never be matched to the row it stored.
            let previous = previous.map(Entry::with_folded_code);
            if let Some(entry) = &previous {
                validate_previous_entry(entry)?;
            }
            // Only the replacement is re-cut: the previous entry is a row the list returned, and it has to reach the Engine exactly as stored to be found.
            let replacement = replacement.map(replacement_for_engine).transpose()?;
            let previous = previous.map(DictionaryEntry::from);
            let replacement = replacement.map(DictionaryEntry::from);
            edit_personal_dictionary(
                &options,
                previous.as_ref(),
                replacement.as_ref(),
                &request_id,
            )?;
            Ok(json!({ "applied": true }))
        }
        Operation::Import {
            kind,
            format,
            text,
            request_id,
        } => {
            require_dictionary_kind(&options, kind.into())?;
            let (entries, report) = if format == "hans" {
                (parse_hans_import(&kind, &text, &options)?, None)
            } else {
                let (entries, report) = parse_import(&kind, &format, &text, Some(&options))?;
                (entries, Some(report))
            };
            if !msime_client_core::is_bounded_ascii_identifier(&request_id, 120) {
                return Err("invalid dictionary request ID".into());
            }
            let _access = DictionaryAccess::try_maintenance(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            let mut applied = 0usize;
            // Rows the Engine refuses are counted and named, not fatal. The
            // shared parser only checks the key alphabet and length, while the
            // Engine additionally demands complete pinyin syllables and one
            // syllable per Han character - so ordinary real files (jianpin
            // rows, a two-syllable code on a three-character word) contain
            // some. Aborting discarded the rows already committed and told the
            // user nothing but "check the format", leaving the dictionary
            // half-written with no way to know how far it got.
            let mut rejected_lines = Vec::with_capacity(entries.len());
            // The engine entry carries no line number, so the parsed report is what maps a refused row back to the line the user has to fix. The two lists are built from the same rows in the same order.
            let source_lines = report
                .as_ref()
                .map(|parsed| {
                    let mut lines = Vec::with_capacity(parsed.entries.len());
                    lines.extend(parsed.entries.iter().map(|entry| entry.line));
                    lines
                })
                .unwrap_or_default();
            let receipts: Vec<String> = (0..entries.len())
                .map(|index| format!("{request_id}-{index}"))
                .collect();
            // 已持有维护锁，直接调 Engine。按 Engine 的批量上限分块，每块只开一次库、提交一次事务；逐行开库提交时一个请求的 1000 行在慢盘上会超过输入法让出词库的 30 秒租约（#9918）。
            for (chunk, (rows, ids)) in entries
                .chunks(IMPORT_EDIT_BATCH)
                .zip(receipts.chunks(IMPORT_EDIT_BATCH))
                .enumerate()
            {
                let start = chunk * IMPORT_EDIT_BATCH;
                for (offset, written) in import_chunk(&options, rows, ids).into_iter().enumerate() {
                    if !written {
                        // A hans import has no parsed report, so its rows have no line to name; they are still counted.
                        rejected_lines.push(source_lines.get(start + offset).copied().unwrap_or(0));
                        continue;
                    }
                    applied += 1;
                }
            }
            // Nothing landed and the Engine refused everything: that is a
            // failed import, not a partial one, and the caller should say so.
            if applied == 0 && !rejected_lines.is_empty() {
                return Err("dictionary import rejected".into());
            }
            let mut report =
                report.unwrap_or(msime_client_services::dictionary::import::ImportReport {
                    entries: Vec::new(),
                    failed: 0,
                    first_failures: Vec::new(),
                    truncated: false,
                    swapped: false,
                });
            report.record_rejected(&rejected_lines);
            let mut result = json!({ "applied": applied });
            // Tell the caller what was skipped instead of reporting a clean import.
            result["failed"] = json!(report.failed);
            result["truncated"] = json!(report.truncated);
            // The file turned out to be the other column order; the card says so rather than
            // leaving the user with a count they cannot explain.
            result["swapped"] = json!(report.swapped);
            result["first_failures"] =
                serde_json::to_value(&report.first_failures).map_err(|error| error.to_string())?;
            Ok(result)
        }
        Operation::QueueEdit { .. } | Operation::ImportPersonal { .. } => {
            Err("personal dictionary operation requires the mobile queue".into())
        }
        Operation::Reset => {
            let _access = DictionaryAccess::try_maintenance(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            // 本地备份恢复排下、还没合并的输入记录和输入习惯先删掉，否则键盘空闲时会把清除前的学习并回来。放在重置之前：删不掉时什么也还没清，报错重试即可。
            if let Some(directory) = preferences_directory.as_deref() {
                crate::dictionary_snapshot::discard_pending_learning(Path::new(directory))
                    .map_err(|_| "learned-data reset rejected")?;
            }
            msime_engine::host::reset_learned_data(&options)
                .map_err(|_| "learned-data reset rejected")?;
            // 命名词库的词经个人词库队列写进用户词库，重置把它们一起清掉了，集合却还写着启用。还拿着维护租约时把已启用集合的词重新排进待发送队列，宿主随后照常送出（Android 设置页紧接着 `synchronize`）。没给偏好目录的宿主（桌面）没有命名词库，不排。重置已经做完，这一步失败时不报整个请求失败（宿主会以为什么都没清、不打云同步标记），而是照样回答 `reset:true`，`collections_requeued` 为 null 并带上 `collections_error`，由宿主告诉用户已添加词库的词没能写回。
            let requeued = match preferences_directory.as_deref() {
                Some(directory) => {
                    msime_client_services::dictionary::collections::DictionaryCollectionsStore::new(
                        directory,
                        PersonalDictionaryStore::new(
                            Path::new(directory).join("PersonalDictionary"),
                        ),
                    )
                    .requeue_enabled_after_reset()
                    .map(|view| view.queued.unwrap_or(0))
                }
                None => Ok(0),
            };
            Ok(match requeued {
                Ok(requeued) => json!({ "reset": true, "collections_requeued": requeued }),
                Err(_) => json!({
                    "reset": true,
                    "collections_requeued": null,
                    "collections_error": "learned-data reset collections unavailable",
                }),
            })
        }
        Operation::Export {
            kind,
            format,
            offset,
            limit,
        } => {
            if !matches!(format.as_str(), "standard" | "windows")
                || offset > 1_000_000
                || !(1..=1000).contains(&limit)
            {
                return Err("invalid dictionary export".into());
            }
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            let page = msime_engine::host::dictionary_export_entries_for_kind(
                &options,
                kind.into(),
                offset,
                limit,
            )
            .map_err(|_| "dictionary read rejected")?;
            let has_more = page.has_more;
            let text = page
                .entries
                .into_iter()
                .map(|entry| {
                    if format == "windows" {
                        format!("{}\t{}\t{}", entry.key, entry.value, entry.weight)
                    } else {
                        format!("{}\t{}\t{}", entry.value, entry.key, entry.weight)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let text = if text.is_empty() {
                text
            } else {
                format!("{text}\n")
            };
            Ok(json!({ "text": text, "has_more": has_more }))
        }
        Operation::Retry { .. } | Operation::DismissFailure { .. } => {
            Err("dictionary failure actions require the Android personal dictionary API".into())
        }
        Operation::Count { kind, user_only } => {
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            count_entries(&options, kind, user_only)
        }
        Operation::ExportSnapshot {
            destination,
            include_learning,
        } => {
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            crate::dictionary_snapshot::export_local_snapshot(
                &options,
                Path::new(&destination),
                include_learning,
            )
            .map_err(str::to_owned)
        }
        Operation::LearningCount => {
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            crate::dictionary_snapshot::learning_count(&options).map_err(str::to_owned)
        }
        Operation::QueueLearningMerge { source } => {
            let preferences =
                preferences_directory.ok_or("personal dictionary shared directory unavailable")?;
            crate::dictionary_snapshot::queue_learning_merge(
                Path::new(&preferences),
                Path::new(&source),
            )
            .map_err(str::to_owned)
        }
        Operation::MergePendingLearning => {
            let preferences =
                preferences_directory.ok_or("personal dictionary shared directory unavailable")?;
            crate::dictionary_snapshot::merge_pending_learning(&options, Path::new(&preferences))
                .map_err(str::to_owned)
        }
        Operation::InspectSnapshot { source } => {
            let source = Path::new(&source);
            if !source.is_absolute() {
                return Err("invalid snapshot path".into());
            }
            let metadata =
                crate::dictionary_snapshot::inspect_snapshot(source).map_err(str::to_owned)?;
            serde_json::to_value(metadata).map_err(|_| "invalid snapshot file".to_owned())
        }
        Operation::ExportHabits { destination } => {
            let _access = DictionaryAccess::try_session(
                Path::new(&options.user_data),
                Path::new(&options.dictionaries),
            )
            .map_err(|_| "dictionary access unavailable")?
            .ok_or("dictionary maintenance busy")?;
            crate::dictionary_snapshot::habits::export_learning_habits(
                &options,
                Path::new(&destination),
            )
            .map_err(str::to_owned)
        }
        Operation::InspectHabits { source } => {
            let source = Path::new(&source);
            if !source.is_absolute() {
                return Err("invalid habits path".into());
            }
            crate::dictionary_snapshot::habits::inspect_learning_habits(source)
                .map(|habits| json!({ "habits": habits }))
                .map_err(str::to_owned)
        }
        Operation::QueueHabitsMerge { source } => {
            let preferences =
                preferences_directory.ok_or("personal dictionary shared directory unavailable")?;
            crate::dictionary_snapshot::habits::queue_habits_merge(
                Path::new(&preferences),
                Path::new(&source),
            )
            .map_err(str::to_owned)
        }
        Operation::WubiCodes { text } => wubi_codes_json(&options, &text),
    }
}

/// `count` 的实现：分页扫完整个用户词库，最多扫一百万行。
fn count_entries(
    options: &msime_engine::host::EngineOptions,
    kind: Option<Kind>,
    user_only: bool,
) -> Result<serde_json::Value, String> {
    const CHUNK: usize = 1000;
    const SCAN_LIMIT: usize = 1_000_000;
    let mut counts = std::collections::BTreeMap::<&'static str, u64>::new();
    let mut scanned = 0usize;
    let mut complete = true;
    loop {
        let page = if user_only {
            msime_engine::host::dictionary_entries(options, scanned, CHUNK)
        } else {
            msime_engine::host::dictionary_export_entries(
                options,
                scanned,
                CHUNK,
                kind.is_none_or(|kind| kind == Kind::Pinyin),
            )
        }
        .map_err(|_| "dictionary read rejected")?;
        let count = page.entries.len();
        for entry in page.entries {
            let Ok(entry) = Entry::try_from(entry) else {
                continue;
            };
            if kind.is_some_and(|wanted| wanted != entry.kind) {
                continue;
            }
            *counts.entry(kind_name(entry.kind)).or_default() += 1;
        }
        scanned = scanned.saturating_add(count);
        if !page.has_more || count == 0 {
            break;
        }
        if scanned >= SCAN_LIMIT {
            complete = false;
            break;
        }
    }
    let total: u64 = counts.values().sum();
    Ok(json!({ "count": total, "kinds": counts, "complete": complete }))
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Pinyin => "pinyin",
        Kind::Wubi => "wubi",
        Kind::QuickPhrase => "quick_phrase",
        Kind::English => "english",
        Kind::Wubi98 => "wubi98",
        Kind::Cantonese => "cantonese",
    }
}

/// Android settings use a shared host/keyboard queue rather than editing the
/// Engine while the IME may still own a session. The queue is intentionally a
/// separate entry point so desktop hosts retain their synchronous contract.
pub fn personal_dictionary_request_json(bytes: &[u8]) -> Result<serde_json::Value, String> {
    // JSON imports are bounded by the Apple-compatible 1 MiB file limit; the
    // small amount of request framing needs room in addition to the file.
    if bytes.len() > DICTIONARY_REQUEST_LIMIT {
        return Err("invalid dictionary buffer".into());
    }
    let request: Request =
        serde_json::from_slice(bytes).map_err(|_| "invalid dictionary request".to_owned())?;
    if request.options.api_version != 1 {
        return Err("unsupported host API version".into());
    }
    request
        .options
        .preferences
        .validate()
        .map_err(|_| "invalid dictionary options".to_owned())?;
    if let Operation::Updates { command, enabled } = &request.action {
        return crate::dictionary_updates::request(&request.options, command, *enabled);
    }

    let directory = request
        .options
        .preferences_directory
        .as_deref()
        .filter(|path| Path::new(path).is_absolute())
        .ok_or("personal dictionary shared directory unavailable")?;
    let store = PersonalDictionaryStore::new(Path::new(directory).join("PersonalDictionary"));
    match request.action {
        Operation::Updates { .. } => Err("dictionary_update_invalid".into()),
        // The kind and prefix travel with the page request, and the keyboard answers them from the user's whole store. The entries returned here are the last page it confirmed, which `page_kind` and `page_query` describe.
        Operation::List {
            offset,
            limit,
            kind,
            query,
            ..
        } => {
            if offset > 1_000_000 || !(1..=1000).contains(&limit) {
                return Err("invalid dictionary page".into());
            }
            store
                .request_page(
                    offset,
                    kind.map(|kind| personal_kind(kind.into())),
                    query.as_deref().unwrap_or_default(),
                )
                .map_err(personal_dictionary_error)?;
            let state = store.read().map_err(personal_dictionary_error)?;
            let has_more = state.has_more;
            let pending_count = state.pending_count();
            let snapshot_date = state.snapshot_date.clone();
            let snapshot_error = state.snapshot_error.clone();
            let page_offset = state.page_offset;
            let requested_page_offset = state.requested_page_offset;
            let page_kind = state.page_kind.map(personal_to_kind);
            let page_query = state.page_query.clone();
            let mut failed_requests = Vec::with_capacity(state.requests.len());
            failed_requests.extend(
                state
                    .requests
                    .iter()
                    .filter(|request| request.status == PersonalWordRequestStatus::Failed)
                    .map(|request| {
                        json!({
                            "request_id": request.id,
                            "label": request.replacement.as_ref()
                                .or(request.previous.as_ref())
                                .map(|word| word.value.clone())
                                .unwrap_or_else(|| "词条".to_owned()),
                            "error": request.error.as_deref().unwrap_or("同步失败"),
                        })
                    }),
            );
            let entries = state
                .entries
                .into_iter()
                .map(personal_to_entry)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({
                "entries": entries,
                "has_more": has_more,
                "pending_count": pending_count,
                "snapshot_date": snapshot_date,
                "snapshot_error": snapshot_error,
                "page_offset": page_offset,
                "requested_page_offset": requested_page_offset,
                "page_kind": page_kind,
                "page_query": page_query,
                "failed_requests": failed_requests,
            }))
        }
        Operation::Edit {
            previous,
            replacement,
            request_id,
        }
        | Operation::QueueEdit {
            previous,
            replacement,
            request_id,
        } => {
            let previous = previous.map(personal_from_stored_entry).transpose()?;
            let replacement = replacement.map(personal_from_entry).transpose()?;
            store
                .enqueue(previous, replacement, request_id)
                .map_err(personal_dictionary_error)?;
            let pending_count = store
                .read()
                .map_err(personal_dictionary_error)?
                .pending_count();
            Ok(json!({ "queued": true, "pending_count": pending_count }))
        }
        Operation::Import {
            kind,
            format,
            text,
            request_id,
        } => {
            let options = request.options.into_engine_options();
            let (words, mut result) = queued_import(&kind, &format, &text, &options)?;
            store
                .enqueue_import(words, request_id)
                .map_err(personal_dictionary_error)?;
            let state = store.read().map_err(personal_dictionary_error)?;
            result["queued"] = json!(true);
            result["pending_count"] = json!(state.pending_count());
            Ok(result)
        }
        Operation::ImportPersonal { text, request_id } => {
            let entries = parse_personal_dictionary_import(&text)?;
            store
                .enqueue_import(entries, request_id)
                .map_err(personal_dictionary_error)?;
            let state = store.read().map_err(personal_dictionary_error)?;
            Ok(json!({ "queued": true, "pending_count": state.pending_count() }))
        }
        Operation::Reset => Err("learned-data reset is unavailable on mobile".into()),
        Operation::Export {
            kind,
            format,
            offset,
            limit,
        } => {
            if !matches!(format.as_str(), "standard" | "windows")
                || offset > 1_000_000
                || !(1..=1000).contains(&limit)
            {
                return Err("invalid dictionary export".into());
            }
            let state = store.read().map_err(personal_dictionary_error)?;
            let mut matching = Vec::with_capacity(state.entries.len());
            matching.extend(
                state
                    .entries
                    .into_iter()
                    .filter(|entry| personal_to_kind(entry.kind) == kind),
            );
            let has_more = matching.len() > offset.saturating_add(limit);
            let text = matching
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|entry| {
                    if format == "windows" {
                        format!("{}\t{}\t{}", entry.key, entry.value, entry.weight)
                    } else {
                        format!("{}\t{}\t{}", entry.value, entry.key, entry.weight)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let text = if text.is_empty() {
                text
            } else {
                format!("{text}\n")
            };
            Ok(json!({ "text": text, "has_more": has_more }))
        }
        Operation::Retry { request_id } => {
            store
                .retry(&request_id)
                .map_err(personal_dictionary_error)?;
            let state = store.read().map_err(personal_dictionary_error)?;
            Ok(json!({ "pending_count": state.pending_count() }))
        }
        Operation::Browse { .. }
        | Operation::Count { .. }
        | Operation::ExportSnapshot { .. }
        | Operation::LearningCount
        | Operation::QueueLearningMerge { .. }
        | Operation::MergePendingLearning
        | Operation::InspectSnapshot { .. }
        | Operation::ExportHabits { .. }
        | Operation::InspectHabits { .. }
        | Operation::QueueHabitsMerge { .. } => {
            Err("dictionary read operations require msime_client_dictionary".into())
        }
        Operation::DismissFailure { request_id } => {
            store
                .dismiss_failure(&request_id)
                .map_err(personal_dictionary_error)?;
            let state = store.read().map_err(personal_dictionary_error)?;
            Ok(json!({ "pending_count": state.pending_count() }))
        }
        // 查码只读五笔码表，不经过个人词库队列：Android 的设置页把所有词库操作都送到这里，而这一个不必等键盘空闲。
        Operation::WubiCodes { text } => {
            wubi_codes_json(&request.options.clone().into_engine_options(), &text)
        }
    }
}

/// 查询是否包含编码或词条内容；ASCII 空白与拼音音节分隔符不计入。
fn names_a_code(kind: Kind, prefix: &str) -> bool {
    prefix
        .bytes()
        .any(|byte| !byte.is_ascii_whitespace() && (kind != Kind::Pinyin || byte != b'\''))
}

/// 按编码前缀或词条查询目标词库的一页，标明个人与内置来源；调用方持有词库访问锁。
fn table_entries_page(
    options: &msime_engine::host::EngineOptions,
    kind: Kind,
    prefix: &str,
    offset: usize,
    limit: usize,
) -> Result<(Vec<Entry>, bool), String> {
    let page =
        msime_engine::host::dictionary_table_entries(options, kind.into(), prefix, offset, limit)
            .map_err(|_| "dictionary read rejected")?;
    let entries = page
        .entries
        .into_iter()
        .map(|row| {
            let source = if row.user_inserted {
                Source::User
            } else {
                Source::Bundled
            };
            Entry::try_from(row.entry).map(|entry| entry.with_source(source))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((entries, page.has_more))
}

/// 个人词条分页，支持种类、编码前缀或词条内容筛选；调用方持有词库访问锁。
fn user_entries_page(
    options: &msime_engine::host::EngineOptions,
    offset: usize,
    limit: usize,
    kind: Option<Kind>,
    prefix: &str,
) -> Result<(Vec<Entry>, bool), String> {
    // Unfiltered pages still go straight through, so the common case costs exactly what it did before.
    if kind.is_none() && prefix.is_empty() {
        let page = msime_engine::host::dictionary_entries(options, offset, limit)
            .map_err(|_| "dictionary read rejected")?;
        let entries: Vec<Entry> = page
            .entries
            .into_iter()
            .map(|entry| Entry::try_from(entry).map(|entry| entry.with_source(Source::User)))
            .collect::<Result<_, _>>()?;
        return Ok((entries, page.has_more));
    }
    if prefix.is_empty() {
        if let Some(kind) = kind {
            let page = msime_engine::host::dictionary_entries_for_kind(
                options,
                kind.into(),
                offset,
                limit,
            )
            .map_err(|_| "dictionary read rejected")?;
            let entries: Vec<Entry> = page
                .entries
                .into_iter()
                .map(|entry| Entry::try_from(entry).map(|entry| entry.with_source(Source::User)))
                .collect::<Result<_, _>>()?;
            return Ok((entries, page.has_more));
        }
    }
    // The Engine pages the whole store in one sequence with no kind or prefix filter, so the selection happens here. Doing it on the client meant asking for 100 rows and discarding most of them: a user with more than a page of pinyin words who selected 五笔 saw an empty page 1 even though wubi entries existed.
    let mut selector = PageSelector::new(offset, limit);
    let mut entries: Vec<Entry> = Vec::with_capacity(limit);
    let mut has_more = false;
    let mut scanned = 0usize;
    // Bound the work: a store with very few matches must not turn one request into an unbounded scan. Reaching the budget is reported as "there may be more" rather than silently ending the list.
    const SCAN_BUDGET: usize = 20_000;
    const CHUNK: usize = 500;
    loop {
        let page = msime_engine::host::dictionary_entries(options, scanned, CHUNK)
            .map_err(|_| "dictionary read rejected")?;
        let count = page.entries.len();
        for raw in page.entries {
            let entry = Entry::try_from(raw)?.with_source(Source::User);
            if !entry.matches(kind, prefix) {
                continue;
            }
            if selector.full() {
                has_more = true;
                break;
            }
            if selector.accept() {
                entries.push(entry);
            }
        }
        scanned += count;
        if has_more || count == 0 || !page.has_more {
            break;
        }
        if scanned >= SCAN_BUDGET {
            has_more = true;
            break;
        }
    }
    Ok((entries, has_more))
}

/// 分批把队列应用到 Engine。整批先申请独占维护租约；词库忙碌时不修改队列，调用方稍后重试。
///
/// 请求直接使用宿主传给 `msime_client_create` 的 HostOptions，与 C 头文件及 Android、HarmonyOS 的调用一致。
pub fn personal_dictionary_sync_json(bytes: &[u8]) -> Result<serde_json::Value, String> {
    if bytes.len() > DICTIONARY_REQUEST_LIMIT {
        return Err("invalid dictionary buffer".into());
    }
    let options = serde_json::from_slice(bytes)
        .ok()
        .and_then(HostOptions::from_document)
        .ok_or_else(|| "invalid dictionary request".to_owned())?;
    if options.api_version != 1 {
        return Err("unsupported host API version".into());
    }
    options
        .preferences
        .validate()
        .map_err(|_| "invalid dictionary options".to_owned())?;

    let directory = options
        .preferences_directory
        .as_deref()
        .filter(|path| Path::new(path).is_absolute())
        .ok_or("personal dictionary shared directory unavailable")?;
    let store = PersonalDictionaryStore::new(Path::new(directory).join("PersonalDictionary"));
    let options = options.into_engine_options();
    // 整个有界同步期间都持有维护租约。活动会话时保留 Pending，让调用方在会话销毁后重试。
    let access = DictionaryAccess::try_maintenance(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    personal_dictionary_sync_with_access(&options, &store, &access)
}

/// 调用方持有本批维护租约，个人队列与集合应用之间不能释放再争抢。
fn personal_dictionary_sync_with_access(
    options: &msime_engine::host::EngineOptions,
    store: &PersonalDictionaryStore,
    _access: &DictionaryAccess,
) -> Result<serde_json::Value, String> {
    let mut applied_count = 0usize;
    store
        .synchronize_batch_reporting_present_with_source(
            |requests| {
                let entries: Vec<_> = requests
                    .iter()
                    .map(|queued| {
                        (
                            queued.previous.as_ref().map(personal_engine_entry),
                            queued.replacement.as_ref().map(personal_engine_entry),
                        )
                    })
                    .collect();
                let mut outcomes = Vec::with_capacity(requests.len());
                let mut edit_indexes = Vec::new();
                for (index, queued) in requests.iter().enumerate() {
                    let (previous, replacement) = &entries[index];
                    let result = crate::validate_personal_dictionary_edit(
                        options,
                        previous.as_ref(),
                        replacement.as_ref(),
                        &queued.id,
                    );
                    if let Err(error) = result {
                        outcomes.push(Some(Err(error)));
                        continue;
                    }
                    edit_indexes.push(index);
                    outcomes.push(None);
                }
                let edits: Vec<_> = edit_indexes
                    .iter()
                    .map(|index| msime_engine::host::DictionaryEdit {
                        previous: entries[*index].0.as_ref(),
                        replacement: entries[*index].1.as_ref(),
                        request_id: &requests[*index].id,
                        preserve_existing: requests[*index]
                            .id
                            .starts_with(COLLECTION_REQUEST_PREFIX),
                    })
                    .collect();
                let applied = msime_engine::host::dictionary_edit_batch(options, &edits)
                    .map_err(|_| "dictionary batch was not committed".to_owned())?;
                for (index, result) in edit_indexes.into_iter().zip(applied) {
                    outcomes[index] = Some(
                        result
                            .map(|outcome| match outcome {
                                msime_engine::host::DictionaryApplied::Written => {
                                    applied_count += 1;
                                    PersonalWordApplied::Written
                                }
                                msime_engine::host::DictionaryApplied::AlreadyPresent => {
                                    PersonalWordApplied::AlreadyPresent
                                }
                            })
                            .map_err(|_| "dictionary edit rejected".to_owned()),
                    );
                }
                outcomes
                    .into_iter()
                    .map(|outcome| {
                        outcome.ok_or_else(|| "dictionary batch result missing".to_owned())
                    })
                    .collect()
            },
            // 队列空、宿主没要新页时，日志的戳没动就沿用上次那一页，聚焦时不必再扫日志、重写 sync.json。
            || msime_engine::host::dictionary_entries_source(options),
            |request| {
                let (entries, has_more) = user_entries_page(
                    options,
                    request.offset,
                    100,
                    request.kind.map(personal_to_kind),
                    &request.query,
                )?;
                Ok(
                    msime_client_services::dictionary::personal::PersonalWordPage {
                        entries: entries
                            .into_iter()
                            .map(|entry| PersonalWord {
                                kind: personal_kind(entry.kind.into()),
                                key: entry.key,
                                value: entry.value,
                                weight: entry.weight,
                            })
                            .collect(),
                        has_more,
                    },
                )
            },
        )
        .map_err(personal_dictionary_error)?;
    let state = store.read().map_err(personal_dictionary_error)?;
    Ok(json!({
        "synchronized": true,
        "applied_count": applied_count,
        "pending_count": state.pending_count(),
        "failed_count": state.requests.iter().filter(|request| request.status == PersonalWordRequestStatus::Failed).count(),
        "snapshot_error": state.snapshot_error,
    }))
}

/// `msime_client_services::dictionary::collections` 给它排队的请求所加的 id 前缀。
const COLLECTION_REQUEST_PREFIX: &str = "collections-";

/// 同一有界调用内复用集合状态；每批以同一维护租约覆盖个人队列和集合，批间释放。
pub(crate) fn synchronize_collections_with_budget(
    options: &HostOptions,
    budget: std::time::Duration,
) -> Result<serde_json::Value, String> {
    let engine_options = options.clone().into_engine_options();
    synchronize_collections_with_access(options, budget, || {
        DictionaryAccess::try_maintenance(
            Path::new(&engine_options.user_data),
            Path::new(&engine_options.dictionaries),
        )
        .map_err(|_| "dictionary access unavailable".to_owned())
    })
}

fn synchronize_collections_with_access(
    options: &HostOptions,
    budget: std::time::Duration,
    mut maintenance: impl FnMut() -> Result<Option<DictionaryAccess>, String>,
) -> Result<serde_json::Value, String> {
    let directory = options
        .preferences_directory
        .as_deref()
        .filter(|path| Path::new(path).is_absolute())
        .ok_or("personal dictionary shared directory unavailable")?;
    let personal = PersonalDictionaryStore::new(Path::new(directory).join("PersonalDictionary"));
    let collections =
        msime_client_services::dictionary::collections::DictionaryCollectionsStore::new(
            directory,
            PersonalDictionaryStore::new(Path::new(directory).join("PersonalDictionary")),
        );
    let started = std::time::Instant::now();
    let mut flushing = collections
        .begin_flush()
        .map_err(|error| error.code().to_owned())?;
    let engine_options = options.clone().into_engine_options();
    let mut previous = usize::MAX;
    let mut applied_count = 0u64;
    let mut total_sent = 0usize;
    let mut progress = None;
    loop {
        // 每批只申请一次租约；后批被会话抢先占用时，保留前批已确认的数量和待处理项。
        let Some(access) = maintenance()? else {
            return progress.ok_or_else(|| "dictionary maintenance busy".to_owned());
        };
        let mut synchronized =
            personal_dictionary_sync_with_access(&engine_options, &personal, &access)?;
        applied_count += synchronized["applied_count"].as_u64().unwrap_or(0);
        let view = if budget.is_zero() {
            flushing.flush()
        } else {
            flushing
                .flush_with_engine(|edits| apply_collection_engine_batch(&engine_options, edits))
        }
        .map_err(|error| error.code().to_owned())?;
        drop(access);
        let state = personal.read().map_err(personal_dictionary_error)?;
        let pending = state.pending_count().saturating_add(view.outbox_pending);
        let sent = view.sent.unwrap_or(0);
        total_sent = total_sent.saturating_add(sent);
        synchronized["applied_count"] = json!(applied_count);
        synchronized["pending_count"] = json!(pending);
        synchronized["failed_count"] = json!(state
            .requests
            .iter()
            .filter(|request| request.status == PersonalWordRequestStatus::Failed)
            .count());
        synchronized["sent"] = json!(total_sent);
        if pending == 0 || started.elapsed() >= budget || (sent == 0 && pending >= previous) {
            return Ok(synchronized);
        }
        previous = pending;
        progress = Some(synchronized);
    }
}

fn apply_collection_engine_batch(
    options: &msime_engine::host::EngineOptions,
    edits: &[msime_client_services::dictionary::collections::CollectionEdit],
) -> Result<
    Vec<Result<msime_client_services::dictionary::collections::CollectionEditApplied, String>>,
    String,
> {
    use msime_client_services::dictionary::collections::CollectionEditApplied;

    let entries: Vec<_> = edits
        .iter()
        .map(|edit| {
            (
                edit.previous.as_ref().map(personal_engine_entry),
                edit.replacement.as_ref().map(personal_engine_entry),
            )
        })
        .collect();
    let mut outcomes = vec![None; edits.len()];
    let mut indexes = Vec::new();
    for (index, edit) in edits.iter().enumerate() {
        let (previous, replacement) = &entries[index];
        if let Err(error) = crate::validate_personal_dictionary_edit(
            options,
            previous.as_ref(),
            replacement.as_ref(),
            &edit.request_id,
        ) {
            outcomes[index] = Some(Err(error));
        } else {
            indexes.push(index);
        }
    }
    let engine_edits: Vec<_> = indexes
        .iter()
        .map(|index| msime_engine::host::DictionaryEdit {
            previous: entries[*index].0.as_ref(),
            replacement: entries[*index].1.as_ref(),
            request_id: &edits[*index].request_id,
            preserve_existing: true,
        })
        .collect();
    let applied = if engine_edits.is_empty() {
        Vec::new()
    } else {
        msime_engine::host::dictionary_edit_collection_batch(options, &engine_edits)
            .map_err(|_| "dictionary batch was not committed".to_owned())?
    };
    for (index, result) in indexes.into_iter().zip(applied) {
        outcomes[index] = Some(
            result
                .map(|outcome| match outcome {
                    msime_engine::host::DictionaryApplied::Written => {
                        CollectionEditApplied::Written
                    }
                    msime_engine::host::DictionaryApplied::AlreadyPresent => {
                        CollectionEditApplied::AlreadyPresent
                    }
                })
                .map_err(|_| "dictionary edit rejected".to_owned()),
        );
    }
    outcomes
        .into_iter()
        .map(|outcome| outcome.ok_or_else(|| "dictionary batch result missing".to_owned()))
        .collect()
}

/// JNI entry point for the Android IME worker.
///
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_personal_dictionary_sync(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > DICTIONARY_REQUEST_LIMIT {
            return Err("invalid dictionary buffer".into());
        }
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        personal_dictionary_sync_json(bytes)
    })
}

fn personal_dictionary_error(error: PersonalDictionaryError) -> String {
    error.to_string()
}

fn personal_from_entry(entry: Entry) -> Result<PersonalWord, String> {
    // The queue holds the user's own words; a bundled row is edited in place through `edit`.
    if entry.is_bundled() {
        return Err(BUNDLED_READ_ONLY.into());
    }
    normalize_personal_word(PersonalWord {
        kind: personal_kind(entry.kind.into()),
        key: entry.key,
        value: entry.value,
        weight: entry.weight,
    })
}

/// The row a queued edit or removal starts from. It is one the list returned, so it is normalized by the Engine's rules for a stored row (`dictionary_validate_previous`): its weight may be 0 or past the new-entry ceiling, and refusing it here left such a row impossible to remove through the queue.
fn personal_from_stored_entry(entry: Entry) -> Result<PersonalWord, String> {
    if entry.is_bundled() {
        return Err(BUNDLED_READ_ONLY.into());
    }
    let word = PersonalWord {
        kind: personal_kind(entry.kind.into()),
        key: entry.key,
        value: entry.value,
        weight: entry.weight,
    };
    let entry = msime_engine::host::dictionary_validate_previous(&personal_engine_entry(&word))
        .map_err(|_| "invalid personal dictionary entry".to_owned())?;
    Ok(PersonalWord {
        kind: personal_kind(entry.kind),
        key: entry.key,
        value: entry.value,
        weight: entry.weight,
    })
}

fn normalize_personal_word(word: PersonalWord) -> Result<PersonalWord, String> {
    let entry = msime_engine::host::dictionary_validate(&personal_engine_entry(&word))
        .map_err(|_| "invalid personal dictionary entry".to_owned())?;
    Ok(PersonalWord {
        kind: personal_kind(entry.kind),
        key: entry.key,
        value: entry.value,
        weight: entry.weight,
    })
}

fn personal_engine_entry(word: &PersonalWord) -> msime_engine::host::DictionaryEntry {
    msime_engine::host::DictionaryEntry {
        kind: match word.kind {
            PersonalWordKind::Pinyin => DictionaryKind::Pinyin,
            PersonalWordKind::Wubi => DictionaryKind::Wubi,
            PersonalWordKind::QuickPhrase => DictionaryKind::QuickPhrase,
            PersonalWordKind::English => DictionaryKind::English,
            PersonalWordKind::Wubi98 => DictionaryKind::Wubi98,
            PersonalWordKind::Cantonese => DictionaryKind::Cantonese,
        },
        key: word.key.clone(),
        value: word.value.clone(),
        weight: word.weight,
    }
}

fn personal_to_entry(entry: PersonalWord) -> Result<Entry, String> {
    Ok(Entry {
        kind: personal_to_kind(entry.kind),
        key: entry.key,
        value: entry.value,
        weight: entry.weight,
        source: None,
    })
}

/// The weight a bundled row is set to, or `None` to delete it. Its code and word are what the dictionary shipped, so a replacement may change the weight and nothing else.
fn bundled_weight(previous: &Entry, replacement: Option<&Entry>) -> Result<Option<i64>, String> {
    let Some(replacement) = replacement else {
        return Ok(None);
    };
    if replacement.kind != previous.kind
        || replacement.key != previous.key
        || replacement.value != previous.value
    {
        return Err(BUNDLED_READ_ONLY.into());
    }
    let weight = replacement.weight.max(MINIMUM_WEIGHT);
    if weight > 100_000_000 {
        return Err(invalid_dictionary_entry("weight is outside 1 to 100000000"));
    }
    Ok(Some(weight))
}

/// Re-weight or delete a bundled row under the maintenance lock, journaled so the change survives replay onto a fresh dictionary. Engine diagnostics are withheld, as for a user entry.
fn edit_bundled_entry(
    options: &msime_engine::host::EngineOptions,
    previous: &Entry,
    weight: Option<i64>,
    request_id: &str,
) -> Result<serde_json::Value, String> {
    require_dictionary_kind(options, previous.kind.into())?;
    let _access = DictionaryAccess::try_maintenance(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    if request_id.is_empty() {
        return Err("dictionary request id required".into());
    }
    let previous = DictionaryEntry {
        kind: previous.kind.into(),
        key: previous.key.clone(),
        value: previous.value.clone(),
        weight: previous.weight,
    };
    msime_engine::host::dictionary_edit_bundled(options, &previous, weight, request_id)
        .map_err(|_| "dictionary edit rejected")?;
    Ok(json!({ "applied": true }))
}

fn personal_kind(kind: DictionaryKind) -> PersonalWordKind {
    match kind {
        DictionaryKind::Pinyin => PersonalWordKind::Pinyin,
        DictionaryKind::Wubi => PersonalWordKind::Wubi,
        DictionaryKind::QuickPhrase => PersonalWordKind::QuickPhrase,
        DictionaryKind::English => PersonalWordKind::English,
        DictionaryKind::Wubi98 => PersonalWordKind::Wubi98,
        DictionaryKind::Cantonese => PersonalWordKind::Cantonese,
        _ => unreachable!("unsupported personal dictionary kind"),
    }
}

fn personal_to_kind(kind: PersonalWordKind) -> Kind {
    match kind {
        PersonalWordKind::Pinyin => Kind::Pinyin,
        PersonalWordKind::Wubi => Kind::Wubi,
        PersonalWordKind::Wubi98 => Kind::Wubi98,
        PersonalWordKind::QuickPhrase => Kind::QuickPhrase,
        PersonalWordKind::English => Kind::English,
        PersonalWordKind::Cantonese => Kind::Cantonese,
    }
}

/// The entry a manual add or edit hands to the Engine: the code folded, the weight inside the Engine's range, the host's own bounds checked, and a pinyin key cut into syllables.
fn replacement_for_engine(entry: Entry) -> Result<Entry, String> {
    let entry = entry.normalized_for_engine();
    validate_entry(&entry)?;
    // `validate_entry` also checks the previous row, which may be a stored quick phrase whose code has a digit; new input follows the reference's letters-only rule.
    if matches!(entry.kind, Kind::QuickPhrase)
        && !msime_client_services::dictionary::quick_phrase_code_is_well_formed(&entry.key)
    {
        return Err(invalid_dictionary_entry(
            "code contains characters this dictionary does not accept",
        ));
    }
    Ok(entry.with_full_pinyin())
}

/// The host's bounds on an entry, each refusal naming the rule it broke. The reasons are fixed text and never repeat the entry.
fn validate_entry(entry: &Entry) -> Result<(), String> {
    validate_entry_within(entry, MINIMUM_WEIGHT..=100_000_000)
}

/// The row an edit or removal starts from. It is one the list returned and has to reach the Engine exactly as stored, so its weight is held only to what the store can hold: English frequency learning lifts a user's own word past 100000000 without a ceiling, and a derived wubi word can sit at 0. Holding it to the new-entry range left those rows impossible to edit or remove.
fn validate_previous_entry(entry: &Entry) -> Result<(), String> {
    validate_entry_within(entry, 0..=i64::MAX)
}

fn validate_entry_within(
    entry: &Entry,
    weights: std::ops::RangeInclusive<i64>,
) -> Result<(), String> {
    let key_limit = match entry.kind {
        Kind::Pinyin | Kind::Cantonese => 512,
        Kind::Wubi | Kind::Wubi98 => 4,
        Kind::QuickPhrase => 32,
        Kind::English => 64,
    };
    let key_valid = match entry.kind {
        Kind::Pinyin => {
            msime_client_services::dictionary::pinyin_code_is_well_formed(&entry.key, true)
        }
        Kind::Wubi | Kind::Wubi98 => {
            msime_client_services::dictionary::wubi_code_is_well_formed(&entry.key)
        }
        Kind::QuickPhrase => {
            msime_client_services::dictionary::quick_phrase_transport_code_is_well_formed(
                &entry.key,
            )
        }
        Kind::English => msime_client_services::dictionary::english_code_is_well_formed(&entry.key),
        Kind::Cantonese => {
            msime_client_services::dictionary::cantonese_code_is_well_formed(&entry.key)
        }
    };
    let allowed_controls: &[char] = if matches!(entry.kind, Kind::QuickPhrase) {
        &['\n', '\t']
    } else {
        &[]
    };
    let value_has_invalid_control =
        msime_client_core::has_disallowed_control_with_allowed(&entry.value, allowed_controls);
    let reason = if entry.key.is_empty() || entry.key.len() > key_limit {
        "code is empty or too long"
    } else if !key_valid {
        "code contains characters this dictionary does not accept"
    } else if entry.value.is_empty() || entry.value.len() > 4096 {
        "word is empty or too long"
    } else if value_has_invalid_control {
        "word contains a control character"
    } else if !weights.contains(&entry.weight) {
        "weight is outside 1 to 100000000"
    } else {
        return Ok(());
    };
    Err(invalid_dictionary_entry(reason))
}

/// Cut a full pinyin key into one syllable per Han character of `value`, the form the Engine stores. `None` when the key is not complete syllables or the counts cannot agree.
fn full_pinyin_key(key: &str, value: &str) -> Option<String> {
    let expected_syllables = value
        .chars()
        .filter(|&character| is_han_character(character))
        .count();
    if !(1..=128).contains(&expected_syllables) {
        return None;
    }
    Some(msime_engine::host::normalize_full_pinyin(
        key,
        expected_syllables,
    ))
    .filter(|normalized| !normalized.is_empty())
}

impl From<&Kind> for msime_client_services::dictionary::import::ImportKind {
    fn from(kind: &Kind) -> Self {
        use msime_client_services::dictionary::import::ImportKind;
        match kind {
            Kind::Pinyin => ImportKind::Pinyin,
            Kind::Wubi => ImportKind::Wubi,
            Kind::QuickPhrase => ImportKind::QuickPhrase,
            Kind::English => ImportKind::English,
            Kind::Wubi98 => ImportKind::Wubi98,
            Kind::Cantonese => ImportKind::Cantonese,
        }
    }
}

/// Entries ready for the Engine, plus what the shared parser skipped.
type ParsedImport = (
    Vec<DictionaryEntry>,
    msime_client_services::dictionary::import::ImportReport,
);

/// Parse a submitted dictionary file through the shared parser. Unusable rows
/// are skipped and counted there rather than rejecting the whole file, so the
/// report is returned alongside the entries.
fn parse_import(
    kind: &Kind,
    format: &str,
    text: &str,
    engine_options: Option<&msime_engine::host::EngineOptions>,
) -> Result<ParsedImport, String> {
    let mut report = msime_client_services::dictionary::import::parse(
        kind.into(),
        format,
        text,
        msime_client_core::cloud::dictionary::MAX_IMPORT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    if matches!(kind, Kind::Pinyin) && engine_options.is_some() {
        let mut usable = Vec::with_capacity(report.entries.len());
        for mut entry in report.entries.drain(..) {
            let Some(normalized) = full_pinyin_key(&entry.key, &entry.value) else {
                report.failed += 1;
                if report.first_failures.len() < 5 {
                    report.first_failures.push(
                        msime_client_services::dictionary::import::ImportFailure {
                            line: entry.line,
                            issue: msime_client_services::dictionary::import::ImportIssue::Pinyin,
                        },
                    );
                }
                continue;
            };
            entry.key = normalized;
            usable.push(entry);
        }
        report.entries = usable;
    }
    let entries = report
        .entries
        .iter()
        .map(|entry| DictionaryEntry {
            kind: (*kind).into(),
            key: entry.key.clone(),
            value: entry.value.clone(),
            weight: entry.weight,
        })
        .collect();
    Ok((entries, report))
}

fn parse_hans_import(
    kind: &Kind,
    text: &str,
    options: &msime_engine::host::EngineOptions,
) -> Result<Vec<DictionaryEntry>, String> {
    if !matches!(kind, Kind::Pinyin)
        || text.is_empty()
        || text.len() > msime_client_core::cloud::dictionary::MAX_IMPORT_BYTES
        || text.contains('\0')
        || msime_client_core::has_disallowed_control_with_line_breaks(text)
    {
        return Err("invalid dictionary import".into());
    }
    let mut entries = Vec::with_capacity(1000);
    for line in text.lines() {
        let word = line.trim();
        if word.is_empty() || word.starts_with('#') {
            continue;
        }
        if entries.len() >= 1000 || word.len() > 1024 || !word.chars().all(is_han_character) {
            return Err("invalid dictionary import".into());
        }
        let key = msime_engine::host::hanzi_to_pinyin(options, word);
        if key.is_empty() || key.len() > 256 {
            return Err("dictionary pinyin unavailable".into());
        }
        entries.push(DictionaryEntry {
            kind: DictionaryKind::Pinyin,
            key,
            value: word.to_owned(),
            weight: 10000,
        });
    }
    if entries.is_empty() {
        return Err("invalid dictionary import".into());
    }
    Ok(entries)
}

/// Host options for a caller that manages the dictionary in-process rather than through the C ABI, such as the MCP server. Built from the same runtime-options document a host passes to `msime_client_create`.
pub struct DictionaryOptions(msime_engine::host::EngineOptions);

impl DictionaryOptions {
    /// Parse a runtime-options document. The Linux desktop publishes the candidate skin catalog into the same file, and the Host API refuses that field, so it is dropped here the way the IBus and Fcitx5 hosts drop it.
    pub fn from_host_document(mut document: serde_json::Value) -> Result<Self, String> {
        if let Some(object) = document.as_object_mut() {
            object.remove("candidate_skin_catalog");
        }
        let options = HostOptions::from_document(document)
            .ok_or_else(|| "invalid host options".to_owned())?;
        if options.api_version != 1 {
            return Err("unsupported host API version".into());
        }
        options
            .preferences
            .validate()
            .map_err(|_| "invalid host options".to_owned())?;
        Ok(Self(options.into_engine_options()))
    }

    /// The Engine's user data directory, where the quiesce lease is published.
    pub fn user_data(&self) -> &str {
        &self.0.user_data
    }
}

/// A quick phrase the user added: typing `code` offers `text`. The weight is deliberately absent; it only orders candidates, and a caller has no use for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickPhrase {
    pub code: String,
    pub text: String,
}

pub struct QuickPhrasePage {
    pub phrases: Vec<QuickPhrase>,
    pub has_more: bool,
}

/// One change to the user's quick phrases. `Replace` and `Remove` name the stored phrase by code and text, which is how a caller that never sees weights identifies a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickPhraseEdit {
    Add(QuickPhrase),
    Replace {
        previous: QuickPhrase,
        replacement: QuickPhrase,
    },
    Remove(QuickPhrase),
}

/// The weight a new quick phrase gets, the same one the settings page gives it.
const NEW_QUICK_PHRASE_WEIGHT: i64 = 10;

/// Upper bound on the user store rows one quick-phrase call reads, as for export.
const QUICK_PHRASE_SCAN_LIMIT: usize = 1_000_000;

/// Walk the user's own quick phrases in store order, stopping when `visit` returns false. Only rows the user added: the Engine never learns a quick phrase from typing, and the bundled table is not the user's.
fn scan_user_quick_phrases(
    options: &msime_engine::host::EngineOptions,
    mut visit: impl FnMut(Entry) -> bool,
) -> Result<(), String> {
    const CHUNK: usize = 1000;
    let mut scanned = 0usize;
    while scanned < QUICK_PHRASE_SCAN_LIMIT {
        let page = msime_engine::host::dictionary_entries(options, scanned, CHUNK)
            .map_err(|_| "dictionary read rejected")?;
        let count = page.entries.len();
        for raw in page.entries {
            if raw.kind != DictionaryKind::QuickPhrase {
                continue;
            }
            if !visit(Entry::try_from(raw)?) {
                return Ok(());
            }
        }
        scanned += count;
        if count == 0 || !page.has_more {
            return Ok(());
        }
    }
    Ok(())
}

/// 个人快捷短语分页：ASCII 按编码前缀忽略大小写查询，非 ASCII 按词条内容查询。
pub fn user_quick_phrases(
    options: &DictionaryOptions,
    code_prefix: &str,
    offset: usize,
    limit: usize,
) -> Result<QuickPhrasePage, String> {
    if limit == 0 || limit > 1000 || code_prefix.len() > 256 {
        return Err("invalid dictionary page".into());
    }
    let options = &options.0;
    let _access = DictionaryAccess::try_session(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    let prefix = code_prefix.trim();
    let mut selector = PageSelector::new(offset, limit);
    let mut phrases = Vec::with_capacity(limit);
    let mut has_more = false;
    scan_user_quick_phrases(options, |entry| {
        if !entry.matches(Some(Kind::QuickPhrase), prefix) {
            return true;
        }
        if selector.full() {
            has_more = true;
            return false;
        }
        if selector.accept() {
            phrases.push(QuickPhrase {
                code: entry.key,
                text: entry.value,
            });
        }
        true
    })?;
    Ok(QuickPhrasePage { phrases, has_more })
}

/// The stored row for `phrase`, looked up by its folded code and exact text. The Engine only edits a row it is handed exactly as stored, weight included.
fn stored_quick_phrase(
    options: &msime_engine::host::EngineOptions,
    phrase: &QuickPhrase,
) -> Result<Option<Entry>, String> {
    let _access = DictionaryAccess::try_session(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    let code = phrase.code.to_ascii_lowercase();
    let mut found = None;
    scan_user_quick_phrases(options, |entry| {
        if entry.key == code && entry.value == phrase.text {
            found = Some(entry);
            return false;
        }
        true
    })?;
    Ok(found)
}

fn quick_phrase_entry(phrase: &QuickPhrase, weight: i64) -> Result<Entry, String> {
    replacement_for_engine(Entry {
        kind: Kind::QuickPhrase,
        key: phrase.code.clone(),
        value: phrase.text.clone(),
        weight,
        source: None,
    })
}

/// Apply one change to the user's quick phrases. Busy (another host holds the dictionary) comes back as "dictionary maintenance busy", which the caller retries under the quiesce lease.
pub fn edit_user_quick_phrase(
    options: &DictionaryOptions,
    edit: &QuickPhraseEdit,
    request_id: &str,
) -> Result<(), String> {
    let options = &options.0;
    require_dictionary_kind(options, DictionaryKind::QuickPhrase)?;
    let (previous, replacement) = match edit {
        QuickPhraseEdit::Add(phrase) => {
            let replacement = quick_phrase_entry(phrase, NEW_QUICK_PHRASE_WEIGHT)?;
            // Adding a phrase that is already there would at best rewrite the weight typing gave it.
            if stored_quick_phrase(options, phrase)?.is_some() {
                return Err("quick phrase already exists".into());
            }
            (None, Some(replacement))
        }
        QuickPhraseEdit::Replace {
            previous,
            replacement,
        } => {
            let stored = stored_quick_phrase(options, previous)?.ok_or("quick phrase not found")?;
            let replacement = quick_phrase_entry(replacement, stored.weight)?;
            (Some(stored), Some(replacement))
        }
        QuickPhraseEdit::Remove(phrase) => {
            let stored = stored_quick_phrase(options, phrase)?.ok_or("quick phrase not found")?;
            (Some(stored), None)
        }
    };
    let previous = previous.map(DictionaryEntry::from);
    let replacement = replacement.map(DictionaryEntry::from);
    edit_personal_dictionary(options, previous.as_ref(), replacement.as_ref(), request_id)
}

/// A typing dictionary a word belongs to. Quick phrases have calls of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordKind {
    /// Full pinyin, which double pinyin types from as well.
    Pinyin,
    Wubi,
    English,
    /// 98 五笔码表。
    Wubi98,
}

impl From<WordKind> for Kind {
    fn from(kind: WordKind) -> Self {
        match kind {
            WordKind::Pinyin => Kind::Pinyin,
            WordKind::Wubi => Kind::Wubi,
            WordKind::English => Kind::English,
            WordKind::Wubi98 => Kind::Wubi98,
        }
    }
}

/// A word in a typing dictionary: typing `code` offers `word`, ranked by `weight`. A `bundled` row shipped with the dictionary or was learned from typing; its code and word are fixed, so it can only be given another weight or removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub kind: WordKind,
    pub code: String,
    pub word: String,
    pub weight: i64,
    pub bundled: bool,
}

pub struct WordPage {
    pub words: Vec<Word>,
    pub has_more: bool,
}

/// A word to add. A pinyin code may be left out, and is then the word's most common reading; the weight defaults to the one the settings page gives a new word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewWord {
    pub code: Option<String>,
    pub word: String,
    pub weight: Option<i64>,
}

/// One change to a typing dictionary. A stored row is named by kind, code and word, and its current weight is looked up, which is what the Engine needs to find it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WordEdit {
    Add(WordKind, NewWord),
    SetWeight {
        kind: WordKind,
        code: String,
        word: String,
        weight: i64,
    },
    Remove {
        kind: WordKind,
        code: String,
        word: String,
    },
}

/// What a batch import did. `rejected` pairs the index of each word that was not added with the rule it broke; the reasons are fixed text and never repeat the word.
#[derive(Debug, PartialEq, Eq)]
pub struct WordImport {
    pub added: usize,
    pub existing: usize,
    pub rejected: Vec<(usize, String)>,
}

/// The weight a new word gets, the same one the settings page gives it.
const NEW_WORD_WEIGHT: i64 = 10;

/// The most words one import takes, the same bound as a file import.
const MAX_WORD_IMPORT: usize = 1000;

/// How many table rows under a code are read to find one word. The rows come back with the user's own first and exact codes next, so a word is found early; the bound keeps a short pinyin code from turning into a scan of the whole table.
const WORD_LOOKUP_LIMIT: usize = 20_000;

fn word_from_entry(kind: WordKind, entry: Entry) -> Word {
    Word {
        kind,
        bundled: entry.is_bundled(),
        code: entry.key,
        word: entry.value,
        weight: entry.weight,
    }
}

/// 按编码前缀或词条内容查询输入词库的一页；默认只列个人词条，`include_bundled` 与非空查询会包含内置行。
pub fn dictionary_words(
    options: &DictionaryOptions,
    kind: WordKind,
    code_prefix: &str,
    include_bundled: bool,
    offset: usize,
    limit: usize,
) -> Result<WordPage, String> {
    if limit == 0 || limit > 1000 || code_prefix.len() > 256 {
        return Err("invalid dictionary page".into());
    }
    let options = &options.0;
    let _access = DictionaryAccess::try_session(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    let prefix = code_prefix.trim();
    let (entries, has_more) = if include_bundled && names_a_code(kind.into(), prefix) {
        table_entries_page(options, kind.into(), prefix, offset, limit)?
    } else {
        user_entries_page(options, offset, limit, Some(kind.into()), prefix)?
    };
    Ok(WordPage {
        words: entries
            .into_iter()
            .map(|entry| word_from_entry(kind, entry))
            .collect(),
        has_more,
    })
}

/// A code as the dictionary compares it: folded, and for pinyin without the syllable separators it may or may not be written with.
fn comparable_code(kind: WordKind, code: &str) -> String {
    let code = code.trim().to_ascii_lowercase();
    match kind {
        WordKind::Pinyin => code.chars().filter(|c| !matches!(c, '\'' | ' ')).collect(),
        WordKind::Wubi | WordKind::Wubi98 | WordKind::English => code,
    }
}

/// The stored row typing `code` offers `word` from, the user's own or bundled, exactly as stored. The caller holds dictionary access.
fn stored_word(
    options: &msime_engine::host::EngineOptions,
    kind: WordKind,
    code: &str,
    word: &str,
) -> Result<Option<Entry>, String> {
    let wanted = comparable_code(kind, code);
    if wanted.is_empty() || code.len() > 256 {
        return Err(invalid_dictionary_entry("code is empty or too long"));
    }
    const CHUNK: usize = 1000;
    let mut offset = 0usize;
    while offset < WORD_LOOKUP_LIMIT {
        let (entries, has_more) =
            table_entries_page(options, kind.into(), code.trim(), offset, CHUNK)?;
        let count = entries.len();
        if let Some(entry) = entries
            .into_iter()
            .find(|entry| entry.value == word && comparable_code(kind, &entry.key) == wanted)
        {
            return Ok(Some(entry));
        }
        offset += count;
        if count == 0 || !has_more {
            break;
        }
    }
    Ok(None)
}

/// The entry a new word is stored as, its pinyin code resolved from the word when it has none.
fn new_word_entry(
    options: &msime_engine::host::EngineOptions,
    kind: WordKind,
    word: &NewWord,
) -> Result<Entry, String> {
    let code = match (&word.code, kind) {
        (Some(code), _) => code.clone(),
        (None, WordKind::Pinyin) => {
            if !word.word.chars().all(is_han_character) {
                return Err(invalid_dictionary_entry(
                    "a word without a code must be Han characters only",
                ));
            }
            let reading = msime_engine::host::hanzi_to_pinyin(options, &word.word);
            if reading.is_empty() {
                return Err(invalid_dictionary_entry("the word has no known reading"));
            }
            reading
        }
        (None, _) => return Err(invalid_dictionary_entry("code is empty or too long")),
    };
    replacement_for_engine(Entry {
        kind: kind.into(),
        key: code,
        value: word.word.clone(),
        weight: word.weight.unwrap_or(NEW_WORD_WEIGHT),
        source: None,
    })
}

/// Apply one change to a typing dictionary. Busy (another host holds the dictionary) comes back as "dictionary maintenance busy", which the caller retries under the quiesce lease.
pub fn edit_dictionary_word(
    options: &DictionaryOptions,
    edit: &WordEdit,
    request_id: &str,
) -> Result<(), String> {
    let options = &options.0;
    let kind = match edit {
        WordEdit::Add(kind, _)
        | WordEdit::SetWeight { kind, .. }
        | WordEdit::Remove { kind, .. } => *kind,
    };
    require_dictionary_kind(options, Kind::from(kind).into())?;
    let lookup = |kind: WordKind, code: &str, word: &str| {
        let _access = DictionaryAccess::try_session(
            Path::new(&options.user_data),
            Path::new(&options.dictionaries),
        )
        .map_err(|_| "dictionary access unavailable")?
        .ok_or("dictionary maintenance busy")?;
        stored_word(options, kind, code, word)
    };
    let (kind, code, word, weight) = match edit {
        WordEdit::Add(kind, word) => {
            let replacement = new_word_entry(options, *kind, word)?;
            // Adding a word that is already there would at best rewrite the weight it has.
            if lookup(*kind, &replacement.key, &replacement.value)?.is_some() {
                return Err("word already exists".into());
            }
            let replacement = DictionaryEntry::from(replacement);
            return edit_personal_dictionary(options, None, Some(&replacement), request_id);
        }
        WordEdit::SetWeight {
            kind,
            code,
            word,
            weight,
        } => (*kind, code, word, Some(*weight)),
        WordEdit::Remove { kind, code, word } => (*kind, code, word, None),
    };
    let stored = lookup(kind, code, word)?.ok_or("word not found")?;
    if stored.is_bundled() {
        let replacement = weight.map(|weight| Entry {
            kind: stored.kind,
            key: stored.key.clone(),
            value: stored.value.clone(),
            weight,
            source: None,
        });
        let weight = bundled_weight(&stored, replacement.as_ref())?;
        return edit_bundled_entry(options, &stored, weight, request_id).map(drop);
    }
    let replacement = weight
        .map(|weight| {
            replacement_for_engine(Entry {
                kind: stored.kind,
                key: stored.key.clone(),
                value: stored.value.clone(),
                weight,
                source: None,
            })
        })
        .transpose()?
        .map(DictionaryEntry::from);
    let previous = DictionaryEntry::from(stored);
    edit_personal_dictionary(options, Some(&previous), replacement.as_ref(), request_id)
}

/// Add many words to one dictionary under one hold of the dictionary. A word already there, the user's or bundled, is left as it is and counted; a word the rules refuse is named by index and the rest still go in. Each word is receipted as `{request_id}-{index}`, so replaying the same import after a failure adds nothing twice.
pub fn import_dictionary_words(
    options: &DictionaryOptions,
    kind: WordKind,
    words: &[NewWord],
    request_id: &str,
) -> Result<WordImport, String> {
    if words.is_empty() || words.len() > MAX_WORD_IMPORT {
        return Err("invalid dictionary import".into());
    }
    if !msime_client_core::is_bounded_ascii_identifier(request_id, 120) {
        return Err("invalid dictionary request ID".into());
    }
    let options = &options.0;
    require_dictionary_kind(options, Kind::from(kind).into())?;
    let _access = DictionaryAccess::try_maintenance(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    let mut outcome = WordImport {
        added: 0,
        existing: 0,
        rejected: Vec::with_capacity(words.len()),
    };
    for (index, word) in words.iter().enumerate() {
        let entry = match new_word_entry(options, kind, word) {
            Ok(entry) => entry,
            Err(reason) => {
                outcome.rejected.push((index, reason));
                continue;
            }
        };
        if stored_word(options, kind, &entry.key, &entry.value)?.is_some() {
            outcome.existing += 1;
            continue;
        }
        let receipt = format!("{request_id}-{index}");
        // The import already holds the maintenance lock; call the Engine directly.
        match msime_engine::host::dictionary_edit(options, None, Some(&entry.into()), &receipt) {
            Ok(()) => outcome.added += 1,
            Err(_) => outcome
                .rejected
                .push((index, "dictionary edit rejected".into())),
        }
    }
    Ok(outcome)
}

/// 候选查询所用的方案：候选来自共享词库、按编码查到的那些方案，即 engine 方案码 0 到 2。日文和韩文不在其中：日文候选经由假名读音而不是编码得到，韩文唯一的候选是正在组字的音节对应的汉字，也不是按编码查到的行。粤拼、注音和笔画读各自的语言词库，只有提供这些方案的宿主才会安装；越南文和藏文没有候选。所以这五个方案在这里也都不提供。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupScheme {
    Quanpin,
    Shuangpin,
    Wubi,
}

/// Where a looked-up candidate came from: the Engine's `msime_engine::CandidateSource`, with a dictionary candidate told apart by the row it was found as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateOrigin {
    /// A row shipped with the dictionary or learned from typing.
    Dictionary,
    /// A word the user added.
    UserWord,
    /// Put together by the Engine from dictionary rows, such as a sentence, with no single row of its own.
    Composed,
    English,
    QuickPhrase,
    Emoji,
    Kaomoji,
    Generated,
    Fallback,
    /// 日文混输（实验选项）给整段罗马字补的日文转换，例如 `ohayou` 的おはよう；来自日文模型，不是哪个词库里的一行。
    Japanese,
}

/// A candidate typing a code offers, in the order the Engine ranks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LookupCandidate {
    pub text: String,
    /// The part of the typed code this candidate covers.
    pub code: String,
    pub origin: CandidateOrigin,
    /// The weight of the row a dictionary or user word was found as.
    pub weight: Option<i64>,
}

/// The most candidates one lookup returns.
pub const MAX_LOOKUP_CANDIDATES: usize = 50;

/// The candidates typing `code` offers from the local dictionaries, in the scheme given or the user's own. The lookup types into a session of its own with learning and frequency adjustment off, so it changes nothing the user's typing offers next, and no cloud, AI or sentence-model provider is attached, so the ranking is the Engine's alone.
pub fn lookup_candidates(
    options: &DictionaryOptions,
    scheme: Option<LookupScheme>,
    code: &str,
    limit: usize,
) -> Result<Vec<LookupCandidate>, String> {
    if limit == 0 || limit > MAX_LOOKUP_CANDIDATES {
        return Err(format!(
            "limit must be between 1 and {MAX_LOOKUP_CANDIDATES}"
        ));
    }
    let mut options = options.0.clone();
    if let Some(scheme) = scheme {
        options.scheme = match scheme {
            LookupScheme::Quanpin => 0,
            LookupScheme::Shuangpin => 1,
            LookupScheme::Wubi => 2,
        };
    }
    if !matches!(options.scheme, 0..=2) {
        return Err("candidates can only be looked up in pinyin, double pinyin or wubi".into());
    }
    // 本版本的 Engine 跑不了这个方案（例如五笔版查全拼），直接说明原因，不要让建会话失败报成词库打不开。
    if !msime_engine::SchemeType::from_u8(options.scheme)
        .is_some_and(|scheme| options.enabled_schemes.contains(scheme))
    {
        return Err("this edition does not offer that scheme".into());
    }
    // A semicolon is a key only in double pinyin; elsewhere it is punctuation and would end the composition.
    let allowed = |byte: u8| {
        byte.is_ascii_lowercase() || byte == b'\'' || (byte == b';' && options.scheme == 1)
    };
    if code.is_empty() || code.len() > 64 || !code.bytes().all(allowed) {
        return Err("the code must be 1 to 64 lowercase letters or apostrophes, and semicolons only in double pinyin".into());
    }
    options.learning = false;
    options.frequency_mode = "disabled".into();
    options.local_unicode = false;
    options.local_date_time = false;
    options.local_quick_phrase = false;
    options.local_emoji = false;
    options.local_kaomoji = false;
    options.local_super_jianpin = false;
    options.local_temporary_english = false;
    options.local_temporary_japanese = false;
    options.local_expression = false;
    options.local_command = false;
    options.local_mention = false;
    options.local_chaizi = false;
    // 日文混输照用户的设置留着：它不学习、不改用户数据，查询看到的就是用户打这串键时看到的那一行日文（`msime expand ohayou`）。
    let _access = DictionaryAccess::try_session(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    let session =
        msime_engine::host::Session::new(&options).map_err(|_| "cannot open the dictionaries")?;
    // 每页候选数用会话选项里那一份（用户偏好的每页候选数，`into_engine_options` 已经交给引擎作首屏门槛）：runtime 建立时会把自己的每页候选数交给引擎，两边要是同一个数，查到的才是用户打这串键时看到的那张表。
    let page_size =
        u8::try_from(options.first_page_rows).map_err(|_| "invalid candidate page size")?;
    let mut runtime =
        msime_input_runtime::Runtime::new(session, page_size).map_err(|error| error.to_string())?;
    // An unfocused runtime drops every keystroke.
    runtime.focus(true).map_err(|error| error.to_string())?;
    // 五笔四码唯一时，第四键直接上屏（`input-runtime` 的自动上屏），之后候选列表是空的。这时上屏的那个词就是这串编码给出的唯一候选，要照样报告。`committed` 只留最后一键的上屏，连同它的编码从哪一键开始。
    let mut committed = None;
    let mut start = 0;
    for (index, value) in code.bytes().enumerate() {
        let transition = runtime
            .dispatch(msime_input_runtime::Action::Character {
                value,
                shift: false,
            })
            .map_err(|error| error.to_string())?;
        committed = transition
            .commit
            .map(|text| (code[start..].to_owned(), text));
        if committed.is_some() {
            start = index + 1;
        }
    }
    // The Engine reports a user's word as a dictionary one, so each is looked up as the row it came from.
    let word_kind = if options.scheme == 2 && options.wubi_profile == 1 {
        WordKind::Wubi98
    } else if options.scheme == 2 {
        WordKind::Wubi
    } else {
        WordKind::Pinyin
    };
    let mut candidates: Vec<(String, String, u8)> = runtime
        .all_candidates()
        .candidates
        .into_iter()
        .map(|candidate| (candidate.text, candidate.code, candidate.source))
        .collect();
    if candidates.is_empty() {
        // 自动上屏的只会是五笔词条，按词库候选（source 0）去查它的来源。
        candidates.extend(committed.map(|(code, text)| (text, code, 0)));
    }
    candidates
        .into_iter()
        .take(limit)
        .map(|(text, code, source)| {
            let row = |kind: WordKind| -> Result<Option<Entry>, String> {
                stored_word(&options, kind, &code, &text)
                    .map(|entry| entry.filter(|_| !code.is_empty()))
            };
            let (origin, weight) = match source {
                0 | 1 => match row(word_kind)? {
                    Some(entry) if entry.is_bundled() => {
                        (CandidateOrigin::Dictionary, Some(entry.weight))
                    }
                    Some(entry) => (CandidateOrigin::UserWord, Some(entry.weight)),
                    None => (CandidateOrigin::Composed, None),
                },
                4 => (
                    CandidateOrigin::English,
                    row(WordKind::English)?.map(|entry| entry.weight),
                ),
                5 => (CandidateOrigin::QuickPhrase, None),
                6 => (CandidateOrigin::Emoji, None),
                7 => (CandidateOrigin::Kaomoji, None),
                8 => (CandidateOrigin::Generated, None),
                9 => (CandidateOrigin::Fallback, None),
                13 => (CandidateOrigin::Japanese, None),
                // Cloud (2) and AI (3) suggestions come only from providers this session never has; one here means the lookup is no longer local.
                _ => return Err("the lookup produced a candidate that is not local".to_owned()),
            };
            Ok(LookupCandidate {
                text,
                code,
                origin,
                weight,
            })
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod scel_fixture;
#[cfg(test)]
mod tests;
