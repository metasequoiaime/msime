//! 同步状态文件：`<state_directory>/cloud-sync/state.json`，版本 2，上限 256 KiB；读改写都在同目录 `state.lock` 的 flock 里完成，持锁期间不发网络请求。键盘扩展和 Android 的 `:ime` 进程只经它给分类打待上传标记，所以它必须跨进程。
//!
//! 基线（`base`）和本机收不下的常用语（`unheld`）只存 SHA-256，不存原文。一轮同步开始时在锁里写下租约 `run`，提交时按租约 id、账号和 `state_version` 比较并交换：期间有别的写入（退出登录、开关、另一轮）就是 `stale_run`，什么都不推进。打待上传标记和记下用户的选择不推进 `state_version`，不会让进行中的那一轮作废。
//!
//! 一轮里要交给宿主写的值没有写完之前，要提交的进度放在旁边的 `pending.json`（上限 4 MiB，iOS 的设置文档可能带着 800 KB 的自定义键盘皮肤），提交时一起读出。

use super::policy::{Choice, Mode, Section, LEASE_MS, MAX_BACKOFF_MS, MAX_STALL_MS};
use crate::account::AccountPreferences;
use crate::file_lock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const STATE_VERSION: u32 = 2;
/// 状态文件的字节上限。
pub const MAX_STATE_BYTES: usize = 256 * 1024;
/// 待提交进度文件的字节上限。
pub const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;
/// 状态目录在宿主给的 `state_directory` 下面的名字。
pub const DIRECTORY_NAME: &str = "cloud-sync";
const STATE_FILE: &str = "state.json";
const LOCK_FILE: &str = "state.lock";
const PENDING_FILE: &str = "pending.json";

/// 一个分类的进度。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionState {
    /// 上次同步到的云端修订号；`None` 表示还没同步过，下一轮按首次处理。
    pub cursor: Option<i64>,
    /// 本机改动代数，每打一次标记加一。
    pub generation: u64,
    /// 上次清掉待上传标记时的代数；与 `generation` 不同即有改动还没上传。
    pub clean: u64,
    /// 上次同步成功时两边一致的内容的摘要：设置是逐键摘要，常用语是正文摘要的集合。三方合并靠它判断哪一侧改过。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<SectionBase>,
    /// 上次应用云端常用语时本机收不下的正文的摘要。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unheld: Vec<String>,
    /// 云端被清空了，等用户选择合并或使用云端；选择之前这一类按首次处理。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cleared: bool,
    /// 只对词库：下载下来交给激活队列、还没确认激活的云端快照的请求 id。游标在下载时就记成了快照的修订号，这份快照最后没有激活（冲突、失败、取消）时，下一轮靠它知道本机缺着那些词，改按合并处理而不是上传。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation: Option<String>,
    /// 只对词库：首次同步（或云端被清空以后）用户选了、合并却还没做完时留下的选择，下一轮词库接着用，不再问。它只给词库用，不进 `pending_choice`：设置和常用语这时多半已经同步过了，以后它们的云端被清空还要重新问用户。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choice: Option<Choice>,
    /// 只对词库：合并一直没做完（词还排着等键盘应用、云端有本机导入不了的词、被别的失败请求挡住，或者键盘还没发布过本机词库版本），或者本机这一侧读写词库失败（`dictionary_storage`、`dictionary_invalid` 这类）时的退避。退避期间回到前台的自动同步跳过词库，不用每次都重新下载整份快照；用户手动同步、打开开关或做选择时不等。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stall: Option<Stall>,
    /// 最近一轮这一类的结果。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_outcome: Option<SectionOutcome>,
}

impl SectionState {
    pub fn dirty(&self) -> bool {
        self.generation != self.clean
    }

    pub fn base_keys(&self) -> Option<&BTreeMap<String, String>> {
        match &self.base {
            Some(SectionBase::Keys(keys)) => Some(keys),
            _ => None,
        }
    }

    pub fn base_texts(&self) -> Option<&BTreeSet<String>> {
        match &self.base {
            Some(SectionBase::Texts(texts)) => Some(texts),
            _ => None,
        }
    }

    pub fn unheld_set(&self) -> BTreeSet<String> {
        self.unheld.iter().cloned().collect()
    }

    /// 还在合并的退避里。离现在超过 [`MAX_STALL_MS`] 的按已过期处理：那是时钟往回拨以前记下的。
    pub fn stalled(&self, now_ms: u64) -> bool {
        self.stall
            .is_some_and(|stall| now_ms < stall.until_ms && stall.until_ms - now_ms <= MAX_STALL_MS)
    }
}

/// 词库合并的退避：连着几轮没做完，退避到什么时候。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Stall {
    /// 连着没有进展的轮数；这一轮往导入队列里排了新的词、或者云端快照里已经在本机的词比上次记下的多（键盘应用了排着的词）时重新从 1 数起。重排键盘暂时没应用的请求不算进展。
    pub rounds: u32,
    pub until_ms: u64,
    /// 最近一次合并时云端快照里已经在本机的词数；下一轮比它多就是有进展。这一轮没有走到合并（本机读写失败、快照交不给激活队列）时沿用上次记下的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub present: Option<u64>,
}

/// 基线：设置是 键 → 值的摘要，常用语是正文摘要的集合。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SectionBase {
    Keys(BTreeMap<String, String>),
    Texts(BTreeSet<String>),
}

impl SectionBase {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Keys(keys) => keys.is_empty(),
            Self::Texts(texts) => texts.is_empty(),
        }
    }
}

/// 一类这一轮的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    /// 写了云端或本机。
    Synced,
    /// 两边都没变。
    Unchanged,
    /// 这一轮没做，原因见 `reason`。
    Skipped,
    Failed,
    NeedsChoice,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SectionOutcome {
    pub outcome: OutcomeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl SectionOutcome {
    pub fn new(outcome: OutcomeKind, reason: Option<&str>) -> Self {
        Self {
            outcome,
            reason: reason.map(str::to_owned),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sections {
    pub settings: SectionState,
    pub skins: SectionState,
    pub phrases: SectionState,
    pub dictionary: SectionState,
}

impl Sections {
    pub fn get(&self, section: Section) -> &SectionState {
        match section {
            Section::Settings => &self.settings,
            Section::Skins => &self.skins,
            Section::Phrases => &self.phrases,
            Section::Dictionary => &self.dictionary,
        }
    }

    pub fn get_mut(&mut self, section: Section) -> &mut SectionState {
        match section {
            Section::Settings => &mut self.settings,
            Section::Skins => &mut self.skins,
            Section::Phrases => &mut self.phrases,
            Section::Dictionary => &mut self.dictionary,
        }
    }
}

/// 一轮同步的租约。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunLease {
    pub id: String,
    pub user_id: String,
    /// 写下租约之后的 `state_version`；提交时还等于它，说明期间没有别的写入。
    pub state_version: u64,
    pub started_ms: u64,
    /// 这一轮用的选择；提交时据此清掉 `pending_choice`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<Choice>,
}

/// 最近一次失败：只记第一条。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyncFailure {
    pub section: Section,
    /// 稳定的原因码：服务端的错误码，或 `network`、`signed_out`、`http`、`phrases_over_limit` 这类本地码。
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// 给用户看的一句话。
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notices {
    /// 自定义皮肤库超出预算，只同步了最近的设计。
    pub skins_trimmed: bool,
}

/// 整份同步状态。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncState {
    pub version: u32,
    pub enabled: bool,
    /// 这份进度属于哪个账号；没有绑定时为空串。
    pub account_id: String,
    /// 每次换账号或退出都加一。
    pub binding_generation: u64,
    /// 除打标记和记下选择以外，每次写入都加一；提交时的比较并交换靠它。
    pub state_version: u64,
    pub sections: Sections,
    pub last_attempt_ms: Option<u64>,
    pub last_synced_ms: Option<u64>,
    /// 上次弹出「云端已有同步数据」的时间。
    pub prompted_at_ms: Option<u64>,
    /// 服务端给了 `Retry-After` 时，在这之前不再联网。
    pub not_before_ms: Option<u64>,
    /// 用户在另一轮还在跑时做出的选择，下一轮用它。记着它不让下一轮跳过节流，只有宿主这一次带来的选择才不等节流。
    pub pending_choice: Option<Choice>,
    pub run: Option<RunLease>,
    pub failure: Option<SyncFailure>,
    pub notices: Notices,
    /// 不再接受旧版本状态的导入：已经导入过一次，或者用户自己打开、关闭过同步。换账号和退出（[`SyncState::reset`]）都不清它，旧状态还留在宿主的存储里、每次启动都会再带来，导入一旦关上就不能再把用户关掉的同步改回打开。
    pub import_closed: bool,
    /// 开关是用户在「云端被清空」时选了「关闭同步，保留本机」关掉的（[`SyncState::disable_keeping_local`]），之后还没有再开关过。只给状态展示用；打开、关闭同步和换账号、退出都清掉它。
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disabled_on_cloud_cleared: bool,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            enabled: false,
            account_id: String::new(),
            binding_generation: 0,
            state_version: 0,
            sections: Sections::default(),
            last_attempt_ms: None,
            last_synced_ms: None,
            prompted_at_ms: None,
            not_before_ms: None,
            pending_choice: None,
            run: None,
            failure: None,
            notices: Notices::default(),
            import_closed: false,
            disabled_on_cloud_cleared: false,
        }
    }
}

/// 打开同步时没有绑定的账号。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotSignedIn;

impl SyncState {
    /// 每次推进一次版本。
    pub fn bump(&mut self) {
        self.state_version = self.state_version.wrapping_add(1);
    }

    /// 清空全部进度并关闭开关，`binding_generation` 加一，`state_version` 继续往上数：进行中的那一轮因此一定提交不了。`import_closed` 保留。
    pub fn reset(&mut self, account_id: &str) {
        let binding = self.binding_generation.wrapping_add(1);
        let version = self.state_version;
        let import_closed = self.import_closed;
        *self = Self {
            account_id: account_id.to_owned(),
            binding_generation: binding,
            state_version: version,
            import_closed,
            ..Self::default()
        };
        self.bump();
    }

    /// 与当前登录的账号对齐：账号不同（换账号、已经退出、原来没有）时清空，相同时不动。返回有没有清空。同一个账号重新登录保留进度。
    pub fn reconcile(&mut self, user_id: Option<&str>) -> bool {
        let user_id = user_id.unwrap_or("");
        if self.account_id == user_id {
            return false;
        }
        self.reset(user_id);
        true
    }

    /// 打开或关闭同步；关闭时保留游标，重新打开后增量继续。打开时把全部分类标成待上传。没有绑定账号时不能打开。开关是用户自己定的，此后不再导入旧状态。
    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), NotSignedIn> {
        if enabled && self.account_id.is_empty() {
            return Err(NotSignedIn);
        }
        self.enabled = enabled;
        self.import_closed = true;
        self.disabled_on_cloud_cleared = false;
        if enabled {
            for section in Section::ALL {
                let state = self.sections.get_mut(section);
                state.generation = state.generation.wrapping_add(1);
            }
        }
        self.bump();
        Ok(())
    }

    /// 有没有分类记着「云端被清空，等用户选择」。
    pub fn cloud_cleared(&self) -> bool {
        Section::ALL
            .iter()
            .any(|section| self.sections.get(*section).cleared)
    }

    /// 用户在「云端被清空」时选了「关闭同步，保留本机」：关掉开关，不上传、不下载，两边的数据都不动。调用方已经用 [`super::machine::disable_gate`] 确认过有绑定的账号、开关开着、确实在等这个选择。
    ///
    /// 清空标记、游标和改动代数都留着，所以用户以后重新打开同步时，第一轮读完云端仍然先问，不会悄悄把本机传回被清空的云端。记着的选择（另一轮在跑时记下的 `pending_choice`、词库合并没做完时留下的）丢掉：那是对这次清空以前的提问的回答，重新打开以后要按那时的云端重新问。`state_version` 加一，在跑的那一轮因此提交不了；它在这之前已经写到云端的不撤回。开关是用户自己定的，此后不再导入旧状态。
    pub fn disable_keeping_local(&mut self) {
        self.enabled = false;
        self.import_closed = true;
        self.disabled_on_cloud_cleared = true;
        self.pending_choice = None;
        self.sections.dictionary.choice = None;
        self.bump();
    }

    /// 记一次本机改动：开关打开时给这个分类的代数加一。关闭时不记，打开时本来就会整份比对。不推进 `state_version`。
    pub fn mark_dirty(&mut self, section: Section) -> bool {
        if !self.enabled {
            return false;
        }
        let state = self.sections.get_mut(section);
        state.generation = state.generation.wrapping_add(1);
        true
    }

    /// 本机清除了学习记录（`msime_client_dictionary` 的 `reset`）：忘掉等激活的云端快照（`activation`），再按 [`Self::mark_dirty`] 给词库记一次改动。等激活的那份是清除之前下载的，重置以后它因本机版本变了而冲突、不会激活；留着记录的话下一轮会把它当成没激活的快照、按合并把云端的旧词并回本机。不管开关开没开都忘掉：以后重新打开同步时也不该再去补它。不推进 `state_version`，与打标记一样不让进行中的那一轮作废。返回有没有记下改动。
    pub fn forget_dictionary_before_reset(&mut self) -> bool {
        self.sections.dictionary.activation = None;
        self.mark_dirty(Section::Dictionary)
    }

    pub fn any_dirty(&self) -> bool {
        Section::ALL
            .iter()
            .any(|section| self.sections.get(*section).dirty())
    }

    /// 还在服务端要求的退避里时返回退避到的时间。离现在超过 [`MAX_BACKOFF_MS`] 的按已过期处理：那是时钟往回拨以前记下的，不能一直挡住同步。
    pub fn backoff_until(&self, now_ms: u64) -> Option<u64> {
        self.not_before_ms
            .filter(|at| now_ms < *at && *at - now_ms <= MAX_BACKOFF_MS)
    }

    /// 有没有一个还没过期的租约。时钟往回拨时按已过期处理，免得一直被挡住。
    pub fn lease_active(&self, now_ms: u64) -> bool {
        self.run
            .as_ref()
            .is_some_and(|run| run.started_ms <= now_ms && now_ms - run.started_ms < LEASE_MS)
    }

    /// 开始一轮：记下尝试时间并写下租约，返回它。
    pub fn begin_run(&mut self, user_id: &str, now_ms: u64, choice: Option<Choice>) -> RunLease {
        self.last_attempt_ms = Some(now_ms);
        self.bump();
        let lease = RunLease {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_owned(),
            state_version: self.state_version,
            started_ms: now_ms,
            choice,
        };
        self.run = Some(lease.clone());
        lease
    }

    /// 这一轮还能不能提交：租约 id、账号和版本都对得上。
    pub fn lease_matches(&self, run_id: &str, user_id: &str) -> bool {
        self.account_id == user_id
            && self.run.as_ref().is_some_and(|run| {
                run.id == run_id
                    && run.user_id == user_id
                    && run.state_version == self.state_version
            })
    }

    /// 放弃这一轮，不推进任何进度。只在租约还对得上时才动。
    pub fn release(&mut self, run_id: &str) -> bool {
        if self.run.as_ref().is_some_and(|run| run.id == run_id) {
            self.run = None;
            self.bump();
            return true;
        }
        false
    }

    /// 提交一轮的进度。调用方已经确认过 [`Self::lease_matches`]。
    pub fn finish_round(&mut self, progress: &RoundProgress, now_ms: u64) {
        for (section, change) in &progress.sections {
            let state = self.sections.get_mut(*section);
            if let Some(cursor) = change.cursor {
                state.cursor = Some(cursor);
            }
            // 只清本轮开始时已经有的标记：之后用户又改了，代数更大，标记留着。
            if let Some(clean) = change.clean_to {
                state.clean = clean;
            }
            if change.mark_dirty {
                state.generation = state.generation.wrapping_add(1);
            }
            match &change.base {
                BaseChange::Keep => {}
                BaseChange::Clear => state.base = None,
                BaseChange::Set(base) => state.base = Some(base.clone()),
            }
            if let Some(unheld) = &change.unheld {
                state.unheld = unheld.clone();
            }
            if let Some(cleared) = change.cleared {
                state.cleared = cleared;
            }
            match &change.activation {
                ActivationChange::Keep => {}
                ActivationChange::Clear => state.activation = None,
                ActivationChange::Set(id) => state.activation = Some(id.clone()),
            }
            match change.stall {
                StallChange::Keep => {}
                StallChange::Clear => state.stall = None,
                StallChange::Set(stall) => state.stall = Some(stall),
            }
            // 游标推进了，首次同步（或清空以后）的那一次选择就用完了。
            if change.cursor.is_some() {
                state.choice = None;
            }
            state.last_outcome = Some(change.outcome.clone());
        }
        if progress.synced {
            self.last_synced_ms = Some(now_ms);
        }
        if progress.failure.is_some() || progress.synced {
            self.failure = progress.failure.clone();
        }
        if let Some(not_before) = progress.not_before_ms {
            self.not_before_ms = Some(not_before.min(now_ms.saturating_add(MAX_BACKOFF_MS)));
        } else if self.backoff_until(now_ms).is_none() {
            self.not_before_ms = None;
        }
        if let Some(trimmed) = progress.skins_trimmed {
            self.notices.skins_trimmed = trimmed;
        }
        if progress.prompted {
            self.prompted_at_ms = Some(now_ms);
        }
        if progress.consumed_choice {
            self.pending_choice = None;
        }
        if let Some(choice) = progress.kept_choice {
            self.sections.dictionary.choice = Some(choice);
        }
        self.run = None;
        self.bump();
    }

    /// 能不能用 `imported` 这份旧版本的状态替换现在的：没有导入过、用户也没有开关过同步（`import_closed`），没有打开、没有游标，而且没有绑定账号或绑定的就是旧状态里的账号。宿主在导入之前先调过 `status` 或 `run` 时，对账已经把登录的账号写进来了，这时仍然要能导入，否则迁移以后又会弹首次选择；用户没开关过同步就不会跑过同步，没有别的进度要保留。只看「现在关着」不够：用户打开过再关掉、或者导入以后又关掉时状态也是这样，再导入一次会把旧状态里的开关改回打开。
    pub fn accepts_import(&self, imported: &SyncState) -> bool {
        !self.import_closed
            && !self.enabled
            && (self.account_id.is_empty() || self.account_id == imported.account_id)
            && Section::ALL
                .iter()
                .all(|section| self.sections.get(*section).cursor.is_none())
    }
}

/// 一类要推进的进度。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SectionProgress {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<i64>,
    /// 清标记清到这个代数（本轮开始时读到的）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clean_to: Option<u64>,
    /// 提交后这一类要再标一次待上传（词库合并后要整份上传）。
    #[serde(default)]
    pub mark_dirty: bool,
    #[serde(default)]
    pub base: BaseChange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unheld: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleared: Option<bool>,
    /// 只对词库：等激活的快照要不要改记。
    #[serde(default)]
    pub activation: ActivationChange,
    /// 只对词库：合并的退避要不要改记。
    #[serde(default)]
    pub stall: StallChange,
    pub outcome: SectionOutcome,
}

impl SectionProgress {
    pub fn outcome(outcome: SectionOutcome) -> Self {
        Self {
            cursor: None,
            clean_to: None,
            mark_dirty: false,
            base: BaseChange::Keep,
            unheld: None,
            cleared: None,
            activation: ActivationChange::Keep,
            stall: StallChange::Keep,
            outcome,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseChange {
    #[default]
    Keep,
    Clear,
    Set(SectionBase),
}

/// 等激活的快照怎么改记。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationChange {
    #[default]
    Keep,
    Clear,
    /// 这一轮交给激活队列的请求 id。
    Set(String),
}

/// 词库合并的退避怎么改记。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StallChange {
    #[default]
    Keep,
    Clear,
    Set(Stall),
}

/// 一轮要提交的全部进度。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RoundProgress {
    pub sections: BTreeMap<Section, SectionProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<SyncFailure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_before_ms: Option<u64>,
    /// 参与的分类都成功了：记下同步时间、清掉上次的失败。
    #[serde(default)]
    pub synced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skins_trimmed: Option<bool>,
    /// 这一轮弹出了首次选择。
    #[serde(default)]
    pub prompted: bool,
    /// 这一轮用掉了记下的选择。
    #[serde(default)]
    pub consumed_choice: bool,
    /// 词库这一轮用了首次同步（或清空以后）的选择、但还没做完（合并没做完，或者键盘还没发布过本机词库版本）：记在词库的 `choice` 里，下一轮词库接着用，不再问用户。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept_choice: Option<Choice>,
}

/// 交给宿主写的值写完之前，要提交的进度。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingCommit {
    pub run_id: String,
    pub progress: RoundProgress,
    /// iOS：要在提交时按宿主实际保存下来的键盘设置应用到共享偏好的那份设置文档。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_document: Option<AccountPreferences>,
    /// iOS：这一轮读本机时共享偏好的修订号。提交时按它比较并交换，读本机以后另一个进程里用户改了设置时不拿这一轮的结果覆盖它；带着 `settings_document` 却没有它（更早的版本写下的）时这一轮按冲突作废。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_revision: Option<u64>,
    /// 这一轮各类用的模式，只给状态展示。
    #[serde(default)]
    pub modes: BTreeMap<Section, Mode>,
}

/// 状态文件读写失败。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StateError;

/// `<state_directory>/cloud-sync` 下的状态文件。
#[derive(Clone, Debug)]
pub struct SyncStateStore {
    directory: PathBuf,
}

impl SyncStateStore {
    pub fn new(state_directory: &Path) -> Self {
        Self {
            directory: state_directory.join(DIRECTORY_NAME),
        }
    }

    /// 状态目录，同步的临时文件（词库快照）也放在它下面。
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    fn lock(&self) -> Result<file_lock::ScopedFileLock, StateError> {
        if !crate::storage::create_directory_and_check(&self.directory).map_err(|_| StateError)? {
            return Err(StateError);
        }
        let file = file_lock::open_private_lock_file(self.directory.join(LOCK_FILE))
            .map_err(|_| StateError)?;
        // 阻塞等锁：每次持锁只读写一个小文件，而键盘打标记时不能因为应用恰好在写就丢掉这一次。
        file_lock::ScopedFileLock::exclusive(file).map_err(|_| StateError)
    }

    fn read_file(&self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>, StateError> {
        let path = self.directory.join(name);
        let file = match file_lock::open_private_file(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(StateError),
        };
        let mut bytes = Vec::new();
        file.take(maximum as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| StateError)?;
        // 超长的文件按不存在处理：读不出的状态当作全新，开关回到关闭，下一次打开时整份比对，不会丢数据。
        Ok((bytes.len() <= maximum).then_some(bytes))
    }

    fn load(&self) -> Result<SyncState, StateError> {
        let Some(bytes) = self.read_file(STATE_FILE, MAX_STATE_BYTES)? else {
            return Ok(SyncState::default());
        };
        Ok(serde_json::from_slice::<SyncState>(&bytes)
            .ok()
            .filter(|state| state.version == STATE_VERSION)
            .unwrap_or_default())
    }

    fn save(&self, state: &SyncState) -> Result<(), StateError> {
        let bytes = serde_json::to_vec(state).map_err(|_| StateError)?;
        if bytes.len() > MAX_STATE_BYTES {
            return Err(StateError);
        }
        file_lock::replace_private_file(self.directory.join(STATE_FILE), &bytes)
            .map_err(|_| StateError)
    }

    fn load_pending(&self) -> Result<Option<PendingCommit>, StateError> {
        Ok(self
            .read_file(PENDING_FILE, MAX_PENDING_BYTES)?
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()))
    }

    fn save_pending(&self, pending: Option<&PendingCommit>) -> Result<(), StateError> {
        let path = self.directory.join(PENDING_FILE);
        match pending {
            Some(pending) => {
                let bytes = serde_json::to_vec(pending).map_err(|_| StateError)?;
                if bytes.len() > MAX_PENDING_BYTES {
                    return Err(StateError);
                }
                file_lock::replace_private_file(&path, &bytes).map_err(|_| StateError)
            }
            None => match crate::storage::remove_private_file(&path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err(StateError),
            },
        }
    }

    /// 读当前状态；文件不存在时是全新的默认值。
    pub fn read(&self) -> Result<SyncState, StateError> {
        let _lock = self.lock()?;
        self.load()
    }

    /// 只读当前状态，不建状态目录、不写状态文件：目录还不存在时直接是全新的默认值。给不持有登录的进程看开关用。
    pub fn peek(&self) -> Result<SyncState, StateError> {
        match std::fs::symlink_metadata(&self.directory) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SyncState::default()),
            _ => self.read(),
        }
    }

    /// 在锁内读出、修改、写回；没有改动时不写。
    pub fn update<T>(&self, change: impl FnOnce(&mut SyncState) -> T) -> Result<T, StateError> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        let before = state.clone();
        let result = change(&mut state);
        if state != before {
            self.save(&state)?;
        }
        Ok(result)
    }

    /// 与 [`Self::update`] 相同，同时读写待提交的进度。闭包返回错误时什么都不写。
    pub fn update_with_pending<T, E>(
        &self,
        change: impl FnOnce(&mut SyncState, &mut Option<PendingCommit>) -> Result<T, E>,
    ) -> Result<Result<T, E>, StateError> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        let mut pending = self.load_pending()?;
        let before = (state.clone(), pending.clone());
        let result = change(&mut state, &mut pending);
        if result.is_ok() {
            if pending != before.1 {
                self.save_pending(pending.as_ref())?;
            }
            if state != before.0 {
                self.save(&state)?;
            }
        }
        Ok(result)
    }

    /// 与 [`Self::update_with_pending`] 相同，但闭包出错时状态的改动照样写回（例如提交失败时放弃租约），待提交的进度也照样写。
    pub fn update_always<T>(
        &self,
        change: impl FnOnce(&mut SyncState, &mut Option<PendingCommit>) -> T,
    ) -> Result<T, StateError> {
        let _lock = self.lock()?;
        let mut state = self.load()?;
        let mut pending = self.load_pending()?;
        let before = (state.clone(), pending.clone());
        let result = change(&mut state, &mut pending);
        if pending != before.1 {
            self.save_pending(pending.as_ref())?;
        }
        if state != before.0 {
            self.save(&state)?;
        }
        Ok(result)
    }
}

// ---- 旧版本状态的导入 ----

fn parse_cursor(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().filter(|revision| *revision >= 0)
}

fn iso_ms(value: Option<&str>) -> Option<u64> {
    let time = crate::account::parse_rfc3339(value?)?;
    u64::try_from(time.unix_timestamp_nanos() / 1_000_000).ok()
}

/// iOS 的 `CloudSyncRecord`（`CloudSync/state.json` 第 1 版）。
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct IosRecord {
    version: u32,
    enabled: bool,
    #[serde(rename = "accountID")]
    account_id: String,
    cursors: BTreeMap<String, String>,
    generations: BTreeMap<String, u64>,
    cleans: BTreeMap<String, u64>,
    unheld_phrases: Vec<String>,
    last_attempt: Option<String>,
    last_synced: Option<String>,
    prompted_at: Option<String>,
}

/// 读入 iOS 第 1 版的状态：开关、账号、游标、改动代数、本机收不下的常用语（换成摘要）和三个时间。读不懂或版本不是 1 时为 `None`。基线没有，第一次三方合并按首次处理（本机优先、并集），与旧版本的行为相同。
pub fn import_ios_v1(record: &serde_json::Value) -> Option<SyncState> {
    let record: IosRecord = serde_json::from_value(record.clone()).ok()?;
    if record.version != 1 {
        return None;
    }
    let mut state = SyncState {
        enabled: record.enabled && !record.account_id.is_empty(),
        account_id: record.account_id,
        last_attempt_ms: iso_ms(record.last_attempt.as_deref()),
        last_synced_ms: iso_ms(record.last_synced.as_deref()),
        prompted_at_ms: iso_ms(record.prompted_at.as_deref()),
        ..SyncState::default()
    };
    for section in [Section::Settings, Section::Phrases] {
        let key = section.as_str();
        let target = state.sections.get_mut(section);
        target.cursor = record
            .cursors
            .get(key)
            .and_then(|cursor| parse_cursor(cursor));
        target.generation = record.generations.get(key).copied().unwrap_or(0);
        target.clean = record.cleans.get(key).copied().unwrap_or(0);
    }
    state.sections.phrases.unheld =
        super::policy::text_digests(record.unheld_phrases.iter().map(String::as_str))
            .into_iter()
            .collect();
    Some(state)
}

/// 读入 Android 的两份 `SharedPreferences`：`msime_sync_v1`（开关、账号、各分类的 `cursor_`/`gen_`/`clean_`、`phrases_unheld`、`last_synced_at`）和 `msime_cloud_sync_status`（`last_attempt`、`prompted_at`、`skins_trimmed`，只在它的 `binding_generation` 与前者相同时才算数，与 `CloudSync.statusLine` 的判断相同）。值的形状不对的键跳过。
pub fn import_android(sync: &serde_json::Value, status: &serde_json::Value) -> Option<SyncState> {
    let sync = sync.as_object()?;
    let empty = serde_json::Map::new();
    let status = status.as_object().unwrap_or(&empty);
    let text = |key: &str| sync.get(key).and_then(serde_json::Value::as_str);
    let number = |map: &serde_json::Map<String, serde_json::Value>, key: &str| {
        map.get(key).and_then(serde_json::Value::as_u64)
    };
    let account_id = text("account_id").unwrap_or("").to_owned();
    let login_kind_valid = matches!(text("login_kind"), Some("google" | "apple" | "email"));
    let mut state = SyncState {
        enabled: sync
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            && login_kind_valid
            && !account_id.is_empty(),
        account_id,
        last_synced_ms: number(sync, "last_synced_at").filter(|at| *at > 0),
        ..SyncState::default()
    };
    for section in Section::ALL {
        let key = section.as_str();
        let target = state.sections.get_mut(section);
        target.cursor = text(&format!("cursor_{key}")).and_then(parse_cursor);
        target.generation = number(sync, &format!("gen_{key}")).unwrap_or(0);
        target.clean = number(sync, &format!("clean_{key}")).unwrap_or(0);
    }
    if let Some(unheld) = sync
        .get("phrases_unheld")
        .and_then(serde_json::Value::as_array)
    {
        state.sections.phrases.unheld =
            super::policy::text_digests(unheld.iter().filter_map(serde_json::Value::as_str))
                .into_iter()
                .collect();
    }
    if number(status, "binding_generation") == number(sync, "binding_generation") {
        state.last_attempt_ms = number(status, "last_attempt").filter(|at| *at > 0);
        state.prompted_at_ms = number(status, "prompted_at").filter(|at| *at > 0);
        state.notices.skins_trimmed = status
            .get("skins_trimmed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
    }
    Some(state)
}

impl SyncState {
    /// 用导入的旧状态替换这一份全新的状态：两个代次继续往上数，此后不再导入。
    pub fn adopt(&mut self, imported: SyncState) {
        let binding = self.binding_generation.wrapping_add(1);
        let version = self.state_version;
        *self = SyncState {
            binding_generation: binding,
            state_version: version,
            import_closed: true,
            ..imported
        };
        self.bump();
    }
}

#[cfg(test)]
mod tests;
