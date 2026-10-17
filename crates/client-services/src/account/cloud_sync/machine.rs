//! sans-IO 的单轮状态机。[`gate`] 决定这一轮要不要跑；[`SectionMachine`] 管一个分类从读云端到提交的每一步：驱动执行它给出的 [`Step`]，把结果作为 [`Event`] 喂回去，直到 [`Step::Done`]。合并的内容由驱动按 [`super::policy`] 算，这里只决定模式、冲突重试和提交什么。
//!
//! 设置（连同皮肤库）、常用语和词库共用一套步骤，区别只在合并怎么落地：设置和常用语的合并是算出一份新文档 `Put` 上去，词库的合并是把云端快照的词排进本机导入队列（`WriteLocal{MergeIntoLocal}`），等键盘应用后下一轮再整份上传。

use super::policy::{
    cloud_changed, cloud_cleared, dictionary_first_run, due, first_run, incremental, is_conflict,
    use_cloud_protected, Choice, Mode, Section, MAX_CONFLICT_RETRIES,
};
use super::state::{OutcomeKind, SectionOutcome, SyncState};
use serde::{Deserialize, Serialize};

/// 宿主为什么叫这一轮。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// 回到前台，按节流决定要不要跑。
    Active,
    /// 用户刚打开同步开关。
    Enable,
    /// 用户在「云端已有同步数据」或「云端被清空」的提示里做了选择。
    Choice,
    /// 用户手动点了同步。
    Manual,
}

impl Trigger {
    /// 不等节流（仍然尊重 `Retry-After`），首次选择的提示也马上弹。
    pub fn forced(self) -> bool {
        self != Self::Active
    }
}

/// 这一轮为什么不跑。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkipReason {
    SignedOut,
    Disabled,
    Backoff,
    Throttled,
    Busy,
    /// 只对「关闭同步，保留本机」：没有分类在等「云端被清空」的选择。
    NotCloudCleared,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignedOut => "signed_out",
            Self::Disabled => "disabled",
            Self::Backoff => "backoff",
            Self::Throttled => "throttled",
            Self::Busy => "busy",
            Self::NotCloudCleared => "not_cloud_cleared",
        }
    }
}

/// 这一轮要不要跑：要有绑定的账号、开关开着、不在服务端要求的退避里；回到前台时还要满足节流（只看 `sections` 这几类的待上传标记，宿主不同步的分类打开开关时也被标过，不能让它们一直把节流缩到 30 秒；词库在合并的退避里时调用方也不把它算进来）；已经有一轮在跑（租约没过期）时不跑。`chosen` 是宿主这一次请求带没带用户的选择：带了就不等节流。状态里记着的选择（另一轮在跑时做的 `pending_choice`、词库合并没做完时留下的）不算，否则合并一直做不完时每次回到前台都会跳过节流、重新下载整份快照。
pub fn gate(
    state: &SyncState,
    now_ms: u64,
    trigger: Trigger,
    chosen: bool,
    sections: &[Section],
) -> Result<(), SkipReason> {
    if state.account_id.is_empty() {
        return Err(SkipReason::SignedOut);
    }
    if !state.enabled {
        return Err(SkipReason::Disabled);
    }
    if state.backoff_until(now_ms).is_some() {
        return Err(SkipReason::Backoff);
    }
    let forced = trigger.forced() || chosen;
    let any_dirty = sections
        .iter()
        .any(|section| state.sections.get(*section).dirty());
    if !forced && !due(now_ms, state.last_attempt_ms, any_dirty) {
        return Err(SkipReason::Throttled);
    }
    if state.lease_active(now_ms) {
        return Err(SkipReason::Busy);
    }
    Ok(())
}

/// 「关闭同步，保留本机」（[`super::policy::Answer::Disable`]）能不能做：要有绑定的账号、开关开着，而且有分类记着「云端被清空，等用户选择」；它是那个提示的第三个回答，首次同步的提示没有它。它不联网也不跑一轮，所以不看节流、`Retry-After` 的退避和租约：另一轮还在跑时也马上关掉（那一轮因此提交不了），不像同步选择那样记成 `pending_choice` 留给下一轮。
pub fn disable_gate(state: &SyncState) -> Result<(), SkipReason> {
    if state.account_id.is_empty() {
        return Err(SkipReason::SignedOut);
    }
    if !state.enabled {
        return Err(SkipReason::Disabled);
    }
    if !state.cloud_cleared() {
        return Err(SkipReason::NotCloudCleared);
    }
    Ok(())
}

/// 一个分类开始这一轮时的已知情况，取自状态文件。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionInput {
    /// 设置、常用语或词库；皮肤库跟着设置走。
    pub section: Section,
    pub cursor: Option<i64>,
    /// 显式的待上传标记（设置时包括皮肤库的标记）。
    pub dirty: bool,
    /// 有没有上次同步的基线。
    pub base_present: bool,
    /// 状态里记着「云端被清空，等用户选择」。
    pub cleared: bool,
    pub choice: Option<Choice>,
}

/// 读到的云端情况。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloudProbe {
    pub revision: i64,
    /// 云端有内容：设置文档非空、常用语列表非空、词库自 0 以来有改动而且不是被清空成空的（`reset_and_empty` 为真时它为假）。
    pub has_data: bool,
    /// 只对词库：云端词库现在是空的。有游标时还要第一条新改动是 `reset`（「删除云端数据」，据此判断云端被清空）；没有游标（首次同步）时只看云端词库的条数，历史里有没有 `reset` 都一样。
    pub reset_and_empty: bool,
}

/// 只对词库：下载下来交给激活队列的云端快照现在怎样了。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotActivation {
    /// 没有在等的快照，或者等的那份已经激活。
    Idle,
    /// 激活队列里有快照还在等键盘激活：这一类这一轮不写任何一边。
    Waiting,
    /// 等的那份没有激活就结束了（本机版本变了而冲突、快照不合规、准备失败或被取消）：游标已经是快照的修订号，本机却没有那些词。这一轮按合并处理，把云端的词并进本机，不能上传，否则本机这份没激活过的词库会整份覆盖云端。
    Lost,
}

/// 读到的本机情况。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalProbe {
    pub has_data: bool,
    /// 按内容判断本机自基线以来改过（只有常用语这样判断）。
    pub content_dirty: bool,
    /// 只对词库：导入队列里还有键盘没应用的词，或者下载的快照还在等激活，现在不能上传。
    pub upload_blocked: bool,
    /// 只对词库：下载的云端快照激活了没有；其他分类总是 `Idle`。
    pub activation: SnapshotActivation,
}

/// 一次联网失败。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpFailure {
    pub status: Option<u16>,
    pub code: Option<String>,
    pub retry_after_seconds: Option<u64>,
}

/// 驱动执行完一步之后喂回来的结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    Fetched(CloudProbe),
    LocalRead(LocalProbe),
    /// 写入成功；`revision` 是写入后的云端修订号（下载时是下载到的修订号）。
    Written {
        revision: i64,
    },
    HttpFailed(HttpFailure),
    /// 本机读写失败，`reason` 是稳定的原因码。
    LocalFailed {
        reason: String,
    },
    /// 只对词库：下载下来要整份替换本机（`WriteLocal{Cloud}`）的云端快照一个词都没有，而本机这一轮读到自己的词。驱动没有替换；这一类改按合并处理（`WriteLocal{MergeIntoLocal}`），两边的词都留着，提交后再标一次待上传，把本机的词传回云端。
    EmptySnapshot,
}

/// 本机写什么。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteTarget {
    /// 用云端的内容替换本机（下载、使用云端）。
    Cloud,
    /// 把刚 `Put` 上去的结果应用到本机。
    Saved,
    /// 只对词库：把云端快照的词并进本机。
    MergeIntoLocal,
}

/// 为什么要问用户。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChoiceReason {
    /// 首次同步，云端已经有数据。
    FirstRun,
    /// 云端被清空了（多半是「删除云端数据」）。除了合并和使用云端，还可以「关闭同步，保留本机」（见 [`disable_gate`]）。
    CloudCleared,
}

impl ChoiceReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FirstRun => "first_run",
            Self::CloudCleared => "cloud_cleared",
        }
    }
}

/// 一类要提交的东西。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SectionCommit {
    pub cursor: i64,
    /// 清掉本轮开始时已有的待上传标记。
    pub clear_dirty: bool,
    /// 提交后再标一次待上传：词库合并以后要等键盘应用，再整份上传。
    pub mark_dirty: bool,
    /// 清掉「云端被清空，等用户选择」。
    pub reset_cleared: bool,
    /// 只对词库：清掉记着的等激活的快照（它已经激活，或者这一轮已经按合并补上了云端的词）。下载时驱动改记这一轮交给激活队列的请求。
    pub clear_activation: bool,
    pub mode: Mode,
}

/// 驱动要执行的下一步。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
    /// 读云端（重试冲突时也是它）。
    Fetch,
    /// 读本机。
    ReadLocal,
    /// 首次同步或云端被清空，没有选择时要问用户。整轮停下，不做任何写入。
    NeedsChoice(ChoiceReason),
    /// 把按 `mode` 算出的结果写到云端。
    Put(Mode),
    WriteLocal(WriteTarget),
    /// 记下进度；驱动记完再调一次 `next(None)`。
    Commit(SectionCommit),
    Done(SectionOutcome),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Start,
    Fetching,
    Reading,
    Putting,
    Refetching,
    Writing,
    Committing,
    Finished,
}

/// 一个分类的一轮。
#[derive(Clone, Debug)]
pub struct SectionMachine {
    input: SectionInput,
    phase: Phase,
    probe: Option<CloudProbe>,
    local: Option<LocalProbe>,
    mode: Mode,
    awaiting_choice: bool,
    retries: u32,
    failure: Option<HttpFailure>,
    outcome: Option<SectionOutcome>,
}

impl SectionMachine {
    pub fn new(input: SectionInput) -> Self {
        Self {
            input,
            phase: Phase::Start,
            probe: None,
            local: None,
            mode: Mode::None,
            awaiting_choice: false,
            retries: 0,
            failure: None,
            outcome: None,
        }
    }

    pub fn section(&self) -> Section {
        self.input.section
    }

    /// 这一类这一轮用的模式（读完本机之后才定下）。
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// 读到的云端情况。
    pub fn probe(&self) -> Option<CloudProbe> {
        self.probe
    }

    /// 这一轮发现了「云端被清空」（或状态里本来就记着）。
    pub fn awaiting_choice(&self) -> bool {
        self.awaiting_choice
    }

    /// 最后一次联网失败，给驱动记失败原因和退避用。
    pub fn failure(&self) -> Option<&HttpFailure> {
        self.failure.as_ref()
    }

    /// 结束时的结果。
    pub fn outcome(&self) -> Option<&SectionOutcome> {
        self.outcome.as_ref()
    }

    fn done(&mut self, outcome: OutcomeKind, reason: Option<&str>) -> Step {
        let outcome = SectionOutcome::new(outcome, reason);
        self.phase = Phase::Finished;
        self.outcome = Some(outcome.clone());
        Step::Done(outcome)
    }

    fn failed(&mut self, failure: HttpFailure) -> Step {
        let reason = failure
            .code
            .clone()
            .unwrap_or_else(|| match failure.status {
                Some(_) => "http".to_owned(),
                None => "network".to_owned(),
            });
        self.failure = Some(failure);
        self.done(OutcomeKind::Failed, Some(&reason))
    }

    fn dictionary(&self) -> bool {
        self.input.section == Section::Dictionary
    }

    /// 读到的云端相对上次同步被清空了（不算状态里已经记着的清空）。
    fn cleared_now(&self, cloud: CloudProbe) -> bool {
        let input = self.input;
        if self.dictionary() {
            input.cursor.is_some()
                && cloud_changed(input.cursor, cloud.revision)
                && cloud.reset_and_empty
        } else {
            cloud_cleared(
                input.base_present,
                input.cursor,
                cloud.revision,
                !cloud.has_data,
            )
        }
    }

    fn ask(&mut self, reason: ChoiceReason) -> Step {
        self.phase = Phase::Finished;
        self.outcome = Some(SectionOutcome::new(
            OutcomeKind::NeedsChoice,
            Some(reason.as_str()),
        ));
        Step::NeedsChoice(reason)
    }

    /// 读完两边之后定模式。
    fn decide(&mut self, cloud: CloudProbe, local: LocalProbe) -> Result<Mode, ChoiceReason> {
        let input = self.input;
        let changed = cloud_changed(input.cursor, cloud.revision);
        self.awaiting_choice = input.cleared || self.cleared_now(cloud);
        if input.cursor.is_some() && !self.awaiting_choice {
            // 下载的快照没有激活：本机缺着游标以前的云端改动，不管两边有没有新改动都要把云端的词补进本机；本机没有词时再下载一次。
            if self.dictionary() && local.activation == SnapshotActivation::Lost {
                return Ok(if local.has_data {
                    Mode::Merge
                } else {
                    Mode::Download
                });
            }
            return Ok(incremental(changed, input.dirty || local.content_dirty));
        }
        let ask = if self.awaiting_choice {
            ChoiceReason::CloudCleared
        } else {
            ChoiceReason::FirstRun
        };
        let needs_choice = cloud.has_data || self.awaiting_choice;
        if !needs_choice {
            // 云端是空的：本机有数据就上传，都空就只记下游标。
            return Ok(if local.has_data {
                Mode::Upload
            } else {
                Mode::None
            });
        }
        let choice = input.choice.ok_or(ask)?;
        let protected = choice == Choice::UseCloud
            && use_cloud_protected(cloud.revision, cloud.has_data, local.has_data);
        let choice = if protected { Choice::Merge } else { choice };
        let mode = if self.dictionary() {
            dictionary_first_run(true, local.has_data, Some(choice))
        } else {
            first_run(true, Some(choice))
        };
        Ok(mode.unwrap_or(Mode::None))
    }

    fn commit(&mut self, cursor: i64) -> Step {
        self.phase = Phase::Committing;
        let dictionary = self.dictionary();
        let waiting = self
            .local
            .is_some_and(|local| local.activation == SnapshotActivation::Waiting);
        Step::Commit(SectionCommit {
            cursor,
            // 词库下载后保留标记：激活时本机版本若已变，请求会作废，标记是本机改动还要上传的唯一信号（与 Android 相同）。
            clear_dirty: !(dictionary && matches!(self.mode, Mode::Download | Mode::Merge)),
            mark_dirty: dictionary && self.mode == Mode::Merge,
            reset_cleared: self.awaiting_choice,
            clear_activation: dictionary && self.mode != Mode::Download && !waiting,
            mode: self.mode,
        })
    }

    fn act(&mut self, local: LocalProbe) -> Step {
        let revision = self.probe.map_or(0, |probe| probe.revision);
        match self.mode {
            Mode::None => self.commit(revision),
            // 快照还在等激活：下载会被激活队列拒绝，合并排进去的词和激活谁先谁后也说不准，上传更不行，这一轮什么都不写。
            _ if self.dictionary() && local.activation == SnapshotActivation::Waiting => {
                self.done(OutcomeKind::Skipped, Some("dictionary_pending"))
            }
            Mode::Download => {
                self.phase = Phase::Writing;
                Step::WriteLocal(WriteTarget::Cloud)
            }
            Mode::Merge if self.dictionary() => {
                self.phase = Phase::Writing;
                Step::WriteLocal(WriteTarget::MergeIntoLocal)
            }
            Mode::Upload if self.dictionary() && local.upload_blocked => {
                self.done(OutcomeKind::Skipped, Some("dictionary_pending"))
            }
            mode => {
                self.phase = Phase::Putting;
                Step::Put(mode)
            }
        }
    }

    /// 下一步。第一次和每次 `Commit` 之后传 `None`，其余时候传上一步的结果；对不上当前阶段的事件按本机失败结束。
    pub fn next(&mut self, event: Option<Event>) -> Step {
        match (self.phase, event) {
            (Phase::Start, None) => {
                self.phase = Phase::Fetching;
                Step::Fetch
            }
            (Phase::Fetching, Some(Event::Fetched(probe))) => {
                self.probe = Some(probe);
                self.phase = Phase::Reading;
                Step::ReadLocal
            }
            (
                Phase::Fetching | Phase::Refetching | Phase::Putting,
                Some(Event::HttpFailed(failure)),
            ) if !(self.phase == Phase::Putting && self.conflict_retry(&failure)) => {
                self.failed(failure)
            }
            (Phase::Putting, Some(Event::HttpFailed(_))) => {
                // 冲突且还能重试：重新读云端，改按合并再写。
                self.retries += 1;
                self.mode = Mode::Merge;
                self.phase = Phase::Refetching;
                Step::Fetch
            }
            (Phase::Reading, Some(Event::LocalRead(local))) => {
                self.local = Some(local);
                let cloud = self.probe.expect("probe is set before reading local");
                match self.decide(cloud, local) {
                    Ok(mode) => {
                        self.mode = mode;
                        self.act(local)
                    }
                    Err(reason) => self.ask(reason),
                }
            }
            (Phase::Refetching, Some(Event::Fetched(probe))) => {
                self.probe = Some(probe);
                // 别的设备在这一轮读云端和写云端之间删除了云端数据：和第一次读到时一样停下来问，不拿旧基线三方合并（否则基线里有、云端没有的本机数据会被当成在云端删掉），也不把本机的词并进去以后再整份传回被清空的云端。这一轮本来就是因为清空才问过、带着选择来的，不再问。
                if !self.awaiting_choice && self.cleared_now(probe) {
                    self.awaiting_choice = true;
                    return self.ask(ChoiceReason::CloudCleared);
                }
                if self.dictionary() {
                    // 词库上传冲突：云端在探测之后变了，把云端的词并进本机，下一轮再整份上传。
                    self.phase = Phase::Writing;
                    Step::WriteLocal(WriteTarget::MergeIntoLocal)
                } else {
                    self.phase = Phase::Putting;
                    Step::Put(Mode::Merge)
                }
            }
            (Phase::Putting, Some(Event::Written { revision })) => {
                if self.dictionary() {
                    self.commit(revision)
                } else {
                    self.phase = Phase::Writing;
                    self.probe = self.probe.map(|probe| CloudProbe { revision, ..probe });
                    Step::WriteLocal(WriteTarget::Saved)
                }
            }
            (Phase::Writing, Some(Event::Written { revision })) => self.commit(revision),
            // 硬性约束：有词的本机词库不拿一个词都没有的快照整份替换。不管这次下载是「使用云端」、首次同步还是之后云端有改动，也不管云端汇总读没读出来，都改按合并处理。
            (Phase::Writing, Some(Event::EmptySnapshot))
                if self.dictionary()
                    && self.mode == Mode::Download
                    && self.local.is_some_and(|local| local.has_data) =>
            {
                self.mode = Mode::Merge;
                Step::WriteLocal(WriteTarget::MergeIntoLocal)
            }
            (_, Some(Event::LocalFailed { reason })) if self.phase != Phase::Finished => {
                self.done(OutcomeKind::Failed, Some(&reason))
            }
            (Phase::Committing, None) => {
                let outcome = if self.mode == Mode::None {
                    OutcomeKind::Unchanged
                } else {
                    OutcomeKind::Synced
                };
                self.done(outcome, None)
            }
            (Phase::Finished, _) => Step::Done(
                self.outcome
                    .clone()
                    .unwrap_or_else(|| SectionOutcome::new(OutcomeKind::Failed, None)),
            ),
            _ => self.done(OutcomeKind::Failed, Some("unexpected_event")),
        }
    }

    fn conflict_retry(&self, failure: &HttpFailure) -> bool {
        is_conflict(failure.status, failure.code.as_deref()) && self.retries < MAX_CONFLICT_RETRIES
    }
}

#[cfg(test)]
mod tests;
