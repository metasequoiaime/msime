//! 多分类自动云同步的驱动：按 client-services 的状态机（`account::cloud_sync::machine`）一步一步执行，联网用宿主那一份会话（令牌刷新、代次防护都是会话自己的），本机写入用 `PreferencesStore`、`CommonPhrasesStore`、词库导出和快照队列。词库导出和激活要用 Engine，所以驱动放在 host-api。C 入口是 `ffi::cloud_sync` 的 `msime_client_cloud_sync`。
//!
//! 一轮的顺序：在状态锁里对齐账号、判断要不要跑并写下租约；先把参与的分类都读一遍（云端和本机），有分类首次同步而云端有数据（或云端被清空）又没有选择时整轮停下问用户；否则逐类执行，一类失败不挡其他类。联网的每一步都落在开始那一轮的登录上（`authenticated_as`），本机写入都在 `with_generation` 的闭包里做，闭包里只写本地文件，不联网、不取别的会话的令牌。游标、清标记和基线只在提交时推进；交给宿主写的值（iOS 的键盘设置、Android 和鸿蒙的按键反馈、Android 的本地设置、皮肤库、要认领的示例常用语）写完之前，进度放在 `pending.json`，宿主写完调 `commit`。
//!
//! 写成对 [`AccountApi`]、[`AccountSessionStorage`] 和 [`SyncLocal`] 泛型的函数，测试用假的后端和内存里的本机驱动。

use crate::dictionary_request_json;
use crate::dictionary_snapshot::inspect_snapshot_json;
use msime_client_core::cloud::snapshot_queue::{DictionarySnapshotQueue, SnapshotRequestStatus};
use msime_client_core::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings,
};
use msime_client_core::ios_keyboard::IosKeyboardPreferences;
use msime_client_core::preferences::{PreferencesError, PreferencesStore};
use msime_client_services::account::cloud_sync::machine::{
    disable_gate, gate, ChoiceReason, CloudProbe, Event, HttpFailure, LocalProbe, SectionCommit,
    SectionInput, SectionMachine, SnapshotActivation, Step, Trigger, WriteTarget,
};
use msime_client_services::account::cloud_sync::policy::{
    self, Choice, LocalPhrase, Mode, Section, StatusLine,
};
use msime_client_services::account::cloud_sync::state::{
    import_android, import_ios_v1, ActivationChange, BaseChange, OutcomeKind, PendingCommit,
    RoundProgress, RunLease, SectionBase, SectionOutcome, SectionProgress, Stall, StallChange,
    SyncFailure, SyncState, SyncStateStore,
};
use msime_client_services::account::harmony_settings::{
    apply_harmony_settings, export_harmony_settings,
};
use msime_client_services::account::ios_settings::{local_account_preferences, IosPreferencePlan};
use msime_client_services::account::settings_sync::{
    android_local_settings, apply_android_settings, custom_keyboard_skins, export_android_settings,
    insert_android_local_settings, HostKeyboardFeedback,
};
use msime_client_services::account::{
    AccountApi, AccountDictionaryProbe, AccountError, AccountFailure, AccountPhrase,
    AccountPhrases, AccountPreferenceSchema, AccountPreferenceValue, AccountPreferences,
    AccountSessionStorage, BackendAccountSession,
};
use msime_client_services::common_phrases::{
    valid_phrase_text, CommonPhrasesError, CommonPhrasesStore,
};
use msime_client_services::dictionary::collections::DictionaryCollectionsStore;
use msime_client_services::dictionary::personal::{
    PersonalDictionaryError, PersonalDictionaryStore, PersonalWord, PersonalWordRequest,
    PersonalWordRequestStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(crate) const INVALID: &str = "cloud_sync_invalid";
pub(crate) const STORAGE: &str = "cloud_sync_storage";
pub(crate) const SIGNED_OUT: &str = "cloud_sync_signed_out";
pub(crate) const STALE_RUN: &str = "stale_run";

/// 宿主这一端能同步哪几类。iOS 是 `{settings, phrases}`：个人词库由键盘扩展里的 Engine 持有，皮肤库的字段只有 Android 有。
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Capabilities {
    pub(crate) settings: bool,
    pub(crate) skins_library: bool,
    pub(crate) phrases: bool,
    pub(crate) dictionary: bool,
}

/// 宿主保管、Rust 读不到的本机值。带 `ios_native` 时设置按 iOS 映射，`harmony` 为真时按鸿蒙映射，否则按 Android 映射。
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct HostInputs {
    /// iOS：App Group 里当前的键盘设置。
    pub(crate) ios_native: Option<IosKeyboardPreferences>,
    /// 鸿蒙：设置按 `platform.harmony.*` 映射，按键反馈仍放在 `feedback`；不能与 `ios_native`、`android_local`、`custom_keyboard_skins` 同时给。
    pub(crate) harmony: bool,
    /// Android 和鸿蒙：当前的按键反馈。
    pub(crate) feedback: Option<HostKeyboardFeedback>,
    /// Android：本地设置文件里参与同步的值，按同步键给出。
    pub(crate) android_local: Option<BTreeMap<String, AccountPreferenceValue>>,
    /// Android：自定义键盘皮肤库的 JSON 数组。
    pub(crate) custom_keyboard_skins: Option<String>,
    /// iOS：社区皮肤试用中，共享文档里是用户还没决定保留的皮肤，这一轮不同步设置。
    pub(crate) settings_locked: bool,
    /// Android：本机预置、还没被认领的示例常用语；不上传，也不会被当成本机多出来的删掉。
    pub(crate) excluded_phrase_texts: Vec<String>,
}

/// 交给宿主写的值。只列出与宿主传进来的不同的那些。
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(crate) struct HostWrites {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ios_native: Option<IosKeyboardPreferences>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) feedback: Option<HostKeyboardFeedback>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) android_local: Option<BTreeMap<String, AccountPreferenceValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) custom_keyboard_skins: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) adopt_texts: Option<Vec<String>>,
}

impl HostWrites {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn absorb(&mut self, other: HostWrites) {
        self.ios_native = other.ios_native.or(self.ios_native.take());
        self.feedback = other.feedback.or(self.feedback.take());
        self.android_local = other.android_local.or(self.android_local.take());
        self.custom_keyboard_skins = other
            .custom_keyboard_skins
            .or(self.custom_keyboard_skins.take());
        self.adopt_texts = other.adopt_texts.or(self.adopt_texts.take());
    }
}

/// 本机导出的设置：已经按版本过滤，还没去掉隐私键、没按字段表过滤；皮肤库单独给，驱动按预算裁剪后再放进文档。
pub(crate) struct LocalSettings {
    pub(crate) settings: BTreeMap<String, AccountPreferenceValue>,
    pub(crate) skins: Option<String>,
    /// 导出时共享偏好的修订号：写回时按它比较并交换，期间被别处改过就不覆盖。
    pub(crate) revision: u64,
}

/// 应用一份设置文档的结果。
pub(crate) struct SettingsApplied {
    pub(crate) writes: HostWrites,
    /// iOS：键盘设置要变，共享偏好要等宿主写完、按实际保存下来的值在提交时再应用。
    pub(crate) deferred: bool,
}

/// 本机词库的情况。
pub(crate) struct DictionaryLocal {
    pub(crate) has_words: bool,
    /// 个人词库导入队列里还没被键盘应用的请求数，加上命名词库待发送队列里还没送进去的增删条数。
    pub(crate) pending: usize,
    /// 下载下来的云端快照：激活队列里还有在等激活的（`Waiting`），或者状态里记着的那份没激活就结束了（`Lost`）。
    pub(crate) activation: SnapshotActivation,
}

/// 词库合并没做完或者失败：`reason` 是原因码（[`MERGE_PENDING`]、[`MERGE_REJECTED`]、[`MERGE_BLOCKED`] 或 `dictionary_storage` 这类）。
#[derive(Debug, PartialEq)]
pub(crate) struct MergeStop {
    pub(crate) reason: String,
    /// 这一轮往导入队列里排了新的词：合并在往前走，退避不加长。重排键盘因为暂时拿不到词库而没应用的请求不算，那些词上一轮已经排过，键盘一直拿不到词库时每一轮都会重排。
    pub(crate) queued: bool,
    /// 云端快照里已经在本机的词数；比上一轮多说明键盘应用了排着的词，合并也在往前走。没走到比对这一步（读写失败）时为 `None`。
    pub(crate) present: Option<usize>,
}

impl From<String> for MergeStop {
    fn from(reason: String) -> Self {
        Self {
            reason,
            queued: false,
            present: None,
        }
    }
}

/// 驱动用到的本机读写。错误是稳定的原因码（`settings_storage`、`phrases_storage`、`dictionary_storage` 等）。
pub(crate) trait SyncLocal {
    fn export_settings(&self) -> Result<LocalSettings, String>;
    /// 把云端文档应用到本机。`read_revision` 是这一轮读本机时（[`Self::export_settings`]）共享偏好的修订号：共享偏好在这之后被别处改过时什么都不写，返回 [`SETTINGS_CHANGED`]。
    fn apply_settings(
        &self,
        document: &AccountPreferences,
        schema: &AccountPreferenceSchema,
        read_revision: u64,
    ) -> Result<SettingsApplied, String>;
    /// iOS 的第二步：按宿主实际保存下来的键盘设置把文档应用到共享偏好。`read_revision` 与 [`Self::apply_settings`] 的相同，是这一轮读本机时共享偏好的修订号：读本机以后（中间隔着联网和宿主写键盘设置）共享偏好被别处改过时什么都不写，返回 `conflict`。失败是 `conflict` 或 `storage`。
    fn commit_settings(
        &self,
        document: &AccountPreferences,
        saved_native: &IosKeyboardPreferences,
        read_revision: u64,
    ) -> Result<(), String>;
    fn phrases(&self) -> Result<Vec<LocalPhrase>, String>;
    /// 把本机改成 `target`，返回本机收不下的正文的摘要。
    fn apply_phrases(
        &self,
        local: &[LocalPhrase],
        target: &[AccountPhrase],
    ) -> Result<BTreeSet<String>, String>;
    /// `awaited` 是状态里记着的、下载以后交给激活队列的请求 id。
    fn dictionary(&self, awaited: Option<&str>) -> Result<DictionaryLocal, String>;
    fn export_dictionary(&self, destination: &Path) -> Result<(), String>;
    /// 把下载到的快照交给激活队列，返回请求 id。
    fn replace_dictionary(
        &self,
        snapshot: &Path,
        account_id: &str,
        revision: i64,
    ) -> Result<String, String>;
    fn merge_dictionary(&self, snapshot: &Path) -> Result<(), MergeStop>;
    fn cancel_dictionary(&self, account_id: &str) -> Result<(), String>;
    /// 校验下载到的快照（与激活队列用的同一套规则），返回它的修订号。
    fn snapshot_revision(&self, snapshot: &Path) -> Result<i64, String>;
    /// 下载到的快照里有没有词（`entry` 减去标成删除的 `overlay`，与合并时看到的相同）。
    fn snapshot_has_words(&self, snapshot: &Path) -> Result<bool, String>;
}

// ---- 状态展示 ----

#[derive(Serialize)]
struct SectionView<'a> {
    cursor: Option<i64>,
    dirty: bool,
    cleared: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_outcome: Option<&'a SectionOutcome>,
}

/// 给宿主看的状态：不含基线和摘要。
pub(crate) fn status_value(state: &SyncState, now_ms: u64) -> Value {
    let sections: BTreeMap<&str, SectionView> = Section::ALL
        .iter()
        .map(|section| {
            let value = state.sections.get(*section);
            (
                section.as_str(),
                SectionView {
                    cursor: value.cursor,
                    dirty: value.dirty(),
                    cleared: value.cleared,
                    last_outcome: value.last_outcome.as_ref(),
                },
            )
        })
        .collect();
    let line = StatusLine {
        enabled: state.enabled,
        disabled_on_cloud_cleared: state.disabled_on_cloud_cleared,
        failure: state
            .failure
            .as_ref()
            .map(|failure| failure.message.as_str()),
        cloud_cleared: state.cloud_cleared(),
        skins_trimmed: state.notices.skins_trimmed,
        unheld_phrases: state.sections.phrases.unheld.len(),
        last_synced_ms: state.last_synced_ms,
        now_ms,
    }
    .render();
    json!({
        "enabled": state.enabled,
        "disabled_on_cloud_cleared": state.disabled_on_cloud_cleared,
        "account_id": (!state.account_id.is_empty()).then_some(&state.account_id),
        "sections": sections,
        "last_attempt_ms": state.last_attempt_ms,
        "last_synced_ms": state.last_synced_ms,
        "not_before_ms": state.backoff_until(now_ms),
        "pending_choice": state.pending_choice,
        "running": state.lease_active(now_ms),
        "failure": state.failure,
        "skins_trimmed": state.notices.skins_trimmed,
        "unheld_phrases": state.sections.phrases.unheld.len(),
        "status_line": line,
    })
}

fn storage<T>(_: T) -> String {
    STORAGE.to_owned()
}

/// 会话当前的账号和代次；没有登录时为 `None`。读不出会话（钥匙串不可用）时是错误：那不等于退出登录，调用方不对账。
fn identity<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
) -> Result<Option<(String, u64)>, AccountError> {
    session.with_current_identity(|identity| {
        Ok(identity.map(|(user, generation)| (user.to_owned(), generation)))
    })
}

/// `status`：与当前登录对齐后的状态。读不出会话时不对账，照样返回。
pub(crate) fn status<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    store: &SyncStateStore,
    now_ms: u64,
) -> Result<Value, String> {
    let identity = identity(session).ok();
    let state = store
        .update(|state| {
            if let Some(identity) = &identity {
                state.reconcile(identity.as_ref().map(|(user, _)| user.as_str()));
            }
            state.clone()
        })
        .map_err(storage)?;
    Ok(status_value(&state, now_ms))
}

/// `sign_out`：清空进度、代次加一，取消旧账号排队中的快照。
pub(crate) fn sign_out<L: SyncLocal>(
    store: &SyncStateStore,
    local: &L,
    now_ms: u64,
) -> Result<Value, String> {
    let (previous, state) = store
        .update_always(|state, pending| {
            let previous = std::mem::take(&mut state.account_id);
            state.reset("");
            *pending = None;
            (previous, state.clone())
        })
        .map_err(storage)?;
    let cancelled = previous.is_empty() || local.cancel_dictionary(&previous).is_ok();
    let mut value = status_value(&state, now_ms);
    value["snapshots_cancelled"] = Value::Bool(cancelled);
    Ok(value)
}

/// `set_enabled`：打开时要有登录的账号，并把全部分类标成待上传。
pub(crate) fn set_enabled<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    store: &SyncStateStore,
    enabled: bool,
    now_ms: u64,
) -> Result<Value, String> {
    let identity = identity(session).map_err(|error| error.code().to_owned())?;
    let state = store
        .update(|state| {
            state.reconcile(identity.as_ref().map(|(user, _)| user.as_str()));
            state.set_enabled(enabled).map(|()| state.clone())
        })
        .map_err(storage)?
        .map_err(|_| SIGNED_OUT.to_owned())?;
    Ok(status_value(&state, now_ms))
}

/// `peek`：只读状态文件，不读会话、不对账、不写任何东西。不持有登录的进程（键盘扩展、Android 的 `:ime`）用它看开关：它们的会话来源里可能看不到登录，调 `status` 会按「已经退出」对账，把共享的进度清空、开关关掉。
pub(crate) fn peek(store: &SyncStateStore, now_ms: u64) -> Result<Value, String> {
    let state = store.peek().map_err(storage)?;
    Ok(status_value(&state, now_ms))
}

/// `mark_dirty`：只做文件操作，不读会话。返回有没有记下（开关关闭时不记）。
pub(crate) fn mark_dirty(store: &SyncStateStore, section: Section) -> Result<Value, String> {
    let marked = store
        .update(|state| state.mark_dirty(section))
        .map_err(storage)?;
    Ok(json!({ "marked": marked }))
}

/// `dictionary_reset`：本机清除学习记录以后调用，只做文件操作，不读会话。先取消状态里绑定账号排在激活队列里、还没激活的云端快照（宿主带了队列目录时），再忘掉等激活的记录、给词库打待上传标记（[`SyncState::forget_dictionary_before_reset`]），下一轮把清除后的这一份整份上传，而不是把清除前下载的那份云端词并回来。返回 `{marked}`，开关关着时为假；取消失败时报 `cloud_sync_storage`，什么也没记，宿主可以再调一次。
pub(crate) fn dictionary_reset<L: SyncLocal>(
    store: &SyncStateStore,
    local: &L,
) -> Result<Value, String> {
    let account = store.peek().map_err(storage)?.account_id;
    if !account.is_empty() {
        local.cancel_dictionary(&account).map_err(storage)?;
    }
    let marked = store
        .update(|state| state.forget_dictionary_before_reset())
        .map_err(storage)?;
    Ok(json!({ "marked": marked }))
}

/// 旧版本的状态。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AndroidLegacy {
    sync_v1: Value,
    #[serde(default)]
    cloud_sync_status: Value,
}

/// `import_legacy`：只在还没有任何进度、而且没有绑定别的账号时导入（见 [`SyncState::accepts_import`]），保留游标和待上传标记，免得迁移以后又弹首次选择。Android 的词库游标还要对照快照激活队列 `snapshot_queue`，见 [`android_dictionary_activation`]。
pub(crate) fn import_legacy(
    store: &SyncStateStore,
    ios_v1: Option<Value>,
    android: Option<AndroidLegacy>,
    snapshot_queue: Option<&Path>,
    now_ms: u64,
) -> Result<Value, String> {
    let imported = match (ios_v1, android) {
        (Some(record), None) => import_ios_v1(&record),
        (None, Some(android)) => import_android(&android.sync_v1, &android.cloud_sync_status)
            .map(|state| android_dictionary_activation(state, snapshot_queue)),
        _ => return Err(INVALID.to_owned()),
    };
    let (imported, state) = store
        .update(|state| {
            let adopted = match imported {
                Some(imported) if state.accepts_import(&imported) => {
                    state.adopt(imported);
                    true
                }
                _ => false,
            };
            (adopted, state.clone())
        })
        .map_err(storage)?;
    Ok(json!({ "imported": imported, "status": status_value(&state, now_ms) }))
}

/// 旧的 Android `CloudSync` 下载词库时先把快照交给激活队列，同时就把游标记成云端的修订号。那份快照要是没有激活就结束了（本机词库变了而冲突、失败或被取消），本机缺着游标以前的云端词，导入的游标却说已经同步到那里：第一轮本机有标记、云端没变时会整份上传，用没收到云端词的本机词库替换云端。所以队列里还有这个账号没应用的请求时，把它记成等激活的快照（`activation`），第一轮按等激活或没激活处理，改按合并补上云端的词，不上传。读不出队列、或者宿主没给队列目录时不知道有没有这样的请求，词库游标不导入，词库下一轮按首次同步处理（会问用户），不冒整份覆盖云端的险。
fn android_dictionary_activation(mut state: SyncState, snapshot_queue: Option<&Path>) -> SyncState {
    if state.sections.dictionary.cursor.is_none() {
        return state;
    }
    let queued = snapshot_queue
        .and_then(|directory| DictionarySnapshotQueue::new(directory).ok())
        .and_then(|queue| queue.read().ok());
    let Some(queued) = queued else {
        state.sections.dictionary.cursor = None;
        return state;
    };
    if let Some(request) = queued.request.filter(|request| {
        request.account_id == state.account_id && request.status != SnapshotRequestStatus::Applied
    }) {
        state.sections.dictionary.activation = Some(request.id.to_string());
    }
    state
}

/// `run` 的参数。
pub(crate) struct RunRequest {
    pub(crate) trigger: Trigger,
    pub(crate) choice: Option<Choice>,
    pub(crate) capabilities: Capabilities,
    /// iOS 社区皮肤试用中，这一轮不同步设置。
    pub(crate) settings_locked: bool,
    /// Android 尚未认领的示例常用语。
    pub(crate) excluded_phrase_texts: Vec<String>,
}

fn skipped(reason: &str, state: &SyncState, now_ms: u64) -> Value {
    json!({ "outcome": "skipped", "reason": reason, "status": status_value(state, now_ms) })
}

/// `run` 带着 `choice:"disable"`：用户在「云端被清空」的提示里选了「关闭同步，保留本机」。不联网、不跑一轮，也不读写本机的设置、常用语和词库，只在状态锁里关掉开关（[`SyncState::disable_keeping_local`]）。账号的对齐与 `run` 相同；会话读不出来时不对齐也不关，与 `run` 一样是 `session_unavailable`。不能做时（[`disable_gate`]）是 `skipped`，原因是 `signed_out`、`disabled` 或 `not_cloud_cleared`，状态不变。
pub(crate) fn disable<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    store: &SyncStateStore,
    now_ms: u64,
) -> Result<Value, String> {
    let identity = match identity(session) {
        Ok(identity) => identity,
        Err(_) => {
            let state = store.read().map_err(storage)?;
            return Ok(skipped("session_unavailable", &state, now_ms));
        }
    };
    let (result, state) = store
        .update(|state| {
            state.reconcile(identity.as_ref().map(|(user, _)| user.as_str()));
            let result = disable_gate(state);
            if result.is_ok() {
                state.disable_keeping_local();
            }
            (result, state.clone())
        })
        .map_err(storage)?;
    Ok(match result {
        Ok(()) => json!({ "outcome": "disabled", "status": status_value(&state, now_ms) }),
        Err(reason) => skipped(reason.as_str(), &state, now_ms),
    })
}

/// 跑的过程中换了账号、退出后重新登录，或会话读不出来：这一轮作废，不写任何进度。
struct Abort;

/// 一轮里的联网和本机写入，都绑在开始那一轮的登录上。
struct Context<'a, A: AccountApi, S: AccountSessionStorage, L: SyncLocal> {
    session: &'a BackendAccountSession<A, S>,
    local: &'a L,
    user: &'a str,
    generation: u64,
    work: PathBuf,
}

impl<A: AccountApi, S: AccountSessionStorage, L: SyncLocal> Context<'_, A, S, L> {
    fn remote<T>(
        &self,
        operation: impl Fn(&A, &str) -> Result<T, AccountFailure>,
    ) -> Result<Result<T, AccountFailure>, Abort> {
        match self
            .session
            .authenticated_as(self.user, self.generation, operation)
        {
            Ok(value) => Ok(Ok(value)),
            Err(failure)
                if failure.error == AccountError::Cancelled && failure.status.is_none() =>
            {
                Err(Abort)
            }
            Err(failure) => Ok(Err(failure)),
        }
    }

    /// 本机写入：会话的代次和账号还是开始时那一次登录时才执行，执行期间退出和换号都要等它写完。闭包里只写本地文件。
    fn locally<T>(
        &self,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<Result<T, String>, Abort> {
        self.session
            .with_generation(self.generation, Some(self.user), || Ok(operation()))
            .map_err(|_| Abort)
    }
}

/// 状态机看的失败。没拿到响应、本机又没有会话（令牌已经清掉）时记成 `signed_out`，不是 `network`。
fn http_failure(failure: &AccountFailure) -> HttpFailure {
    let signed_out = failure.status.is_none() && failure.error == AccountError::Unauthorized;
    HttpFailure {
        status: failure.status,
        code: failure
            .server_code
            .clone()
            .or_else(|| signed_out.then(|| "signed_out".to_owned())),
        retry_after_seconds: failure.retry_after_seconds,
    }
}

/// 一类这一轮的数据。
struct Slot {
    machine: SectionMachine,
    /// 等驱动执行的下一步。
    step: Step,
    /// 本轮开始时读到的改动代数：提交时只清到这里。
    generation: u64,
    /// 设置时还有皮肤库的代数。
    skins_generation: Option<u64>,
    failure: Option<AccountFailure>,
    progress: Option<SectionProgress>,
}

struct SettingsData {
    schema: Option<AccountPreferenceSchema>,
    cloud: Option<AccountPreferences>,
    local: Option<LocalSettings>,
    saved: Option<AccountPreferences>,
    skins_trimmed: Option<bool>,
    /// 这一轮没有动皮肤库的原因：本机或云端的皮肤库读不懂，云端的那份原样留着。
    skins_skipped: Option<&'static str>,
    writes: HostWrites,
    /// iOS：留到提交时再写共享偏好的设置文档，和这一轮读本机时共享偏好的修订号。
    deferred: Option<(AccountPreferences, u64)>,
}

struct PhrasesData {
    cloud: Option<AccountPhrases>,
    local: Option<Vec<LocalPhrase>>,
    saved: Option<AccountPhrases>,
    unheld: Option<BTreeSet<String>>,
    adopt: Option<Vec<String>>,
}

struct DictionaryData {
    probe: Option<AccountDictionaryProbe>,
    /// 这一轮下载后交给激活队列的请求 id。
    enqueued: Option<String>,
    /// 这一轮合并往导入队列里排了新的词。
    queued: bool,
    /// 这一轮合并没做完时，云端快照里已经在本机的词数。
    present: Option<usize>,
    /// 这一轮读本机时本机词库有没有用户自己的词；还没读到时为 `None`。
    local_words: Option<bool>,
}

/// 云端文档里会写回本机、也会被上传的部分：去掉隐私键和字段表外的键。
fn syncable(
    settings: &BTreeMap<String, AccountPreferenceValue>,
    schema: &AccountPreferenceSchema,
) -> BTreeMap<String, AccountPreferenceValue> {
    let mut settings = settings.clone();
    policy::without_local_only(&mut settings);
    policy::within_schema(&mut settings, schema);
    settings
}

struct Round<'a, A: AccountApi, S: AccountSessionStorage, L: SyncLocal> {
    context: Context<'a, A, S, L>,
    state: SyncState,
    capabilities: Capabilities,
    excluded: HashSet<String>,
    settings: Option<(Slot, SettingsData)>,
    phrases: Option<(Slot, PhrasesData)>,
    dictionary: Option<(Slot, DictionaryData)>,
    locked_settings: bool,
    /// 词库在合并的退避里，这一轮跳过它。
    dictionary_stalled: bool,
    /// 词库这一轮用的选择：宿主带来的、另一轮在跑时记下的，或者词库合并没做完时留下的。
    dictionary_choice: Option<Choice>,
}

impl<A: AccountApi, S: AccountSessionStorage, L: SyncLocal> Round<'_, A, S, L> {
    fn input(&self, section: Section, choice: Option<Choice>) -> (SectionInput, u64, Option<u64>) {
        let value = self.state.sections.get(section);
        let mut dirty = value.dirty();
        let mut skins_generation = None;
        if section == Section::Settings && self.capabilities.skins_library {
            dirty |= self.state.sections.skins.dirty();
            skins_generation = Some(self.state.sections.skins.generation);
        }
        let base_present = value.base.as_ref().is_some_and(|base| !base.is_empty());
        (
            SectionInput {
                section,
                cursor: value.cursor,
                dirty,
                base_present,
                cleared: value.cleared,
                choice,
            },
            value.generation,
            skins_generation,
        )
    }

    fn slot(&self, section: Section, choice: Option<Choice>) -> Slot {
        let (input, generation, skins_generation) = self.input(section, choice);
        let mut machine = SectionMachine::new(input);
        let step = machine.next(None);
        Slot {
            machine,
            step,
            generation,
            skins_generation,
            failure: None,
            progress: None,
        }
    }

    // ---- 设置 ----

    fn settings_step(&mut self) -> Result<bool, Abort> {
        let Some((slot, data)) = self.settings.as_mut() else {
            return Ok(false);
        };
        let context = &self.context;
        let skins = self.capabilities.skins_library;
        let event = match slot.step.clone() {
            Step::Fetch => {
                let fetched = context.remote(|api, token| {
                    let schema = match &data.schema {
                        Some(schema) => schema.clone(),
                        None => api.preference_schema_detailed(token)?,
                    };
                    Ok((schema, api.preferences_detailed(token)?))
                })?;
                match fetched {
                    Ok((schema, cloud)) => {
                        let probe = CloudProbe {
                            revision: cloud.revision,
                            has_data: !cloud.settings.is_empty(),
                            reset_and_empty: false,
                        };
                        data.schema = Some(schema);
                        data.cloud = Some(cloud);
                        Event::Fetched(probe)
                    }
                    Err(failure) => {
                        let event = Event::HttpFailed(http_failure(&failure));
                        slot.failure = Some(failure);
                        event
                    }
                }
            }
            Step::ReadLocal => match context.local.export_settings() {
                Ok(local) => {
                    let schema = data.schema.as_ref().expect("schema is read first");
                    let has_data = !syncable(&local.settings, schema).is_empty()
                        || skins
                            && local
                                .skins
                                .as_deref()
                                .is_some_and(|library| library != "[]");
                    data.local = Some(local);
                    Event::LocalRead(LocalProbe {
                        has_data,
                        content_dirty: false,
                        upload_blocked: false,
                        activation: SnapshotActivation::Idle,
                    })
                }
                Err(reason) => Event::LocalFailed { reason },
            },
            Step::Put(mode) => {
                let schema = data.schema.clone().expect("schema is read first");
                let cloud = data.cloud.clone().expect("cloud is read first");
                let local = data.local.as_ref().expect("local is read first");
                // 这一轮发现云端被清空时，旧基线已经不作数：按首次合并处理，不能把「云端删光」当成云端删了这些键再传到本机。
                let base = if slot.machine.awaiting_choice() {
                    None
                } else {
                    self.state.sections.settings.base_keys().cloned()
                };
                match settings_upload(&cloud, local, base.as_ref(), &schema, skins, mode) {
                    Err(reason) => Event::LocalFailed { reason },
                    Ok((document, trimmed, skipped)) => {
                        data.skins_trimmed = trimmed;
                        data.skins_skipped = skipped;
                        match context
                            .remote(|api, token| api.put_preferences_detailed(&document, token))?
                        {
                            Ok(saved) => {
                                let revision = saved.revision;
                                data.saved = Some(saved);
                                Event::Written { revision }
                            }
                            Err(failure) => {
                                let event = Event::HttpFailed(http_failure(&failure));
                                slot.failure = Some(failure);
                                event
                            }
                        }
                    }
                }
            }
            Step::WriteLocal(target) => {
                let schema = data.schema.clone().expect("schema is read first");
                let mut document = match target {
                    WriteTarget::Saved => data.saved.clone(),
                    _ => data.cloud.clone(),
                }
                .expect("document is read first");
                document.settings = syncable(&document.settings, &schema);
                if !skins {
                    document.settings.remove(policy::SKINS_KEY);
                }
                let revision = document.revision;
                let read_revision = data.local.as_ref().expect("local is read first").revision;
                match context.locally(|| {
                    context
                        .local
                        .apply_settings(&document, &schema, read_revision)
                })? {
                    Ok(applied) => {
                        if applied.deferred {
                            data.deferred = Some((document, read_revision));
                        }
                        data.writes.absorb(applied.writes);
                        Event::Written { revision }
                    }
                    Err(reason) => Event::LocalFailed { reason },
                }
            }
            Step::Commit(commit) => {
                let schema = data.schema.as_ref().expect("schema is read first");
                let document = data
                    .saved
                    .as_ref()
                    .or(data.cloud.as_ref())
                    .expect("document is read first");
                let base =
                    if commit.mode == Mode::None && self.state.sections.settings.base.is_some() {
                        BaseChange::Keep
                    } else {
                        BaseChange::Set(SectionBase::Keys(policy::settings_digests(&syncable(
                            &document.settings,
                            schema,
                        ))))
                    };
                slot.progress = Some(committed(slot, commit, base, None));
                return Ok(advance(slot, None));
            }
            Step::NeedsChoice(_) | Step::Done(_) => return Ok(false),
        };
        Ok(advance(slot, Some(event)))
    }

    // ---- 常用语 ----

    fn phrases_step(&mut self) -> Result<bool, Abort> {
        let Some((slot, data)) = self.phrases.as_mut() else {
            return Ok(false);
        };
        let context = &self.context;
        let section = &self.state.sections.phrases;
        // 这一轮发现云端被清空时不用旧基线：否则合并会把基线里有、云端没有的本机常用语当成在云端被删掉，再从本机删掉。
        let base = if slot.machine.awaiting_choice() {
            None
        } else {
            section.base_texts().cloned()
        };
        let unheld = section.unheld_set();
        let event = match slot.step.clone() {
            Step::Fetch => match context.remote(|api, token| api.phrases(token))? {
                Ok(cloud) => {
                    let probe = CloudProbe {
                        revision: cloud.revision,
                        has_data: !cloud.phrases.is_empty(),
                        reset_and_empty: false,
                    };
                    data.cloud = Some(cloud);
                    Event::Fetched(probe)
                }
                Err(failure) => {
                    let event = Event::HttpFailed(http_failure(&failure));
                    slot.failure = Some(failure);
                    event
                }
            },
            Step::ReadLocal => match context.local.phrases() {
                Ok(local) => {
                    let cloud = data.cloud.as_ref().expect("cloud is read first");
                    let local: Vec<LocalPhrase> = local
                        .into_iter()
                        .filter(|phrase| !self.excluded.contains(&phrase.text))
                        .collect();
                    let probe = LocalProbe {
                        has_data: !local.is_empty(),
                        content_dirty: policy::phrases_content_dirty(
                            &local,
                            &cloud.phrases,
                            base.as_ref(),
                            &unheld,
                        ),
                        upload_blocked: false,
                        activation: SnapshotActivation::Idle,
                    };
                    data.local = Some(local);
                    Event::LocalRead(probe)
                }
                Err(reason) => Event::LocalFailed { reason },
            },
            Step::Put(_) => {
                let cloud = data.cloud.clone().expect("cloud is read first");
                let local = data.local.as_ref().expect("local is read first");
                let merged = policy::merge_phrases(local, &cloud.phrases, base.as_ref(), &unheld);
                match policy::repair_phrases(merged, cloud.revision, policy::generated_phrase_id) {
                    Err(problem) => Event::LocalFailed {
                        reason: problem.reason().to_owned(),
                    },
                    Ok(phrases) => {
                        let upload = AccountPhrases {
                            revision: cloud.revision,
                            phrases,
                        };
                        match context.remote(|api, token| api.put_phrases(&upload, token))? {
                            Ok(saved) => {
                                let revision = saved.revision;
                                data.saved = Some(saved);
                                Event::Written { revision }
                            }
                            Err(failure) => {
                                let event = Event::HttpFailed(http_failure(&failure));
                                slot.failure = Some(failure);
                                event
                            }
                        }
                    }
                }
            }
            Step::WriteLocal(target) => {
                let document = match target {
                    WriteTarget::Saved => data.saved.clone(),
                    _ => data.cloud.clone(),
                }
                .expect("document is read first");
                let local = data.local.clone().expect("local is read first");
                let revision = document.revision;
                match context.locally(|| context.local.apply_phrases(&local, &document.phrases))? {
                    Ok(unheld) => {
                        let adopt: Vec<String> = document
                            .phrases
                            .iter()
                            .filter(|phrase| self.excluded.contains(&phrase.text))
                            .map(|phrase| phrase.text.clone())
                            .collect();
                        data.unheld = Some(unheld);
                        data.adopt = (!adopt.is_empty()).then_some(adopt);
                        Event::Written { revision }
                    }
                    Err(reason) => Event::LocalFailed { reason },
                }
            }
            Step::Commit(commit) => {
                let document = data
                    .saved
                    .as_ref()
                    .or(data.cloud.as_ref())
                    .expect("document is read first");
                let base = if commit.mode == Mode::None && base.is_some() {
                    BaseChange::Keep
                } else {
                    BaseChange::Set(SectionBase::Texts(policy::text_digests(
                        document.phrases.iter().map(|phrase| phrase.text.as_str()),
                    )))
                };
                let unheld = data
                    .unheld
                    .as_ref()
                    .map(|unheld| unheld.iter().cloned().collect());
                slot.progress = Some(committed(slot, commit, base, unheld));
                return Ok(advance(slot, None));
            }
            Step::NeedsChoice(_) | Step::Done(_) => return Ok(false),
        };
        Ok(advance(slot, Some(event)))
    }

    // ---- 个人词库 ----

    fn dictionary_step(&mut self) -> Result<bool, Abort> {
        let Some((slot, data)) = self.dictionary.as_mut() else {
            return Ok(false);
        };
        let context = &self.context;
        let cursor = self.state.sections.dictionary.cursor;
        let awaited = self.state.sections.dictionary.activation.as_deref();
        let event = match slot.step.clone() {
            Step::Fetch => {
                let after = cursor.unwrap_or(0);
                match context.remote(|api, token| api.dictionary_probe(after, token))? {
                    Err(failure) => {
                        let event = Event::HttpFailed(http_failure(&failure));
                        slot.failure = Some(failure);
                        event
                    }
                    Ok(probe) => {
                        // 第一条新改动是 reset 时再看云端是不是空了：「删除云端数据」会清掉全部历史再写一条 reset；别的设备整份上传快照也写 reset，但云端不空。从没同步过（没有游标）时不管第一条改动是什么都要看：删除以后云端只剩那条 reset；在云词库面板里加了词又全删掉，改动都不是 reset，词库却是空的。这两种都不能当成云端有词，否则「使用云端」会拿空快照替换本机的词。读不出汇总时按不空处理。
                        let reset_and_empty = probe.changed
                            && (probe.reset || cursor.is_none())
                            && matches!(
                                context.remote(|api, token| api.data_summary(token))?,
                                Ok(summary) if summary.sections.iter().any(|section| section.id == "dictionary" && section.items == 0)
                            );
                        data.probe = Some(probe);
                        Event::Fetched(CloudProbe {
                            revision: probe.revision,
                            has_data: probe.changed && !reset_and_empty,
                            reset_and_empty,
                        })
                    }
                }
            }
            Step::ReadLocal => match context.local.dictionary(awaited) {
                Ok(local) => Event::LocalRead({
                    data.local_words = Some(local.has_words);
                    LocalProbe {
                        has_data: local.has_words,
                        content_dirty: false,
                        upload_blocked: policy::upload_blocked(
                            local.pending,
                            local.activation == SnapshotActivation::Waiting,
                        ),
                        activation: local.activation,
                    }
                }),
                Err(reason) => Event::LocalFailed { reason },
            },
            Step::Put(_) => {
                let revision = data.probe.map_or(0, |probe| probe.revision);
                let file = context.work.join("upload.ndjson");
                let exported = context.local.export_dictionary(&file);
                let event = match exported {
                    Err(reason) => Event::LocalFailed { reason },
                    Ok(()) => match context.remote(|api, token| {
                        api.dictionary_snapshot_upload(&file, revision, token)
                    })? {
                        Ok(restored) => Event::Written {
                            revision: restored.revision,
                        },
                        Err(failure) => {
                            let event = Event::HttpFailed(http_failure(&failure));
                            slot.failure = Some(failure);
                            event
                        }
                    },
                };
                let _ = std::fs::remove_file(&file);
                event
            }
            Step::WriteLocal(target) => {
                let file = context.work.join("download.ndjson");
                let _ = std::fs::remove_file(&file);
                let event = match context
                    .remote(|api, token| api.dictionary_snapshot_download(&file, token))?
                {
                    Err(failure) => {
                        let event = Event::HttpFailed(http_failure(&failure));
                        slot.failure = Some(failure);
                        event
                    }
                    // 硬性约束：本机有自己的词时，从不拿一个词都没有的快照整份替换它。首次同步读不出云端汇总时云端按有词处理，「使用云端」走到这里拿到的可能是空快照；之后的下载同样查。不替换，交给状态机改按合并处理，两边的词都留着。
                    Ok(_) => match context.local.snapshot_revision(&file).and_then(|revision| {
                        let empty = target == WriteTarget::Cloud
                            && data.local_words != Some(false)
                            && !context.local.snapshot_has_words(&file)?;
                        Ok((revision, empty))
                    }) {
                        Err(reason) => Event::LocalFailed { reason },
                        Ok((_, true)) => Event::EmptySnapshot,
                        Ok((revision, false)) => {
                            let user = context.user;
                            let queued = std::cell::Cell::new(false);
                            let present = std::cell::Cell::new(None);
                            let written = context.locally(|| match target {
                                WriteTarget::MergeIntoLocal => context
                                    .local
                                    .merge_dictionary(&file)
                                    .map(|()| None)
                                    .map_err(|stop| {
                                        queued.set(stop.queued);
                                        present.set(stop.present);
                                        stop.reason
                                    }),
                                _ => context
                                    .local
                                    .replace_dictionary(&file, user, revision)
                                    .map(Some),
                            })?;
                            data.queued |= queued.get();
                            data.present = present.get().or(data.present);
                            match written {
                                Ok(enqueued) => {
                                    data.enqueued = enqueued;
                                    Event::Written { revision }
                                }
                                Err(reason) => Event::LocalFailed { reason },
                            }
                        }
                    },
                };
                let _ = std::fs::remove_file(&file);
                event
            }
            Step::Commit(commit) => {
                let mut progress = committed(slot, commit, BaseChange::Keep, None);
                // 下载以后记下交给激活队列的请求，下一轮据此知道它激活了没有。
                progress.activation = match (commit.mode, &data.enqueued) {
                    (Mode::Download, Some(id)) => ActivationChange::Set(id.clone()),
                    _ if commit.clear_activation => ActivationChange::Clear,
                    _ => ActivationChange::Keep,
                };
                slot.progress = Some(progress);
                return Ok(advance(slot, None));
            }
            Step::NeedsChoice(_) | Step::Done(_) => return Ok(false),
        };
        // 联网失败的那一步可能是下载以后的本机写入；`HttpFailed` 只在联网失败时出现。
        Ok(advance(slot, Some(event)))
    }

    fn step(&mut self, section: Section) -> Result<bool, Abort> {
        match section {
            Section::Settings => self.settings_step(),
            Section::Phrases => self.phrases_step(),
            Section::Dictionary => self.dictionary_step(),
            Section::Skins => Ok(false),
        }
    }

    fn slot_ref(&self, section: Section) -> Option<&Slot> {
        match section {
            Section::Settings => self.settings.as_ref().map(|(slot, _)| slot),
            Section::Phrases => self.phrases.as_ref().map(|(slot, _)| slot),
            Section::Dictionary => self.dictionary.as_ref().map(|(slot, _)| slot),
            Section::Skins => None,
        }
    }
}

/// 把事件喂给状态机，返回还要不要接着执行。
fn advance(slot: &mut Slot, event: Option<Event>) -> bool {
    slot.step = slot.machine.next(event);
    !matches!(slot.step, Step::NeedsChoice(_) | Step::Done(_))
}

fn committed(
    slot: &Slot,
    commit: SectionCommit,
    base: BaseChange,
    unheld: Option<Vec<String>>,
) -> SectionProgress {
    SectionProgress {
        cursor: Some(commit.cursor),
        clean_to: commit.clear_dirty.then_some(slot.generation),
        mark_dirty: commit.mark_dirty,
        base,
        unheld,
        cleared: commit.reset_cleared.then_some(false),
        activation: ActivationChange::Keep,
        stall: StallChange::Keep,
        outcome: SectionOutcome::new(OutcomeKind::Synced, None),
    }
}

/// 共享偏好在这一轮读本机以后被别处改过（另一个进程里用户改了设置）：不拿云端的值覆盖它，这一类这一轮跳过、再标一次待上传，游标和基线不动，下一轮按三方合并。
pub(crate) const SETTINGS_CHANGED: &str = "settings_changed";

/// 本机的皮肤库（宿主给的 `custom_keyboard_skins`）不是 JSON 数组。
const LOCAL_SKINS_UNREADABLE: &str = "skins_library_unreadable";
/// 云端的皮肤库不是 JSON 数组（可能是更新的版本写的）。
const CLOUD_SKINS_UNREADABLE: &str = "cloud_skins_library_unreadable";

/// 要上传的设置文档：云端文档（去掉隐私键和字段表外的键）按键三方合并本机的值；有皮肤库能力时按剩下的预算放进裁剪后的皮肤库，合并时先并上云端的设计。本机或云端的皮肤库读不懂时这个键原样用云端的值（云端没有就不放），不拿另一份替换它。返回文档、皮肤库有没有被裁剪，和皮肤库没有动的原因。
fn settings_upload(
    cloud: &AccountPreferences,
    local: &LocalSettings,
    base: Option<&BTreeMap<String, String>>,
    schema: &AccountPreferenceSchema,
    skins: bool,
    mode: Mode,
) -> Result<(AccountPreferences, Option<bool>, Option<&'static str>), String> {
    let cloud_settings = syncable(&cloud.settings, schema);
    let mut local_settings = syncable(&local.settings, schema);
    local_settings.remove(policy::SKINS_KEY);
    if policy::settings_type_mismatch(&local_settings, schema) {
        return Err("schema_type_mismatch".to_owned());
    }
    let mut settings = policy::merge_settings(&cloud_settings, &local_settings, base);
    let mut trimmed = None;
    let mut skipped = None;
    let skins_field = schema.fields.contains_key(policy::SKINS_KEY);
    if skins && skins_field {
        if let Some(library) = &local.skins {
            settings.remove(policy::SKINS_KEY);
            let other =
                serde_json::to_vec(&json!({ "revision": cloud.revision, "settings": settings }))
                    .map_or(usize::MAX, |bytes| bytes.len());
            let existing = cloud_settings.get(policy::SKINS_KEY);
            // 云端的库：没有、读得懂，或者读不懂（不是字符串、不是 JSON 数组）。
            let cloud_library = match existing {
                None => Ok(None),
                Some(AccountPreferenceValue::String(text))
                    if policy::skin_library_readable(text) =>
                {
                    Ok(Some(text.as_str()))
                }
                Some(_) => Err(CLOUD_SKINS_UNREADABLE),
            };
            let library = match cloud_library {
                _ if !policy::skin_library_readable(library) => Err(LOCAL_SKINS_UNREADABLE),
                Err(reason) => Err(reason),
                Ok(Some(cloud_library)) if mode == Mode::Merge => {
                    policy::merge_skin_libraries(library, cloud_library)
                        .ok_or(CLOUD_SKINS_UNREADABLE)
                }
                Ok(_) => Ok(library.clone()),
            };
            match library.and_then(|library| {
                policy::trim_skin_library(&library, policy::skin_budget(other))
                    .ok_or(LOCAL_SKINS_UNREADABLE)
            }) {
                Ok((kept, total, count)) => {
                    trimmed = Some(count < total);
                    settings.insert(
                        policy::SKINS_KEY.to_owned(),
                        AccountPreferenceValue::String(kept),
                    );
                }
                // 本机或云端的皮肤库读不懂：这个键原样用云端的值。服务端整份替换设置文档，不放回去就等于因为一份读不懂的本机值删掉了云端的皮肤库；云端那份读不懂时（可能是更新的版本写的）也不能拿本机的库整份替换它。
                Err(reason) => {
                    if let Some(existing) = existing {
                        settings.insert(policy::SKINS_KEY.to_owned(), existing.clone());
                    }
                    skipped = Some(reason);
                }
            }
        }
    } else {
        // 本机不同步皮肤库（iOS）：云端的皮肤库是别的平台上传的，原样留着。服务端整份替换设置文档，去掉这个键就等于从云端删掉了它。
        match cloud_settings.get(policy::SKINS_KEY) {
            Some(library) => {
                settings.insert(policy::SKINS_KEY.to_owned(), library.clone());
            }
            None => {
                settings.remove(policy::SKINS_KEY);
            }
        }
    }
    let document = AccountPreferences {
        revision: cloud.revision,
        settings,
    };
    let size = serde_json::to_vec(&document).map_or(usize::MAX, |bytes| bytes.len());
    if size > schema.maximum_bytes.min(1024 * 1024)
        || msime_client_services::account::validate_account_preferences(&document).is_err()
    {
        return Err("settings_too_large".to_owned());
    }
    Ok((document, trimmed, skipped))
}

/// `run`：跑一轮。返回 `{outcome, reason?, run_id?, host_writes?, sections?, status}`，见头文件。
pub(crate) fn run<A: AccountApi, S: AccountSessionStorage, L: SyncLocal>(
    session: &BackendAccountSession<A, S>,
    store: &SyncStateStore,
    local: &L,
    request: RunRequest,
    now_ms: u64,
) -> Result<Value, String> {
    let identity = match identity(session) {
        Ok(identity) => identity,
        Err(_) => {
            let state = store.read().map_err(storage)?;
            return Ok(skipped("session_unavailable", &state, now_ms));
        }
    };
    let user = identity.as_ref().map(|(user, _)| user.clone());
    let capabilities = request.capabilities;
    let participating: Vec<Section> = [
        (
            capabilities.settings && !request.settings_locked,
            Section::Settings,
        ),
        (
            capabilities.settings && capabilities.skins_library && !request.settings_locked,
            Section::Skins,
        ),
        (capabilities.phrases, Section::Phrases),
        (capabilities.dictionary, Section::Dictionary),
    ]
    .into_iter()
    .filter_map(|(enabled, section)| enabled.then_some(section))
    .collect();
    // 宿主这一次带来的选择才不等节流；状态里记着的选择（`pending_choice`、词库合并没做完时留下的）照样等。
    let forced = request.trigger.forced() || request.choice.is_some();
    let (begun, snapshot, dictionary_stalled) = store
        .update(|state| {
            state.reconcile(user.as_deref());
            let choice = request.choice.or(state.pending_choice);
            // 词库合并在退避里：回到前台时这一轮不碰词库，它的待上传标记也不把节流缩到 30 秒。另一轮在跑时用户做的选择（`pending_choice`）这一轮就用掉，词库这时跳过就把它给词库的那一份丢了，下一轮又要问，所以记着它时词库这一轮不等退避；它不让这一轮跳过节流，用掉以后照常退避。
            let dictionary_stalled = capabilities.dictionary
                && !forced
                && state.pending_choice.is_none()
                && state.sections.dictionary.stalled(now_ms);
            let gated: Vec<Section> = participating
                .iter()
                .copied()
                .filter(|section| !(dictionary_stalled && *section == Section::Dictionary))
                .collect();
            let begun = match gate(
                state,
                now_ms,
                request.trigger,
                request.choice.is_some(),
                &gated,
            ) {
                Err(reason) => {
                    // 另一轮还在跑时做出的选择记下来，下一轮用它，不丢掉。
                    if let (Some(choice), "busy") = (request.choice, reason.as_str()) {
                        state.pending_choice = Some(choice);
                    }
                    Err(reason)
                }
                Ok(()) => {
                    let user = user.as_deref().expect("gate requires an account");
                    Ok(state.begin_run(user, now_ms, choice))
                }
            };
            (begun, state.clone(), dictionary_stalled)
        })
        .map_err(storage)?;
    let lease = match begun {
        Ok(lease) => lease,
        Err(reason) => return Ok(skipped(reason.as_str(), &snapshot, now_ms)),
    };
    let (user, generation) = identity.expect("gate requires an account");
    // 词库快照的中转文件放在状态目录下面；状态锁已经建好了状态目录。
    let work = store.directory().join("work");
    let dictionary = capabilities.dictionary && !dictionary_stalled;
    if dictionary && std::fs::create_dir_all(&work).is_err() {
        let state = store
            .update(|state| {
                state.release(&lease.id);
                state.clone()
            })
            .map_err(storage)?;
        return Ok(skipped("dictionary_storage", &state, now_ms));
    }
    let choice = lease.choice;
    let dictionary_choice = choice.or(snapshot.sections.dictionary.choice);
    let mut round = Round {
        context: Context {
            session,
            local,
            user: &user,
            generation,
            work,
        },
        state: snapshot,
        capabilities: request.capabilities,
        excluded: request.excluded_phrase_texts.into_iter().collect(),
        settings: None,
        phrases: None,
        dictionary: None,
        locked_settings: request.settings_locked,
        dictionary_stalled,
        dictionary_choice,
    };
    if capabilities.settings && !round.locked_settings {
        let slot = round.slot(Section::Settings, choice);
        round.settings = Some((
            slot,
            SettingsData {
                schema: None,
                cloud: None,
                local: None,
                saved: None,
                skins_trimmed: None,
                skins_skipped: None,
                writes: HostWrites::default(),
                deferred: None,
            },
        ));
    }
    if capabilities.phrases {
        let slot = round.slot(Section::Phrases, choice);
        round.phrases = Some((
            slot,
            PhrasesData {
                cloud: None,
                local: None,
                saved: None,
                unheld: None,
                adopt: None,
            },
        ));
    }
    if dictionary {
        // 设置和常用语只用宿主带来的或 `pending_choice` 里的选择；词库合并没做完时留下的选择只给词库。
        let slot = round.slot(Section::Dictionary, dictionary_choice);
        round.dictionary = Some((
            slot,
            DictionaryData {
                probe: None,
                enqueued: None,
                queued: false,
                present: None,
                local_words: None,
            },
        ));
    }
    let sections = [Section::Settings, Section::Phrases, Section::Dictionary];
    let outcome = (|| -> Result<(), Abort> {
        // 先把每一类都读到能定模式为止。
        for section in sections {
            while round
                .slot_ref(section)
                .is_some_and(|slot| matches!(slot.step, Step::Fetch | Step::ReadLocal))
            {
                if !round.step(section)? {
                    break;
                }
            }
        }
        if sections.iter().any(|section| {
            round
                .slot_ref(*section)
                .is_some_and(|slot| matches!(slot.step, Step::NeedsChoice(_)))
        }) {
            return Ok(());
        }
        for section in sections {
            while round.step(section)? {}
        }
        Ok(())
    })();
    if outcome.is_err() {
        let state = store
            .update(|state| {
                state.release(&lease.id);
                state.clone()
            })
            .map_err(storage)?;
        return Ok(skipped(STALE_RUN, &state, now_ms));
    }
    finish(store, round, &lease, request.trigger, now_ms)
}

fn finish<A: AccountApi, S: AccountSessionStorage, L: SyncLocal>(
    store: &SyncStateStore,
    round: Round<'_, A, S, L>,
    lease: &RunLease,
    trigger: Trigger,
    now_ms: u64,
) -> Result<Value, String> {
    let run_id = lease.id.as_str();
    let round_dictionary_choice = round.dictionary_choice;
    let dictionary_stalled = round.dictionary_stalled;
    let user = lease.user_id.as_str();
    let mut progress = RoundProgress::default();
    let mut outcomes: BTreeMap<Section, SectionOutcome> = BTreeMap::new();
    let mut modes = BTreeMap::new();
    let mut needs_choice = None;
    let mut writes = HostWrites::default();
    let mut deferred = None;
    let mut slots: Vec<(Section, Slot)> = Vec::new();
    // 词库还没同步过（或云端被清空后等着选择）时这一轮用的是用户的选择；合并没排完、或者键盘还没发布过本机词库版本时把选择留给下一轮，不再问一次。
    let dictionary_needs_choice = {
        let dictionary = &round.state.sections.dictionary;
        dictionary.cursor.is_none() || dictionary.cleared
    };
    let previous_stall = round.state.sections.dictionary.stall;
    let mut keep_choice = false;
    // 留给词库的选择改记成「合并」：见下面 `dictionary_unpublished` 那一条。
    let mut keep_as_merge = false;
    let mut dictionary_queued = false;
    let mut dictionary_present = None;
    let mut dictionary_local_words = None;
    let mut skins_skipped = None;
    if dictionary_stalled {
        let outcome = SectionOutcome::new(OutcomeKind::Skipped, Some(DICTIONARY_BACKOFF));
        progress.sections.insert(
            Section::Dictionary,
            SectionProgress::outcome(outcome.clone()),
        );
        outcomes.insert(Section::Dictionary, outcome);
    }
    if let Some((slot, data)) = round.settings {
        writes.absorb(data.writes);
        deferred = data.deferred;
        if let Some(trimmed) = data.skins_trimmed {
            progress.skins_trimmed = Some(trimmed);
        }
        skins_skipped = data.skins_skipped;
        slots.push((Section::Settings, slot));
    } else if round.locked_settings && round.capabilities.settings {
        outcomes.insert(
            Section::Settings,
            SectionOutcome::new(OutcomeKind::Skipped, Some("settings_locked")),
        );
    }
    if let Some((slot, data)) = round.phrases {
        if let Some(adopt) = data.adopt {
            writes.absorb(HostWrites {
                adopt_texts: Some(adopt),
                ..HostWrites::default()
            });
        }
        slots.push((Section::Phrases, slot));
    }
    if let Some((slot, data)) = round.dictionary {
        dictionary_queued = data.queued;
        dictionary_present = data.present;
        dictionary_local_words = data.local_words;
        slots.push((Section::Dictionary, slot));
    }
    let mut failed = false;
    for (section, slot) in slots {
        let mut outcome = slot
            .machine
            .outcome()
            .cloned()
            .unwrap_or_else(|| SectionOutcome::new(OutcomeKind::Skipped, Some("not_run")));
        // 写回本机时共享偏好已经被别处改过：不算失败，这一类跳过、再标一次待上传，游标和基线不动，下一轮按三方合并，不丢那次修改。
        let local_changed = outcome.outcome == OutcomeKind::Failed
            && outcome.reason.as_deref() == Some(SETTINGS_CHANGED);
        if local_changed {
            outcome = SectionOutcome::new(OutcomeKind::Skipped, Some(SETTINGS_CHANGED));
        }
        modes.insert(section, slot.machine.mode());
        match outcome.outcome {
            OutcomeKind::NeedsChoice => {
                let reason = if slot.machine.awaiting_choice() {
                    ChoiceReason::CloudCleared
                } else {
                    ChoiceReason::FirstRun
                };
                if needs_choice.is_none() || reason == ChoiceReason::CloudCleared {
                    needs_choice = Some(reason);
                }
                let mut change = SectionProgress::outcome(outcome.clone());
                // 新发现云端被清空：记下来、丢掉基线，选择之前这一类按首次处理。
                if slot.machine.awaiting_choice() {
                    change.cleared = Some(true);
                    change.base = BaseChange::Clear;
                }
                progress.sections.insert(section, change);
            }
            OutcomeKind::Failed => {
                failed = true;
                let merge_unfinished = section == Section::Dictionary
                    && outcome.reason.as_deref().is_some_and(merge_stalled);
                keep_choice |= merge_unfinished && dictionary_needs_choice;
                // 本机这一侧的失败（`dictionary_storage`、`dictionary_invalid` 这类，没有联网失败的详情）也退避：它们多半一直在，词库带着待上传标记时每满 30 秒就会重下一次整份快照。联网失败照旧只看 `Retry-After`。选择只在合并没做完时留下。
                let stalled =
                    merge_unfinished || section == Section::Dictionary && slot.failure.is_none();
                // 键盘还没发布过本机词库版本时本机没有自己的词，「合并」和「使用云端」这一轮都是下载云端快照，用户的选择在这时没有区别。留到以后的那一轮时键盘多半已经打开过、学到了词，留着「使用云端」就会拿云端快照把这些词整份替换掉，所以改记成「合并」。本机这一轮就有词、用户选了「使用云端」时照原样留着。
                keep_as_merge |= section == Section::Dictionary
                    && outcome.reason.as_deref() == Some(DICTIONARY_UNPUBLISHED)
                    && dictionary_local_words != Some(true);
                if progress.failure.is_none() {
                    progress.failure = Some(failure_record(section, &slot, &outcome));
                }
                if let Some(seconds) = slot.failure.as_ref().and_then(|f| f.retry_after_seconds) {
                    let at = now_ms.saturating_add(seconds.saturating_mul(1000));
                    progress.not_before_ms =
                        Some(progress.not_before_ms.map_or(at, |old| old.max(at)));
                }
                let mut change = SectionProgress::outcome(outcome.clone());
                // 合并没做完或本机读写失败：退避一阵再自动重试，不在每次回到前台时都重新下载整份快照。这一轮排进了新的词、或者云端的词在本机的比上次记下的多（键盘应用了排着的词）就从头数，没有进展才翻倍；只重排了键盘暂时没应用的请求不算进展。
                if stalled {
                    let present = dictionary_present.map(|count| count as u64);
                    let before = previous_stall.and_then(|stall| stall.present);
                    let progressed = dictionary_queued
                        || matches!((present, before), (Some(now), Some(before)) if now > before);
                    let rounds = match previous_stall {
                        Some(stall) if !progressed => stall.rounds.saturating_add(1),
                        _ => 1,
                    };
                    change.stall = StallChange::Set(Stall {
                        rounds,
                        until_ms: now_ms.saturating_add(policy::stall_ms(rounds)),
                        present: present.or(before),
                    });
                }
                progress.sections.insert(section, change);
            }
            OutcomeKind::Synced | OutcomeKind::Unchanged => {
                let mut change = slot
                    .progress
                    .clone()
                    .unwrap_or_else(|| SectionProgress::outcome(outcome.clone()));
                change.outcome = outcome.clone();
                if section == Section::Dictionary {
                    change.stall = StallChange::Clear;
                }
                if section == Section::Settings {
                    if let Some(generation) = slot.skins_generation {
                        // 皮肤库读不懂、这一轮没有动它：设置照常推进，皮肤库在 sections 里单独报出来。标记照样清掉，读不懂的库再传几次也一样，不能让它一直把节流缩到 30 秒。
                        let skins_outcome = match skins_skipped {
                            Some(reason) => {
                                let skipped =
                                    SectionOutcome::new(OutcomeKind::Skipped, Some(reason));
                                outcomes.insert(Section::Skins, skipped.clone());
                                skipped
                            }
                            None => outcome.clone(),
                        };
                        progress.sections.insert(
                            Section::Skins,
                            SectionProgress {
                                cursor: change.cursor,
                                clean_to: change.clean_to.map(|_| generation),
                                ..SectionProgress::outcome(skins_outcome)
                            },
                        );
                    }
                }
                progress.sections.insert(section, change);
            }
            OutcomeKind::Skipped => {
                progress.sections.insert(
                    section,
                    SectionProgress {
                        mark_dirty: local_changed,
                        ..SectionProgress::outcome(outcome.clone())
                    },
                );
            }
        }
        outcomes.insert(section, outcome);
    }
    // 云端被清空也可能是在写的阶段（写入冲突后重读）才发现的，这时排在前面的分类可能已经写了云端或本机：不能把它们的进度连同要宿主写的值一起丢掉，照常提交，问选择的那一类在上面已经记下清空、丢掉基线，下一轮读完云端就会问。
    let wrote = !writes.is_empty()
        || deferred.is_some()
        || outcomes
            .values()
            .any(|outcome| outcome.outcome == OutcomeKind::Synced);
    if let Some(reason) = needs_choice.filter(|_| !wrote) {
        let result = store
            .update(|state| {
                if !state.lease_matches(run_id, user) {
                    return None;
                }
                progress.prompted =
                    policy::prompt_due(now_ms, state.prompted_at_ms, trigger.forced());
                // 别的分类这一轮什么都没写：只留下问选择的和读云端就失败的（失败原因和退避照样记下）。
                progress.sections.retain(|_, change| {
                    change.mark_dirty
                        || matches!(
                            change.outcome.outcome,
                            OutcomeKind::NeedsChoice | OutcomeKind::Failed
                        )
                });
                progress.skins_trimmed = None;
                state.finish_round(&progress, now_ms);
                Some((progress.prompted, state.clone()))
            })
            .map_err(storage)?;
        let Some((prompted, state)) = result else {
            let state = store.read().map_err(storage)?;
            return Ok(skipped(STALE_RUN, &state, now_ms));
        };
        if !prompted {
            return Ok(skipped("awaiting_choice", &state, now_ms));
        }
        return Ok(json!({
            "outcome": "needs_choice",
            "reason": reason.as_str(),
            "sections": outcomes,
            "status": status_value(&state, now_ms),
        }));
    }
    // 词库在退避里没有跑，也不算同步成功：上次的失败原因留着，状态行照旧说明词库为什么停下。
    progress.synced = !failed && !dictionary_stalled;
    progress.consumed_choice = lease.choice.is_some();
    progress.kept_choice = round_dictionary_choice
        .filter(|_| keep_choice)
        .map(|choice| if keep_as_merge { Choice::Merge } else { choice });
    if writes.is_empty() && deferred.is_none() {
        let result = store
            .update(|state| {
                if !state.lease_matches(run_id, user) {
                    return None;
                }
                state.finish_round(&progress, now_ms);
                Some(state.clone())
            })
            .map_err(storage)?;
        let Some(state) = result else {
            let state = store.read().map_err(storage)?;
            return Ok(skipped(STALE_RUN, &state, now_ms));
        };
        return Ok(json!({
            "outcome": "done",
            "sections": outcomes,
            "status": status_value(&state, now_ms),
        }));
    }
    let (settings_document, settings_revision) = deferred.unzip();
    let pending = PendingCommit {
        run_id: run_id.to_owned(),
        progress,
        settings_document,
        settings_revision,
        modes,
    };
    let result = store
        .update_with_pending(|state, slot| {
            if !state.lease_matches(run_id, user) {
                return Err(());
            }
            *slot = Some(pending);
            Ok(state.clone())
        })
        .map_err(storage)?;
    let Ok(state) = result else {
        let state = store.read().map_err(storage)?;
        return Ok(skipped(STALE_RUN, &state, now_ms));
    };
    Ok(json!({
        "outcome": "pending_commit",
        "run_id": run_id,
        "host_writes": writes,
        "sections": outcomes,
        "status": status_value(&state, now_ms),
    }))
}

fn failure_record(section: Section, slot: &Slot, outcome: &SectionOutcome) -> SyncFailure {
    let reason = outcome
        .reason
        .clone()
        .unwrap_or_else(|| "failed".to_owned());
    match &slot.failure {
        Some(failure) => SyncFailure {
            section,
            reason,
            status: failure.status,
            code: failure.server_code.clone(),
            message: policy::failure_message(failure)
                .unwrap_or_else(|| policy::NETWORK_MESSAGE.to_owned()),
        },
        None => SyncFailure {
            section,
            message: policy::local_message(&reason).to_owned(),
            reason,
            status: None,
            code: None,
        },
    }
}

/// `commit`：宿主写完 `host_writes` 以后提交这一轮。iOS 还要带上实际保存下来的键盘设置，按它把设置文档应用到共享偏好；那一步失败（`conflict`、`storage`）时放弃这一轮，宿主把键盘设置恢复成旧值。
pub(crate) fn commit<A: AccountApi, S: AccountSessionStorage, L: SyncLocal>(
    session: &BackendAccountSession<A, S>,
    store: &SyncStateStore,
    local: &L,
    run_id: &str,
    saved_native: Option<&IosKeyboardPreferences>,
    now_ms: u64,
) -> Result<Value, String> {
    let user = identity(session)
        .map_err(|error| error.code().to_owned())?
        .map(|(user, _)| user)
        .ok_or_else(|| STALE_RUN.to_owned())?;
    let result = store
        .update_always(|state, pending| {
            let Some(current) = pending.clone().filter(|pending| pending.run_id == run_id) else {
                return Err(STALE_RUN.to_owned());
            };
            if !state.lease_matches(run_id, &user) {
                // 这一轮作废：它的待提交进度和租约一起放掉，不用等租约过期。
                state.release(run_id);
                *pending = None;
                return Err(STALE_RUN.to_owned());
            }
            if let Some(document) = &current.settings_document {
                let Some(saved) = saved_native else {
                    return Err(INVALID.to_owned());
                };
                // 没有读本机时的修订号（更早的版本写下的待提交进度）就没法比较并交换：按冲突作废，下一轮重来。
                let applied = current
                    .settings_revision
                    .ok_or_else(|| "conflict".to_owned())
                    .and_then(|revision| local.commit_settings(document, saved, revision));
                if let Err(code) = applied {
                    state.release(run_id);
                    *pending = None;
                    return Err(code);
                }
            }
            state.finish_round(&current.progress, now_ms);
            *pending = None;
            Ok(state.clone())
        })
        .map_err(storage)?;
    result.map(|state| status_value(&state, now_ms))
}

// ---- 本机读写的默认实现 ----

/// 宿主的本机存储：偏好目录（共享偏好和常用语）、Engine 选项（词库）和快照队列目录。
pub(crate) struct HostLocal {
    pub(crate) preferences: PathBuf,
    pub(crate) options: Option<Value>,
    pub(crate) snapshot_queue: Option<PathBuf>,
    pub(crate) inputs: HostInputs,
}

fn preferences_error(error: PreferencesError) -> String {
    match error {
        PreferencesError::Conflict => "conflict",
        _ => "storage",
    }
    .to_owned()
}

impl HostLocal {
    fn store(&self) -> PreferencesStore {
        PreferencesStore::new(&self.preferences)
    }

    fn options(&self) -> Result<&Value, String> {
        self.options
            .as_ref()
            .ok_or_else(|| "dictionary_storage".to_owned())
    }

    fn dictionary_call(&self, action: Value) -> Result<Value, String> {
        let request = json!({ "options": self.options()?, "action": action });
        dictionary_request_json(request.to_string().as_bytes())
            .map_err(|_| "dictionary_storage".to_owned())
    }

    fn queue(&self) -> Result<DictionarySnapshotQueue, String> {
        let directory = self
            .snapshot_queue
            .as_ref()
            .ok_or_else(|| "dictionary_storage".to_owned())?;
        DictionarySnapshotQueue::new(directory.clone()).map_err(|_| "dictionary_storage".to_owned())
    }

    /// Engine 选项里的 `preferences_directory`：个人词库导入队列和命名词库都在它下面。
    fn dictionary_preferences(&self) -> Result<&Path, String> {
        self.options()?
            .get("preferences_directory")
            .and_then(Value::as_str)
            .map(Path::new)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| "dictionary_storage".to_owned())
    }

    /// 个人词库导入队列，在 Engine 选项的 `preferences_directory` 下，与 `msime_client_personal_dictionary_request` 用的是同一份。
    fn personal_store(&self) -> Result<PersonalDictionaryStore, String> {
        Ok(PersonalDictionaryStore::new(
            self.dictionary_preferences()?.join("PersonalDictionary"),
        ))
    }

    /// 命名词库，与 `msime_client_dictionary_collections` 用的是同一份。
    fn collections_store(&self) -> Result<DictionaryCollectionsStore, String> {
        Ok(DictionaryCollectionsStore::new(
            self.dictionary_preferences()?,
            self.personal_store()?,
        ))
    }
}

/// 云端快照里的词还没有全部进本机：有排着等键盘应用的，或者这一轮排不下的，下一轮接着合并。
const MERGE_PENDING: &str = "dictionary_merge_pending";
/// 云端有本机导入不了的词：本机新词的规则不收（例如旧版本留下的带数字的快捷短语编码），或者键盘应用时拒绝了（例如本机方案不读主词库，存不了中文词）。这些词不在本机，上传就会把它们从云端删掉，所以这一类不推进、不上传。
const MERGE_REJECTED: &str = "dictionary_merge_rejected";
/// 云端的词被导入队列里别的失败请求挡住：用户自己的修改没保存成功、占着同一个词或占满了队列，又没有排着的请求能腾出位置，要用户在个人词库里处理。
const MERGE_BLOCKED: &str = "dictionary_merge_blocked";
/// 合并排进导入队列的请求 id 前缀，与 Android 的 `cloud-merge-` 相同。
const MERGE_REQUEST_PREFIX: &str = "cloud-merge-";
/// 键盘应用导入请求时的暂时失败，排一次就能过：`edit_personal_dictionary` 拿不到独占维护权（还有会话开着）或打不开词库目录。
const TRANSIENT_APPLY_FAILURES: [&str; 2] = [
    "dictionary maintenance busy",
    "dictionary access unavailable",
];
/// 新词的权重上限，与 Engine `user_dictionary::personal::MAX_ENTRY_WEIGHT` 相同（那个模块不公开）。
const MAX_IMPORT_WEIGHT: i64 = 100_000_000;

/// 键盘还没发布过本机词库版本，下载的快照没法交给激活队列（新装以后还没打开过键盘就打开了同步）。
const DICTIONARY_UNPUBLISHED: &str = "dictionary_unpublished";
/// 词库在合并的退避里，这一轮没有碰它。
const DICTIONARY_BACKOFF: &str = "dictionary_backoff";

/// 词库这一轮没做完、下一轮还要重来的那几个原因：合并没做完，或者键盘还没发布过本机词库版本。游标不推进，首次同步的选择留给下一轮，词库退避一阵再自动重试。
fn merge_stalled(reason: &str) -> bool {
    matches!(
        reason,
        MERGE_PENDING | MERGE_REJECTED | MERGE_BLOCKED | DICTIONARY_UNPUBLISHED
    )
}

/// 快照里的一个词在本机的身份，本机导出的词和云端的词都这样算，两边才对得上；种类不认识时为 `None`。
fn stored_identity(word: &policy::Word) -> Option<String> {
    let kind = policy::personal_kind(&word.kind)?;
    crate::dictionary::personal_stored_identity(kind, &word.key, &word.value)
}

/// 云端的一个词换成导入请求。权重收进新词的范围 1..=[`MAX_IMPORT_WEIGHT`]：英文学习会把词的权重抬过上限，服务端也收 0，这样的词照样导进来，只是权重被收紧；编码和词按新词的规则校验，不合规时为 `None`。
fn import_word(word: &policy::Word) -> Option<PersonalWord> {
    let kind = policy::personal_kind(&word.kind)?;
    crate::dictionary::personal_import_word(
        kind,
        &word.key,
        &word.value,
        word.weight.clamp(1, MAX_IMPORT_WEIGHT),
    )
}

/// 云端皮肤库里的每个设计宿主都已经有了（上传时裁剪、重排过的库就是这样）：不用交给宿主再导入一遍。宿主导入时按 id 和更新时间合并，不会删掉本机多出来的设计。
fn library_contained(library: &str, host: Option<&str>) -> bool {
    let parse = |text: &str| serde_json::from_str::<Vec<Value>>(text).ok();
    match (parse(library), host.and_then(parse)) {
        (Some(cloud), Some(host)) => cloud.iter().all(|item| host.contains(item)),
        _ => false,
    }
}

/// 快照文件里的用户词：`entry` 减去同一 (kind, code, word) 上标成删除的 `overlay`。墓碑按词本身匹配，不按 `data.id`：云词库面板改词时服务端保留词条 id，旧词写成删除、新词写成生效的覆盖层，两条覆盖层和改后的词条共用同一个 id（msime-cloud `dictionary_store.go` 的 `dictionaryEdit`），按 id 匹配会把改后的词也当成删掉；覆盖层在服务端本来就以 (user, kind, code, word) 为唯一键，恢复快照时也按这个三元组校验词条与覆盖层。Android 的 `SyncApi.snapshotWords` 按 id 匹配，有同样的问题，这里以 Rust 为准。
fn snapshot_words(file: &Path) -> Result<Vec<policy::Word>, String> {
    use std::io::BufRead;
    let reader = std::io::BufReader::new(
        std::fs::File::open(file).map_err(|_| "dictionary_storage".to_owned())?,
    );
    let mut entries: Vec<policy::Word> = Vec::new();
    let mut deleted: HashSet<(String, String, String)> = HashSet::new();
    for line in reader.lines() {
        let line = line.map_err(|_| "dictionary_invalid".to_owned())?;
        if line.is_empty() {
            continue;
        }
        let record: Value =
            serde_json::from_str(&line).map_err(|_| "dictionary_invalid".to_owned())?;
        let Some(data) = record.get("data") else {
            continue;
        };
        let text = |key: &str| data.get(key).and_then(Value::as_str).map(str::to_owned);
        let (Some(kind), Some(key), Some(value)) = (text("kind"), text("code"), text("word"))
        else {
            continue;
        };
        match record.get("type").and_then(Value::as_str) {
            Some("overlay") => {
                if record.get("deleted") == Some(&Value::Bool(true)) {
                    deleted.insert((kind, key, value));
                }
            }
            Some("entry") => entries.push(policy::Word {
                kind,
                key,
                value,
                weight: data.get("weight").and_then(Value::as_i64).unwrap_or(1),
            }),
            _ => {}
        }
    }
    Ok(entries
        .into_iter()
        .filter(|word| {
            !deleted.contains(&(word.kind.clone(), word.key.clone(), word.value.clone()))
        })
        .collect())
}

impl SyncLocal for HostLocal {
    fn export_settings(&self) -> Result<LocalSettings, String> {
        let store = self.store();
        let snapshot = store.load().map_err(|_| "settings_storage".to_owned())?;
        let mut settings = match &self.inputs.ios_native {
            Some(native) => local_account_preferences(
                native,
                &snapshot.preferences,
                snapshot.preferences.custom_theme.keyboard.as_ref(),
            ),
            None if self.inputs.harmony => {
                export_harmony_settings(&snapshot.preferences, self.inputs.feedback.as_ref())
            }
            None => export_android_settings(&snapshot.preferences, self.inputs.feedback.as_ref())
                .map(|mut settings| {
                    if let Some(local) = &self.inputs.android_local {
                        insert_android_local_settings(&mut settings, local);
                    }
                    settings
                }),
        }
        .map_err(|_| "settings_storage".to_owned())?;
        filter_uploaded_account_settings(Some(store.edition()), &mut settings);
        Ok(LocalSettings {
            settings,
            // 皮肤库的字段只有 Android 有。
            skins: (self.inputs.ios_native.is_none() && !self.inputs.harmony)
                .then(|| self.inputs.custom_keyboard_skins.clone())
                .flatten(),
            revision: snapshot.revision,
        })
    }

    fn apply_settings(
        &self,
        document: &AccountPreferences,
        schema: &AccountPreferenceSchema,
        read_revision: u64,
    ) -> Result<SettingsApplied, String> {
        let store = self.store();
        let mut document = document.clone();
        filter_downloaded_account_settings(Some(store.edition()), &mut document.settings);
        let invalid = |_| "settings_storage".to_owned();
        // 按这一轮读本机时的修订号比较并交换：读本机到现在（中间隔着一次联网）另一个进程里用户改了设置时，不能用基于旧值算出的结果覆盖它。
        let local = store.load().map_err(|_| "settings_storage".to_owned())?;
        if local.revision != read_revision {
            return Err(SETTINGS_CHANGED.to_owned());
        }
        let save = |preferences| match store.save(read_revision, preferences) {
            Ok(_) => Ok(()),
            Err(PreferencesError::Conflict) => Err(SETTINGS_CHANGED.to_owned()),
            Err(_) => Err("settings_storage".to_owned()),
        };
        if let Some(current) = &self.inputs.ios_native {
            let plan = IosPreferencePlan::from_cloud(&document).map_err(invalid)?;
            let requested = plan.requested_native(current).map_err(invalid)?;
            if &requested != current {
                return Ok(SettingsApplied {
                    writes: HostWrites {
                        ios_native: Some(requested),
                        ..HostWrites::default()
                    },
                    deferred: true,
                });
            }
            // 键盘设置不用变：共享偏好现在就应用。
            let mut next = local.preferences.clone();
            plan.apply_shared(current, &mut next).map_err(invalid)?;
            if next != local.preferences {
                save(next)?;
            }
            return Ok(SettingsApplied {
                writes: HostWrites::default(),
                deferred: false,
            });
        }
        if self.inputs.harmony {
            let applied = apply_harmony_settings(
                &local.preferences,
                &document,
                schema,
                self.inputs.feedback.clone(),
            )
            .map_err(invalid)?;
            if applied.preferences != local.preferences {
                save(applied.preferences)?;
            }
            let mut writes = HostWrites::default();
            if applied.feedback.is_some() && applied.feedback != self.inputs.feedback {
                writes.feedback = applied.feedback;
            }
            return Ok(SettingsApplied {
                writes,
                deferred: false,
            });
        }
        let applied = apply_android_settings(
            &local.preferences,
            &document,
            schema,
            self.inputs.feedback.clone(),
        )
        .map_err(invalid)?;
        if applied.preferences != local.preferences {
            save(applied.preferences)?;
        }
        let mut writes = HostWrites::default();
        if applied.feedback.is_some() && applied.feedback != self.inputs.feedback {
            writes.feedback = applied.feedback;
        }
        if let Some(Ok(library)) = custom_keyboard_skins(&document, schema).map_err(invalid)? {
            if !library_contained(&library, self.inputs.custom_keyboard_skins.as_deref()) {
                writes.custom_keyboard_skins = Some(library);
            }
        }
        let (android_local, _) = android_local_settings(&document, schema).map_err(invalid)?;
        let current = self.inputs.android_local.clone().unwrap_or_default();
        if android_local
            .iter()
            .any(|(key, value)| current.get(key) != Some(value))
        {
            writes.android_local = Some(android_local);
        }
        Ok(SettingsApplied {
            writes,
            deferred: false,
        })
    }

    fn commit_settings(
        &self,
        document: &AccountPreferences,
        saved_native: &IosKeyboardPreferences,
        read_revision: u64,
    ) -> Result<(), String> {
        let store = self.store();
        let mut document = document.clone();
        filter_downloaded_account_settings(Some(store.edition()), &mut document.settings);
        let plan = IosPreferencePlan::from_cloud(&document).map_err(|_| "storage".to_owned())?;
        let local = store.load().map_err(|_| "storage".to_owned())?;
        // 与 `apply_settings` 一样按读本机时的修订号比较并交换：这份文档是按那时的本机值合并出来的，期间用户在别处改的设置不能被它盖回去。
        if local.revision != read_revision {
            return Err(preferences_error(PreferencesError::Conflict));
        }
        let mut next = local.preferences.clone();
        plan.apply_shared(saved_native, &mut next)
            .map_err(|_| "storage".to_owned())?;
        store
            .save(read_revision, next)
            .map(|_| ())
            .map_err(preferences_error)
    }

    fn phrases(&self) -> Result<Vec<LocalPhrase>, String> {
        let document = CommonPhrasesStore::new(&self.preferences)
            .load()
            .map_err(|_| "phrases_storage".to_owned())?;
        Ok(document
            .phrases
            .into_iter()
            .filter(|phrase| phrase.pack.is_none())
            .map(|phrase| LocalPhrase {
                id: phrase.id.to_string(),
                text: phrase.text,
            })
            .collect())
    }

    fn apply_phrases(
        &self,
        local: &[LocalPhrase],
        target: &[AccountPhrase],
    ) -> Result<BTreeSet<String>, String> {
        let plan = policy::local_plan(local, target);
        let store = CommonPhrasesStore::new(&self.preferences);
        // 删不掉的跳过，下一轮再对齐，与两端原来的做法相同。
        for id in &plan.remove {
            if let Ok(id) = msime_client_core::uuid::Uuid::parse_str(id) {
                let _ = store.remove(id);
            }
        }
        let mut unheld = BTreeSet::new();
        for text in &plan.add {
            if !valid_phrase_text(text) {
                unheld.insert(policy::text_digest(text));
                continue;
            }
            match store.add(text) {
                Ok(_) | Err(CommonPhrasesError::Duplicate) => {}
                Err(_) => {
                    unheld.insert(policy::text_digest(text));
                }
            }
        }
        Ok(unheld)
    }

    fn dictionary(&self, awaited: Option<&str>) -> Result<DictionaryLocal, String> {
        let count = self.dictionary_call(json!({ "operation": "count", "user_only": true }))?;
        // 命名词库的启用、停用和 Android 本地备份恢复的大批词先进命名词库的待发送队列，由键盘空闲时一批批送进个人词库导入队列（那边同时只收 128 个请求）。还没送进去的词不在本机导出的快照里：这时上传会把它们从云端漏掉，之后云端一有改动，下载下来的快照还会整份替换掉随后才送进去的词。所以两个队列都算排着的改动，与 Android 原来的 Java 驱动相同（#8201 迁移计划 5d）。
        let pending = self
            .personal_store()?
            .read()
            .map_err(|_| "dictionary_storage".to_owned())?
            .pending_count()
            + self
                .collections_store()?
                .load()
                .map_err(|_| "dictionary_storage".to_owned())?
                .outbox_pending;
        // 没有激活队列目录的宿主不会下载快照，也就没有等激活的。
        let activation = match self.snapshot_queue {
            None => SnapshotActivation::Idle,
            Some(_) => {
                let queue = self
                    .queue()?
                    .read()
                    .map_err(|_| "dictionary_storage".to_owned())?;
                let request = queue.request.as_ref();
                if request.is_some_and(|request| request.status.active()) {
                    SnapshotActivation::Waiting
                } else {
                    match awaited {
                        None => SnapshotActivation::Idle,
                        // 激活成功时队列把本机版本换成以这个请求 id 为代次的版本，之后键盘发布的版本也沿用它；宿主读过终态（`acknowledge`）以后请求本身不在了，只能看这一条。
                        Some(id)
                            if request.is_some_and(|request| {
                                request.id.to_string() == id
                                    && request.status == SnapshotRequestStatus::Applied
                            }) || queue
                                .local_version
                                .as_deref()
                                .is_some_and(|version| version.split(':').nth(1) == Some(id)) =>
                        {
                            SnapshotActivation::Idle
                        }
                        Some(_) => SnapshotActivation::Lost,
                    }
                }
            }
        };
        Ok(DictionaryLocal {
            has_words: count.get("count").and_then(Value::as_u64).unwrap_or(0) > 0,
            pending,
            activation,
        })
    }

    fn export_dictionary(&self, destination: &Path) -> Result<(), String> {
        let _ = std::fs::remove_file(destination);
        self.dictionary_call(json!({
            "operation": "export_snapshot",
            "destination": destination,
        }))
        .map(|_| ())
    }

    fn replace_dictionary(
        &self,
        snapshot: &Path,
        account_id: &str,
        revision: i64,
    ) -> Result<String, String> {
        let queue = self.queue()?;
        // 键盘还没发布过本机词库版本时没法激活，留到下一轮。
        let local_version = queue
            .read()
            .map_err(|_| "dictionary_storage".to_owned())?
            .local_version
            .ok_or_else(|| DICTIONARY_UNPUBLISHED.to_owned())?;
        let sha256 = inspect_snapshot_json(snapshot)
            .ok()
            .and_then(|metadata| {
                metadata
                    .get("fileSha256")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .ok_or_else(|| "dictionary_invalid".to_owned())?;
        // 云端快照只有用户自己的词，激活时保留本机学到的东西（#10043）。
        queue
            .enqueue(
                snapshot,
                account_id,
                revision,
                &local_version,
                &sha256,
                true,
            )
            .map(|id| id.to_string())
            .map_err(|_| "dictionary_storage".to_owned())
    }

    /// 把云端快照的词并进本机：本机还没有的词排进个人词库导入队列，等键盘应用。只有云端的每个词都已经在本机（或者本机排着删除、改掉它）时才算合并完，返回 `Ok`；否则这一类以 [`MERGE_PENDING`]、[`MERGE_REJECTED`] 或 [`MERGE_BLOCKED`] 失败，游标不推进、不标待上传，下一轮云端相对游标仍然有改动，走的还是合并，重新下载、接着排。合并完以后才会整份上传，所以没进本机的云端词不会被本机快照从云端删掉。
    fn merge_dictionary(&self, snapshot: &Path) -> Result<(), MergeStop> {
        let storage = |_| "dictionary_storage".to_owned();
        let cloud = snapshot_words(snapshot)?;
        // 本机已经有的词不再排队：队列一次最多容纳 `PERSONAL_IMPORT_BATCH` 个没应用完的请求，云端的词多于这个数时要分几轮排进去，每一轮都得跳过前几轮已经被键盘应用的词，才能往后推进。
        let local_file = snapshot.with_file_name("merge-local.ndjson");
        let local = self
            .export_dictionary(&local_file)
            .and_then(|()| snapshot_words(&local_file));
        let _ = std::fs::remove_file(&local_file);
        let held: HashSet<String> = local?.iter().filter_map(stored_identity).collect();
        let store = self.personal_store()?;
        let state = store.read().map_err(storage)?;
        let waiting: Vec<&PersonalWordRequest> = state
            .requests
            .iter()
            .filter(|request| request.status != PersonalWordRequestStatus::Applied)
            .collect();
        // 本机排着删除或改掉的词按本机的改动算。
        let edited: HashSet<String> = waiting
            .iter()
            .filter_map(|request| request.previous.as_ref())
            .map(PersonalWord::identity)
            .collect();
        let queued: HashMap<String, &PersonalWordRequest> = waiting
            .iter()
            .filter_map(|request| {
                request
                    .replacement
                    .as_ref()
                    .map(|word| (word.identity(), *request))
            })
            .collect();
        // 云端快照里已经在本机的词：比上一轮多就是键盘应用了排着的词。
        let present = cloud
            .iter()
            .filter_map(stored_identity)
            .filter(|identity| held.contains(identity))
            .collect::<HashSet<_>>()
            .len();
        let mut seen = HashSet::new();
        let mut words: Vec<PersonalWord> = Vec::new();
        let mut retry = Vec::new();
        let (mut unfinished, mut rejected, mut blocked) = (false, false, false);
        let mut enqueued = false;
        for word in &cloud {
            let Some(identity) = stored_identity(word) else {
                rejected = true;
                continue;
            };
            if held.contains(&identity) || edited.contains(&identity) || !seen.insert(identity) {
                continue;
            }
            let Some(import) = import_word(word) else {
                rejected = true;
                continue;
            };
            match queued.get(&import.identity()) {
                // 排着的会由键盘写进去，写进去以后下一轮就在本机了。
                Some(request) if request.status == PersonalWordRequestStatus::Pending => {
                    unfinished = true;
                }
                // 用户自己的修改没保存成功、占着同一个词：队列不收这个词，要用户处理。
                Some(request) if !request.id.starts_with(MERGE_REQUEST_PREFIX) => blocked = true,
                // 这一类之前排的请求被键盘拒了：暂时的失败重排一次，别的再排还会被拒。
                Some(request)
                    if request
                        .error
                        .as_deref()
                        .is_some_and(|error| TRANSIENT_APPLY_FAILURES.contains(&error)) =>
                {
                    retry.push(request.id.clone());
                }
                Some(_) => rejected = true,
                None => words.push(import),
            }
        }
        // 重排不算排进了新的词：键盘一直拿不到词库时每一轮都会重排同一批，算成进展的话退避每轮都从 5 分钟重新数。
        for id in &retry {
            match store.retry(id) {
                // 冲突说明同一个词已经有排着的请求。
                Ok(()) | Err(PersonalDictionaryError::Conflict) => unfinished = true,
                Err(_) => return Err("dictionary_storage".to_owned().into()),
            }
        }
        let room = policy::PERSONAL_IMPORT_BATCH.saturating_sub(waiting.len());
        let left_over = words.len() > room;
        words.truncate(room);
        if !words.is_empty() {
            unfinished = true;
            let id = format!(
                "{MERGE_REQUEST_PREFIX}{}",
                msime_client_core::uuid::Uuid::new_v4()
            );
            match store.enqueue_import(words, id) {
                Ok(()) => enqueued = true,
                // 排队和读状态之间别处也排了词：这一轮先停下，下一轮接着排。
                Err(
                    PersonalDictionaryError::TooManyRequests | PersonalDictionaryError::Conflict,
                ) => {}
                Err(_) => return Err("dictionary_storage".to_owned().into()),
            }
        }
        // 队列被失败的请求占满，又没有排着的请求会被键盘应用、腾出位置：等下去也排不进去。
        blocked |= left_over && !unfinished && state.pending_count() == 0;
        let stop = |reason: &str| MergeStop {
            reason: reason.to_owned(),
            queued: enqueued,
            present: Some(present),
        };
        if rejected {
            return Err(stop(MERGE_REJECTED));
        }
        if blocked {
            return Err(stop(MERGE_BLOCKED));
        }
        if unfinished || left_over {
            return Err(stop(MERGE_PENDING));
        }
        Ok(())
    }

    fn cancel_dictionary(&self, account_id: &str) -> Result<(), String> {
        if self.snapshot_queue.is_none() {
            return Ok(());
        }
        self.queue()?
            .cancel(account_id)
            .map_err(|_| "dictionary_storage".to_owned())
    }

    fn snapshot_revision(&self, snapshot: &Path) -> Result<i64, String> {
        inspect_snapshot_json(snapshot)
            .ok()
            .and_then(|metadata| metadata.get("cloudRevision").and_then(Value::as_i64))
            .ok_or_else(|| "dictionary_invalid".to_owned())
    }

    fn snapshot_has_words(&self, snapshot: &Path) -> Result<bool, String> {
        snapshot_words(snapshot).map(|words| !words.is_empty())
    }
}

#[cfg(test)]
mod tests;
