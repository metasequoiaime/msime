//! 云同步的纯决策：每个分类这一轮该上传、下载、合并还是不动，什么时候该跑，设置和常用语怎么三方合并，皮肤库能占多少字节，失败怎么告诉用户。不碰网络和磁盘。
//!
//! 节流、模式表、隐私键、皮肤预算和失败文案从 iOS 的 `BackendSyncPolicy` 和 Android 的 `SyncMergePolicy`、`CloudSync.message` 逐条搬来，文案逐字相同。三方合并、合并后修复常用语 id、云端被清空和 `use_cloud` 保护是 #8201 定下的新规则，与两端原来的并集与本机全覆盖不同，差异清单在 PR 正文。

use crate::account::{
    valid_phrase_id, valid_phrases, AccountError, AccountFailure, AccountPhrase, AccountPhrases,
    AccountPreferenceSchema, AccountPreferenceValue, MAX_PHRASES, MAX_PHRASES_BYTES,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// 两次自动同步之间至少隔这么久。
pub const THROTTLE_MS: u64 = 5 * 60 * 1000;
/// 本机有改动时两次同步之间至少隔这么久，连续改几项设置只上传一次。
pub const DIRTY_DEBOUNCE_MS: u64 = 30 * 1000;
/// PUT 回 409 `revision_conflict` 时重新拉取、改按合并再写的次数；超过就放到下一轮。
pub const MAX_CONFLICT_RETRIES: u32 = 2;
/// 一轮同步的租约有效期：超过它还没有提交，别的进程可以开始新的一轮。
pub const LEASE_MS: u64 = 180 * 1000;
/// 服务端 `Retry-After` 退避的上限，与遥测（`telemetry.rs` 的 `MAX_RETRY_AFTER`）相同：更长的按这么长记，离现在比这更远的 `not_before_ms`（时钟往回拨以后）不再挡，手动同步最多被挡这么久。
pub const MAX_BACKOFF_MS: u64 = 24 * 60 * 60 * 1000;
/// 词库合并没做完时第一次退避的时长；之后每多一轮没有进展就翻倍，到 [`MAX_STALL_MS`] 为止。
pub const STALL_BASE_MS: u64 = THROTTLE_MS;
/// 词库合并退避的上限：云端有本机导入不了的词时合并永远做不完，这样每台设备一天最多自动重下几次整份快照。
pub const MAX_STALL_MS: u64 = 6 * 60 * 60 * 1000;
/// 设备本地且涉及隐私、无论如何都不上传也不应用的设置，按键名里的片段识别。
pub const LOCAL_ONLY_FRAGMENTS: [&str; 4] = [
    "incognito",
    "developer_options",
    "diagnostic_log",
    "contribute_audio",
];
/// 整份偏好文档给皮肤库留预算时用的上限：服务端 1 MiB，这里只用到 900 KiB。
pub const DOCUMENT_LIMIT_BYTES: usize = 900 * 1024;
/// 自定义键盘皮肤库所在的键。
pub const SKINS_KEY: &str = crate::account::settings_sync::CUSTOM_KEYBOARD_SKINS;
/// 皮肤库字段在服务端字段表里的 `maxLength`。
pub const SKIN_FIELD_LIMIT: usize = crate::account::settings_sync::MAX_CUSTOM_KEYBOARD_SKINS_BYTES;
/// 个人词库队列一次导入最多的词条数，与 client-services 的 `MAX_QUEUED_IMPORT` 一致。
pub const PERSONAL_IMPORT_BATCH: usize = 128;
/// 合并后为冲突的常用语重新生成的 id 长度。
const GENERATED_PHRASE_ID_LENGTH: usize = 16;
const GENERATED_PHRASE_ID_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const MAX_PHRASE_TEXT_UNITS: usize = 2000;
const MAX_PHRASE_GROUP_UNITS: usize = 32;

/// 同步的分类。`skins` 是设置文档里的皮肤库，与设置共用一个修订号，只有自己的改动标记。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Settings,
    Skins,
    Phrases,
    Dictionary,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::Settings,
        Section::Skins,
        Section::Phrases,
        Section::Dictionary,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Skins => "skins",
            Self::Phrases => "phrases",
            Self::Dictionary => "dictionary",
        }
    }
}

/// 一个分类这一轮做什么。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    None,
    Upload,
    Download,
    Merge,
}

/// 首次同步（或云端被清空后）用户的选择。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Merge,
    UseCloud,
}

/// 宿主带来的用户回答：两个同步选择，加上只在「云端被清空」时提供的「关闭同步，保留本机」（#8201 的决定 8）。
///
/// 「关闭同步」不是一种同步模式，从不进状态文件：[`Choice`] 是能记下来留给以后那一轮的选择（`pending_choice`、词库留下的选择），它只有两种，所以记着的旧选择不会在以后哪一轮悄悄把同步关掉。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    Merge,
    UseCloud,
    /// 这台设备关闭同步：不上传、不下载，也不删任何一边的数据。
    Disable,
}

impl Answer {
    /// 要跑一轮同步的选择；「关闭同步」为 `None`。
    pub fn choice(self) -> Option<Choice> {
        match self {
            Self::Merge => Some(Choice::Merge),
            Self::UseCloud => Some(Choice::UseCloud),
            Self::Disable => None,
        }
    }
}

/// 首次开启：云端没有任何数据就直接上传，否则要等用户选「合并」或「使用云端」，返回 `None` 表示需要问。
pub fn first_run(cloud_has_data: bool, choice: Option<Choice>) -> Option<Mode> {
    if !cloud_has_data {
        return Some(Mode::Upload);
    }
    choice.map(|choice| match choice {
        Choice::Merge => Mode::Merge,
        Choice::UseCloud => Mode::Download,
    })
}

/// 之后每一轮：两边都没变不动；只有本机变了上传；只有云端变了下载；两边都变了合并。
pub fn incremental(cloud_changed: bool, local_dirty: bool) -> Mode {
    match (cloud_changed, local_dirty) {
        (true, true) => Mode::Merge,
        (true, false) => Mode::Download,
        (false, true) => Mode::Upload,
        (false, false) => Mode::None,
    }
}

/// 云端版本是不是和上次同步记下的游标不同；从没记过也算不同。
pub fn cloud_changed(cursor: Option<i64>, revision: i64) -> bool {
    cursor != Some(revision)
}

/// 这一次要不要真的跑：距上次尝试满 [`THROTTLE_MS`]；有分类带着待上传标记时只要满 [`DIRTY_DEBOUNCE_MS`]。没跑过或时钟往回拨时当作已到期。
pub fn due(now_ms: u64, last_attempt_ms: Option<u64>, any_dirty: bool) -> bool {
    let Some(last) = last_attempt_ms.filter(|last| *last <= now_ms) else {
        return true;
    };
    let elapsed = now_ms - last;
    elapsed >= THROTTLE_MS || any_dirty && elapsed >= DIRTY_DEBOUNCE_MS
}

/// 词库合并连着 `rounds` 轮没做完以后要退避多久：第一轮 [`STALL_BASE_MS`]，之后每轮翻倍，不超过 [`MAX_STALL_MS`]。`rounds` 为 0 时按 1 算。
pub fn stall_ms(rounds: u32) -> u64 {
    let shift = rounds.saturating_sub(1).min(16);
    STALL_BASE_MS
        .saturating_mul(1u64 << shift)
        .min(MAX_STALL_MS)
}

/// 首次选择的提示要不要再弹：用户刚打开开关或手动同步时总是弹；否则距上次弹满 [`THROTTLE_MS`] 才再弹。
pub fn prompt_due(now_ms: u64, prompted_at_ms: Option<u64>, force: bool) -> bool {
    match prompted_at_ms {
        Some(prompted) if !force && prompted <= now_ms => now_ms - prompted >= THROTTLE_MS,
        _ => true,
    }
}

/// 「使用云端」的保护：云端从没上传过（revision 0）或内容为空、而本机有数据时，按合并处理（上传本机），不清空本机。
pub fn use_cloud_protected(
    cloud_revision: i64,
    cloud_has_data: bool,
    local_has_data: bool,
) -> bool {
    (cloud_revision == 0 || !cloud_has_data) && local_has_data
}

/// 云端被清空：本机有基线，云端修订号变了（正常只会前进；后退说明云端状态异常，同样按清空处理），内容却是空的（设置 `{}`、常用语 `[]`）。这时不做三方合并也不下载（否则会把「云端删光」传到本机），交给用户选择。
pub fn cloud_cleared(
    base_present: bool,
    cursor: Option<i64>,
    revision: i64,
    cloud_empty: bool,
) -> bool {
    base_present && cursor.is_some_and(|cursor| revision != cursor) && cloud_empty
}

/// 首次开启时个人词库怎么做。快照是整份替换，所以本机没有自己的词时「合并」直接下载云端快照。
pub fn dictionary_first_run(
    cloud_has_data: bool,
    local_has_words: bool,
    choice: Option<Choice>,
) -> Option<Mode> {
    match first_run(cloud_has_data, choice)? {
        Mode::Merge if !local_has_words => Some(Mode::Download),
        mode => Some(mode),
    }
}

/// 上传本机快照之前，个人词库队列里不能还有没被键盘应用的词，否则导出会漏掉它们、上传后云端也跟着丢；下载下来的云端快照还在激活队列里等键盘激活（`snapshot_pending`）时也不能上传，否则导出的是激活前的旧词库，会把云端整份换成它（Android 只看个人词库队列，Rust 以这里为准）。快照没激活就结束了的情况不在这里挡，状态机改按合并处理（`machine::SnapshotActivation::Lost`）。
pub fn upload_blocked(pending_queue_count: usize, snapshot_pending: bool) -> bool {
    pending_queue_count > 0 || snapshot_pending
}

// ---- 设置 ----

pub fn is_local_only(key: &str) -> bool {
    LOCAL_ONLY_FRAGMENTS
        .iter()
        .any(|fragment| key.contains(fragment))
}

/// 去掉设备本地且涉及隐私的键（隐私模式、开发者选项、诊断日志、语音贡献）。
pub fn without_local_only(settings: &mut BTreeMap<String, AccountPreferenceValue>) {
    settings.retain(|key, _| !is_local_only(key));
}

/// 只留服务端字段表收录的键：服务端只要有一个键不在表里就拒绝整份文档，云端文档里 schema 外的键也不写回本机。
pub fn within_schema(
    settings: &mut BTreeMap<String, AccountPreferenceValue>,
    schema: &AccountPreferenceSchema,
) {
    settings.retain(|key, _| schema.fields.contains_key(key));
}

/// 有没有值的类型与字段表不符（`number` 字段也收整数）。
pub fn settings_type_mismatch(
    settings: &BTreeMap<String, AccountPreferenceValue>,
    schema: &AccountPreferenceSchema,
) -> bool {
    settings.iter().any(|(key, value)| {
        schema.fields.get(key).is_some_and(|field| {
            field.value_type != value.kind()
                && !(field.value_type == "number" && value.kind() == "integer")
        })
    })
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// 一个设置值的摘要，基线里只存它。
pub fn value_digest(value: &AccountPreferenceValue) -> String {
    digest(&serde_json::to_vec(value).unwrap_or_default())
}

/// 一份设置的逐键摘要：同步成功时记成基线。
pub fn settings_digests(
    settings: &BTreeMap<String, AccountPreferenceValue>,
) -> BTreeMap<String, String> {
    settings
        .iter()
        .map(|(key, value)| (key.clone(), value_digest(value)))
        .collect()
}

/// 按键三方合并：以云端文档为底，本机自 `base` 以来改过的键用本机的值，其余用云端的值。没有基线（首次合并）时本机导出的键全部胜出，与现有「同一项以本机为准」的提示相同。本机没有导出的键（别的平台的键）保持云端的值。
pub fn merge_settings(
    cloud: &BTreeMap<String, AccountPreferenceValue>,
    local: &BTreeMap<String, AccountPreferenceValue>,
    base: Option<&BTreeMap<String, String>>,
) -> BTreeMap<String, AccountPreferenceValue> {
    let mut merged = cloud.clone();
    for (key, value) in local {
        // 本机的值与基线相同就是本机没改过，这个键跟云端走（云端删掉了它，结果里也没有）。
        if base.is_none_or(|base| base.get(key) != Some(&value_digest(value))) {
            merged.insert(key.clone(), value.clone());
        }
    }
    merged
}

// ---- 常用语 ----

/// 本机一条用户自己的常用语。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalPhrase {
    pub id: String,
    pub text: String,
}

/// 常用语正文的摘要；基线和本机收不下的正文都只存它。
pub fn text_digest(text: &str) -> String {
    digest(text.as_bytes())
}

pub fn text_digests<'a>(texts: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    texts.into_iter().map(text_digest).collect()
}

/// 本机这一侧的正文集合：本机列表，加上云端里本机收不下、没进本机列表的那些（`unheld`）。它们不在本机不是因为被删了，不能当成本机删除。
fn local_side<'a>(
    local: &'a [LocalPhrase],
    cloud: &'a [AccountPhrase],
    unheld: &BTreeSet<String>,
) -> HashSet<&'a str> {
    local
        .iter()
        .map(|phrase| phrase.text.as_str())
        .chain(
            cloud
                .iter()
                .filter(|phrase| unheld.contains(&text_digest(&phrase.text)))
                .map(|phrase| phrase.text.as_str()),
        )
        .collect()
}

/// 本机常用语自上次同步以来有没有改动（按正文集合与基线比较）。漏了一处打标记也能兜住；没有基线时不算，首次同步另有规则。
pub fn phrases_content_dirty(
    local: &[LocalPhrase],
    cloud: &[AccountPhrase],
    base: Option<&BTreeSet<String>>,
    unheld: &BTreeSet<String>,
) -> bool {
    base.is_some_and(|base| {
        let side = local_side(local, cloud, unheld);
        side.len() != base.len() || side.iter().any(|text| !base.contains(&text_digest(text)))
    })
}

/// 按正文三方合并：两边都有的保留（沿用云端的 id 和分组）；只在一侧、而且在基线里的，是另一侧删掉的，按删除处理；只在一侧、不在基线里的，是这一侧新加的，保留。没有基线（首次合并）时取并集。顺序是本机在前，再接云端独有的。
pub fn merge_phrases(
    local: &[LocalPhrase],
    cloud: &[AccountPhrase],
    base: Option<&BTreeSet<String>>,
    unheld: &BTreeSet<String>,
) -> Vec<AccountPhrase> {
    let in_base = |text: &str| base.is_some_and(|base| base.contains(&text_digest(text)));
    let mut by_text: HashMap<&str, &AccountPhrase> = HashMap::with_capacity(cloud.len());
    for phrase in cloud {
        by_text.entry(phrase.text.as_str()).or_insert(phrase);
    }
    let side = local_side(local, cloud, unheld);
    let mut seen = HashSet::new();
    let mut merged = Vec::with_capacity(local.len() + cloud.len());
    for phrase in local {
        if !seen.insert(phrase.text.as_str()) {
            continue;
        }
        match by_text.get(phrase.text.as_str()) {
            Some(cloud) => merged.push((*cloud).clone()),
            None if in_base(&phrase.text) => {}
            None => merged.push(AccountPhrase {
                id: phrase.id.clone(),
                text: phrase.text.clone(),
                group: String::new(),
                position: 0,
            }),
        }
    }
    for phrase in cloud {
        if !seen.insert(phrase.text.as_str()) {
            continue;
        }
        if side.contains(phrase.text.as_str()) || !in_base(&phrase.text) {
            merged.push(phrase.clone());
        }
    }
    merged
}

/// 合并结果为什么不能上传。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhrasesProblem {
    /// 超过 500 条。
    OverLimit,
    /// 请求体超过服务端的 256 KiB。
    TooLarge,
}

impl PhrasesProblem {
    pub fn reason(self) -> &'static str {
        match self {
            Self::OverLimit => "phrases_over_limit",
            Self::TooLarge => "phrases_too_large",
        }
    }
}

/// 一个随机的 16 位 `[A-Za-z0-9_-]` id。
pub fn generated_phrase_id() -> String {
    let mut id = String::with_capacity(GENERATED_PHRASE_ID_LENGTH);
    let bytes = uuid::Uuid::new_v4().into_bytes();
    for byte in bytes.iter().take(GENERATED_PHRASE_ID_LENGTH) {
        id.push(GENERATED_PHRASE_ID_ALPHABET[usize::from(byte & 63)] as char);
    }
    id
}

/// 合并后的修复：去掉服务端会拒的正文（空、含 NUL、超过 2000 个 UTF-16 单元），按正文去重（先出现的保留），分组不合规的清空；id 重复或不合规时先出现的保留、后出现的换一个新生成的 id；position 重排。然后按服务端规则重新校验：超过 500 条是 [`PhrasesProblem::OverLimit`]，请求体超过 256 KiB 是 [`PhrasesProblem::TooLarge`]，两种都不截断，本地和云端都不写。只有空白的正文合法，保留。
pub fn repair_phrases(
    phrases: Vec<AccountPhrase>,
    revision: i64,
    mut new_id: impl FnMut() -> String,
) -> Result<Vec<AccountPhrase>, PhrasesProblem> {
    let mut texts = HashSet::with_capacity(phrases.len());
    let mut ids = HashSet::with_capacity(phrases.len());
    let mut repaired = Vec::with_capacity(phrases.len());
    for mut phrase in phrases {
        if phrase.text.is_empty()
            || phrase.text.contains('\0')
            || !crate::is_bounded_utf16(&phrase.text, MAX_PHRASE_TEXT_UNITS)
            || !texts.insert(phrase.text.clone())
        {
            continue;
        }
        if phrase.group.chars().any(char::is_control)
            || !crate::is_bounded_utf16(&phrase.group, MAX_PHRASE_GROUP_UNITS)
        {
            phrase.group.clear();
        }
        while !valid_phrase_id(&phrase.id) || ids.contains(&phrase.id) {
            phrase.id = new_id();
        }
        ids.insert(phrase.id.clone());
        phrase.position = repaired.len() as i64;
        repaired.push(phrase);
    }
    if repaired.len() > MAX_PHRASES {
        return Err(PhrasesProblem::OverLimit);
    }
    let document = AccountPhrases {
        revision,
        phrases: repaired,
    };
    if !valid_phrases(&document.phrases) {
        return Err(PhrasesProblem::OverLimit);
    }
    if serde_json::to_vec(&document).map_or(usize::MAX, |body| body.len()) > MAX_PHRASES_BYTES {
        return Err(PhrasesProblem::TooLarge);
    }
    Ok(document.phrases)
}

/// 把本机改成 `target` 要做的事：`target` 里有、本机没有的正文要新增；本机有、`target` 里没有的 id 要删掉。本机 id 和云端 id 不通用，所以按正文对齐。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocalPlan {
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

pub fn local_plan(local: &[LocalPhrase], target: &[AccountPhrase]) -> LocalPlan {
    let wanted: HashSet<&str> = target.iter().map(|phrase| phrase.text.as_str()).collect();
    let present: HashSet<&str> = local.iter().map(|phrase| phrase.text.as_str()).collect();
    let mut added = HashSet::new();
    LocalPlan {
        add: target
            .iter()
            .filter(|phrase| {
                !present.contains(phrase.text.as_str()) && added.insert(phrase.text.as_str())
            })
            .map(|phrase| phrase.text.clone())
            .collect(),
        remove: local
            .iter()
            .filter(|phrase| !wanted.contains(phrase.text.as_str()))
            .map(|phrase| phrase.id.clone())
            .collect(),
    }
}

// ---- 皮肤库 ----

/// 皮肤库最多能占的字节数：整份文档 900 KiB 减去其余键已经用掉的，再不超过字段本身的上限。
///
/// `other_document_bytes` 是不含皮肤库键时整份偏好文档编码后的字节数。
pub fn skin_budget(other_document_bytes: usize) -> usize {
    let overhead = SKINS_KEY.len() + 8;
    DOCUMENT_LIMIT_BYTES
        .saturating_sub(other_document_bytes)
        .saturating_sub(overhead)
        .min(SKIN_FIELD_LIMIT)
}

fn library_items(library: &str) -> Option<Vec<serde_json::Value>> {
    match serde_json::from_str(library).ok()? {
        serde_json::Value::Array(items) => Some(items),
        _ => None,
    }
}

/// 皮肤库读不读得懂：是 JSON 数组才算。
pub fn skin_library_readable(library: &str) -> bool {
    library_items(library).is_some()
}

fn updated_at(item: &serde_json::Value) -> u64 {
    item.get("updated_at")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

/// 放不下全部设计时只留最近的：按更新时间从新到旧，装到预算为止（与 Android 的 `CustomSkinLibrary.exportDesigns` 相同，装不下的第一个之后都不要）。返回裁剪后的 JSON、总数和留下的个数；不是 JSON 数组时为 `None`。
pub fn trim_skin_library(library: &str, budget: usize) -> Option<(String, usize, usize)> {
    let mut items = library_items(library)?;
    let total = items.len();
    items.sort_by_key(|item| std::cmp::Reverse(updated_at(item)));
    let mut kept = Vec::with_capacity(items.len());
    let mut used = 2usize;
    for item in items {
        let size = item.to_string().len() + usize::from(!kept.is_empty());
        if used + size > budget {
            break;
        }
        used += size;
        kept.push(item);
    }
    let count = kept.len();
    Some((serde_json::Value::Array(kept).to_string(), total, count))
}

/// 合并两份皮肤库：按 id 对齐，同一 id 取更新时间更晚的，相同时本机胜出；先本机、再接云端独有的。没有字符串 id 的项丢掉。任一份不是 JSON 数组时为 `None`。
pub fn merge_skin_libraries(local: &str, cloud: &str) -> Option<String> {
    let local = library_items(local)?;
    let cloud = library_items(cloud)?;
    let id = |item: &serde_json::Value| {
        item.get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut merged: Vec<serde_json::Value> = Vec::with_capacity(local.len() + cloud.len());
    let mut index: HashMap<String, usize> = HashMap::new();
    for item in local {
        if let Some(key) = id(&item) {
            if let std::collections::hash_map::Entry::Vacant(entry) = index.entry(key) {
                entry.insert(merged.len());
                merged.push(item);
            }
        }
    }
    for item in cloud {
        let Some(key) = id(&item) else { continue };
        match index.get(&key) {
            Some(&position) if updated_at(&item) > updated_at(&merged[position]) => {
                merged[position] = item;
            }
            Some(_) => {}
            None => {
                index.insert(key, merged.len());
                merged.push(item);
            }
        }
    }
    Some(serde_json::Value::Array(merged).to_string())
}

// ---- 个人词库 ----

/// 云端快照里的一个用户词，`kind` 已换成个人词库导入文件的写法。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Word {
    pub kind: String,
    pub key: String,
    pub value: String,
    pub weight: i64,
}

/// 快照里的词库种类换成个人词库导入文件的写法；不认识的种类为 `None`，调用方跳过这个词。云端快照和本机导出写的都是 `quick`。
pub fn personal_kind(snapshot_kind: &str) -> Option<&'static str> {
    match snapshot_kind {
        "pinyin" => Some("pinyin"),
        "wubi" => Some("wubi"),
        "wubi98" => Some("wubi98"),
        "english" => Some("english"),
        "quick" | "quick_phrase" | "quickPhrase" => Some("quickPhrase"),
        _ => None,
    }
}

/// 把要导入的词按 种类 + 编码 + 词 去重，再切成每批不超过 `size` 条（导入文件里有重复会整批失败）。`size` 为 0 时按 1 处理。
pub fn batches(words: Vec<Word>, size: usize) -> Vec<Vec<Word>> {
    let size = size.max(1);
    let mut seen = HashSet::with_capacity(words.len());
    let mut result = Vec::new();
    let mut current = Vec::with_capacity(size.min(words.len()));
    for word in words {
        let Some(kind) = personal_kind(&word.kind) else {
            continue;
        };
        if word.key.is_empty()
            || word.value.is_empty()
            || !seen.insert((kind, word.key.clone(), word.value.clone()))
        {
            continue;
        }
        current.push(Word {
            kind: kind.to_owned(),
            ..word
        });
        if current.len() == size {
            result.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

// ---- 失败与状态行 ----

pub const NETWORK_MESSAGE: &str = "网络不可用，稍后自动重试";
/// 云端被清空、等用户选择时的状态行。
pub const CLOUD_CLEARED_LINE: &str = "云端数据已被清空，请选择合并、使用云端或关闭同步";
/// 用户在云端被清空时选了「关闭同步，保留本机」以后的状态行。
pub const DISABLED_ON_CLOUD_CLEARED_LINE: &str = "云端数据已被清空，已关闭同步，本机数据没有改动";
const SIGNED_OUT_MESSAGE: &str = "登录已失效，请重新登录后再同步";
const BUSY_MESSAGE: &str = "云端暂时繁忙，稍后自动重试";

/// 这次失败是不是 CAS 冲突：别的设备在我们读之后先写了。
pub fn is_conflict(status: Option<u16>, code: Option<&str>) -> bool {
    status == Some(409) && code == Some("revision_conflict")
}

/// 一次联网失败展示给用户的原因，措辞与 iOS 的 `BackendSyncPolicy.message` 和 Android 的 `CloudSync.message` 相同；`None` 表示这一轮作废（跑的过程中退出登录或换了账号），不写进状态。没拿到响应的失败按网络不可用说。
pub fn failure_message(failure: &AccountFailure) -> Option<String> {
    let code = failure.server_code.as_deref();
    let Some(status) = failure.status else {
        return match failure.error {
            AccountError::Cancelled => None,
            AccountError::Unauthorized => Some(SIGNED_OUT_MESSAGE.to_owned()),
            _ => Some(NETWORK_MESSAGE.to_owned()),
        };
    };
    if status == 401 {
        return Some(SIGNED_OUT_MESSAGE.to_owned());
    }
    if status == 503 && matches!(code, Some("provider_disabled" | "service_disabled")) {
        return Some("云同步暂时不可用".to_owned());
    }
    let known = match code {
        Some(code @ ("invalid_dictionary_snapshot" | "invalid_dictionary_entry")) => Some(format!(
            "词库里有云端暂不支持的词条，词库没有同步（{code}）"
        )),
        Some("dictionary_limit") => Some("云端词库最多保存 10 万条，词库没有同步".to_owned()),
        Some("snapshot_too_large") => Some("词库太大，无法同步到云端".to_owned()),
        Some(code @ ("invalid_preference_field" | "invalid_preferences")) => {
            Some(format!("有设置项云端不认识，设置没有同步（{code}）"))
        }
        Some("invalid_phrases") => Some(local_message("phrases_over_limit").to_owned()),
        Some("account_banned") => Some("账号已被停用，无法同步".to_owned()),
        Some("rate_limit_exceeded") => Some("同步太频繁，稍后自动重试".to_owned()),
        Some(
            "snapshot_restore_busy"
            | "engine_unavailable"
            | "auth_unavailable"
            | "dictionary_timeout",
        ) => Some(BUSY_MESSAGE.to_owned()),
        _ => None,
    };
    if known.is_some() {
        return known;
    }
    if status == 429 || status >= 500 {
        return Some(format!("{BUSY_MESSAGE}（HTTP {status}）"));
    }
    Some(match code {
        Some(code) => format!("同步失败（HTTP {status} {code}）"),
        None => format!("同步失败（HTTP {status}）"),
    })
}

/// 本机这一侧的失败原因（稳定的 `reason` 码）给用户看的话。不认识的码按分类说「同步失败，稍后自动重试」。
pub fn local_message(reason: &str) -> &'static str {
    match reason {
        "phrases_over_limit" => {
            "常用语超出云端限制（最多 500 条，每条最多 2000 字），常用语没有同步"
        }
        "phrases_too_large" => "常用语超出云端 256 KiB 的大小限制，常用语没有同步",
        "schema_type_mismatch" => "有设置项的类型与云端字段表不符，设置没有同步",
        "settings_too_large" => "设置超出云端的大小限制，设置没有同步",
        "invalid_response" => BUSY_MESSAGE,
        "settings_storage" => "设置同步失败，稍后自动重试",
        "phrases_storage" => "常用语同步失败，稍后自动重试",
        "dictionary_storage" => "词库同步失败，稍后自动重试",
        "dictionary_merge_pending" => "云端的词还在排队导入本机，用过键盘以后会接着同步",
        "dictionary_merge_rejected" => {
            "云端词库里有本机导入不了的词，为了不从云端删掉它们，词库暂停同步"
        }
        "dictionary_merge_blocked" => "个人词库里有没保存成功的修改，处理以后词库才会接着同步",
        _ => "同步失败，稍后自动重试",
    }
}

/// 「我的」页同步行的一行状态，措辞与 iOS 的 `BackendSyncPolicy.statusLine` 和 Android 的 `CloudSync.statusLine` 相同：关闭、最近一次失败的原因、云端被清空等待选择、皮肤库被裁剪、云端常用语本机收不下、尚未同步，或「已同步 · 最近 …」。两端没有的两句是 #8201 的决定 8 加的：云端被清空时多了「关闭同步」这个选择，等待选择那一句把它列上；用户选了它以后关闭的那一句说明为什么关着。
pub struct StatusLine<'a> {
    pub enabled: bool,
    /// 开关是用户在「云端被清空」时选了「关闭同步，保留本机」关掉的，之后还没有再开关过。
    pub disabled_on_cloud_cleared: bool,
    pub failure: Option<&'a str>,
    pub cloud_cleared: bool,
    pub skins_trimmed: bool,
    pub unheld_phrases: usize,
    pub last_synced_ms: Option<u64>,
    pub now_ms: u64,
}

impl StatusLine<'_> {
    pub fn render(&self) -> String {
        if !self.enabled {
            if self.disabled_on_cloud_cleared {
                return DISABLED_ON_CLOUD_CLEARED_LINE.to_owned();
            }
            return "云同步已关闭".to_owned();
        }
        if let Some(failure) = self.failure.filter(|failure| !failure.is_empty()) {
            return failure.to_owned();
        }
        if self.cloud_cleared {
            return CLOUD_CLEARED_LINE.to_owned();
        }
        if self.skins_trimmed {
            return "自定义皮肤太多，只同步了最近的设计".to_owned();
        }
        if self.unheld_phrases > 0 {
            return format!(
                "云端有 {} 条常用语超出本机上限，未下载但已保留",
                self.unheld_phrases
            );
        }
        match self.last_synced_ms {
            None => "云同步已开启 · 尚未同步".to_owned(),
            Some(then) => format!("已同步 · 最近 {}", ago(self.now_ms, then)),
        }
    }
}

/// 「已同步 · 最近 …」里的相对时间，与 Android 的 `RelativeTimePolicy.minutesAgo` 相同。
pub fn ago(now_ms: u64, then_ms: u64) -> String {
    let minutes = now_ms.saturating_sub(then_ms) / 60_000;
    if minutes < 1 {
        return "刚刚".to_owned();
    }
    if minutes < 60 {
        return format!("{minutes} 分钟前");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours} 小时前");
    }
    format!("{} 天前", hours / 24)
}

#[cfg(test)]
mod tests;
