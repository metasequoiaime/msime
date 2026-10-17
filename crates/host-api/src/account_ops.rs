//! `msime_client_account` 在 #8201 新增的操作（流式对话是单独的 `msime_client_account_chat_stream`，见 [`chat_stream`]）：对话、注销账号、释义、常用语、我的设备、云端数据、反馈、诊断上传、发送下载链接和语音贡献。请求形状、返回值和错误码见头文件；规则和传输都在 client-services，这里只按操作把它们接起来。
//!
//! 写成对 [`AccountApi`] 和 [`AccountSessionStorage`] 泛型的函数，测试用假的后端驱动；C 导出（`super::ffi::account`）把按目录或注入存储建好的两个会话交进来。
//!
//! 令牌从哪个会话来：注销账号、常用语、我的设备、云端数据和发送下载链接只用登录账号；对话、模型目录、释义、反馈和诊断上传登录账号优先、没有登录时用本机匿名账号；语音贡献和删除语音样本（`voice_delete`）只用本机匿名账号（#8201 的决定 12），注销账号成功后、删除云端数据的 `voice` 分区时也尽力删掉匿名账号名下的样本，云端数据汇总把匿名账号的语音样本合进 `voice` 分区；查看、换令牌和删除诊断快照绑定到上传时返回的 `user_id`，登录会话和匿名会话都不是它时是 `account_identity_changed`，不会退回另一个账号。

use base64::Engine as _;
use msime_client_services::account::page::{ChatModelsResponse, ChatResponse};
use msime_client_services::account::{
    delete_anonymous_voice_samples, has_saved_anonymous_identity, prepare_chat, prepare_feedback,
    prepare_voice_contribution, translate_request_body, valid_data_sections,
    with_shared_access_token, AccountApi, AccountChatMessage, AccountDataSummary, AccountError,
    AccountFailure, AccountFeedbackInput, AccountPhrase, AccountPhrases, AccountSessionStorage,
    AccountTranslateRequest, AccountVoiceContributionInput, AnonymousIdentitySource,
    BackendAccountSession, ChatStreamCancel, DEFAULT_TRANSLATE_TIMEOUT_MS,
    MAX_VOICE_CONTRIBUTION_AUDIO_BYTES, TRANSLATE_TIMEOUT_MS,
};
use msime_client_services::diagnostics::upload::{
    prepare_upload, valid_diagnostic_label, DIAGNOSTIC_TTLS,
};
use msime_client_services::diagnostics::{
    build_bundle, DiagnosticBundleRequest, DiagnosticInclude, DiagnosticSources,
};
use msime_client_services::typing_statistics::upload::{UploadError, UPLOAD_PLATFORMS};
use msime_client_services::typing_statistics::TypingStatisticsStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// 宿主给的路径的字节上限，与 `msime_client_account` 的目录相同。
const MAX_PATH_BYTES: usize = 16 * 1024;
/// 诊断快照绑定的账号 id 的字节上限。
const MAX_USER_ID_BYTES: usize = 128;
/// 语音贡献的音频（最多 2 MiB）编成标准 base64 后的长度上限。
const MAX_VOICE_AUDIO_BASE64_BYTES: usize = MAX_VOICE_CONTRIBUTION_AUDIO_BYTES.div_ceil(3) * 4;

/// 新增的操作。与已有的操作同在 `action` 里，按 `operation` 区分。
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Operation {
    Chat {
        messages: Vec<AccountChatMessage>,
        model: String,
    },
    ChatModels,
    /// 注销账号，与已有的 `delete` 相同，失败时是 [`AccountFailure`]：需要最近登录时是 `account_recent_login_required`。成功后尽力删掉本机匿名账号名下的语音样本，结果见 [`voice_samples`]。
    AccountDelete,
    Translate {
        texts: Vec<String>,
        source_lang: String,
        target_lang: String,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    PhrasesLoad,
    PhrasesSave {
        revision: i64,
        phrases: Vec<AccountPhrase>,
    },
    SessionsList,
    SessionRevoke {
        id: String,
    },
    DataSummary,
    DataExport {
        destination: String,
    },
    DataDelete {
        sections: Vec<String>,
    },
    Feedback {
        #[serde(rename = "type")]
        kind: String,
        text: String,
        platform: String,
        app_version: String,
        edition: String,
        #[serde(default)]
        diagnostics: Option<BTreeMap<String, String>>,
        #[serde(default)]
        screenshots: Option<Vec<String>>,
    },
    DiagnosticsUpload {
        platform: String,
        app_version: String,
        ttl: String,
        bundle: UploadBundle,
    },
    DiagnosticsState {
        user_id: String,
    },
    DiagnosticsRegenerateToken {
        user_id: String,
    },
    DiagnosticsDelete {
        user_id: String,
    },
    DownloadLink {
        platform: String,
    },
    /// 删掉本机匿名账号名下的语音样本，不要求登录（#8201 的决定 12 后续 3）：给从没登录过的用户，以及注销账号、删除云端数据时这一步没做成、要单独重试的宿主。
    VoiceDelete,
    VoiceContribution {
        language: String,
        provider: String,
        transcript: String,
        app_version: String,
        /// WAV 文件的标准 base64。
        audio: String,
    },
    /// 打字统计「上传到云端」的本机状态，加上本机是否登录了账号。`statistics_directory` 是打字统计目录（`msime_client_typing_statistics` 的 `directory`）。
    TypingUploadStatus {
        statistics_directory: String,
    },
    /// 打开或关闭「上传到云端」，与共享设置应用的 `set_typing_statistics_upload` 相同。`day` 是宿主的本地日期，`platform` 是服务端认的平台 id。
    TypingUploadSet {
        statistics_directory: String,
        enabled: bool,
        day: String,
        platform: String,
    },
    /// 打开了上传、已登录、离上次超过十分钟时上传最近几天，与 `upload_typing_statistics` 相同。
    TypingUpload {
        statistics_directory: String,
        day: String,
        platform: String,
        #[serde(default)]
        force: bool,
    },
    /// 「绑定 QQ 群助手」：本人绑定的 QQ 群友。
    QqLinks,
    /// 用机器人给的 8 位绑定码绑定一个 QQ 群友。
    QqLinkRedeem {
        code: String,
    },
    /// 解除一条绑定。
    QqLinkDelete {
        id: String,
    },
}

/// 串行化本进程里的打字统计上传和删除：关掉上传后的 `DELETE` 不能和还在路上的 `PUT` 交错，否则服务端可能先删、后收到旧的上传。每次调用都新建统计目录的 store，它自带的锁管不到别的调用，所以这里用一把进程级的锁。
static TYPING_UPLOAD: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn typing_upload_lock() -> std::sync::MutexGuard<'static, ()> {
    TYPING_UPLOAD
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 打字统计目录读写失败时的码。
fn statistics_storage<E>(_: E) -> OperationFailure {
    AccountError::Storage.into()
}

/// `typing_upload_*` 的回答：`{enabled, last_upload_ms, signed_in}`，`last_upload_ms` 还没有上传过时是 null。
fn typing_upload_state(
    store: &TypingStatisticsStore,
    signed_in: bool,
) -> Result<Value, OperationFailure> {
    let state = store.upload_state().map_err(statistics_storage)?;
    Ok(json!({
        "enabled": state.enabled,
        "last_upload_ms": (state.last_upload_ms > 0).then_some(state.last_upload_ms),
        "signed_in": signed_in,
    }))
}

/// 打开了上传、已登录时上传最近几天，`force` 时不看十分钟的间隔；没有登录时不上传、不报错，开关保持原样，登录后下一次照常上传。
fn typing_upload<A, S>(
    account: &BackendAccountSession<A, S>,
    store: &TypingStatisticsStore,
    day: &str,
    platform: &str,
    now_ms: u64,
    force: bool,
) -> Result<Value, OperationFailure>
where
    A: AccountApi,
    S: AccountSessionStorage,
{
    let signed_in = account.status()?.is_some();
    if signed_in {
        match store.upload_if_due(day, platform, now_ms, force, |payload| {
            account.put_typing_statistics(payload)
        }) {
            Ok(_) => {}
            Err(UploadError::Send(failure)) => return Err(failure.into()),
            Err(UploadError::Storage(error)) => return Err(statistics_storage(error)),
        }
    }
    typing_upload_state(store, signed_in)
}

fn statistics_store(directory: &str) -> Result<TypingStatisticsStore, OperationFailure> {
    Ok(TypingStatisticsStore::new(absolute_path(directory)?))
}

/// 本地日期：`YYYY-MM-DD`，只看形状，日期本身由统计模块解析。
fn valid_day(day: &str) -> bool {
    day.len() == 10
        && day.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

/// 现在的 Unix 毫秒。
fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

/// 诊断上传读哪些本机记录，与 `msime_client_diagnostic_bundle` 的请求相同，只是没有 `destination`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UploadBundle {
    state_root: String,
    #[serde(default)]
    include: DiagnosticInclude,
    #[serde(default)]
    sources: DiagnosticSources,
}

/// 一次新操作的失败：错误码，以及服务端给了 `Retry-After` 时的秒数。
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OperationFailure {
    pub(crate) code: &'static str,
    pub(crate) retry_after_seconds: Option<u64>,
}

impl From<AccountFailure> for OperationFailure {
    fn from(failure: AccountFailure) -> Self {
        Self {
            code: failure.code(),
            retry_after_seconds: failure.retry_after_seconds,
        }
    }
}

impl From<AccountError> for OperationFailure {
    fn from(error: AccountError) -> Self {
        AccountFailure::from(error).into()
    }
}

fn invalid() -> OperationFailure {
    AccountError::Invalid.into()
}

fn value<T: Serialize>(value: T) -> Result<Value, OperationFailure> {
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable.into())
}

fn absolute_path(value: &str) -> Result<&Path, OperationFailure> {
    let path = Path::new(value);
    if value.len() > MAX_PATH_BYTES || !path.is_absolute() {
        return Err(invalid());
    }
    Ok(path)
}

fn bound_user(user_id: &str) -> Result<&str, OperationFailure> {
    if user_id.is_empty() || !msime_client_core::is_bounded_text(user_id, MAX_USER_ID_BYTES) {
        return Err(invalid());
    }
    Ok(user_id)
}

/// 登录账号优先、没有登录时用匿名账号做一次请求（与释义相同的 [`with_shared_access_token`]），回来以后核对给出令牌的那个会话仍是同一个账号：期间退出或换了号是 `account_cancelled`，回答不交给已经换了账号的页面。
fn preferred_account_request<A, S1, S2, T>(
    account: &BackendAccountSession<A, S1>,
    anonymous: &BackendAccountSession<A, S2>,
    operation: impl Fn(&A, &str) -> Result<T, AccountFailure>,
) -> Result<T, OperationFailure>
where
    A: AccountApi,
    S1: AccountSessionStorage,
    S2: AccountSessionStorage,
{
    let (result, token) = with_shared_access_token(account, anonymous, None, operation)?;
    let current = if token.anonymous {
        anonymous.status()?
    } else {
        account.status()?
    };
    if current.is_none_or(|user| user.id != token.user_id) {
        return Err(AccountError::Cancelled.into());
    }
    Ok(result)
}

/// 注销账号成功或删除云端数据的登录账号分区以后，尽力删掉本机匿名账号名下的语音样本（[`delete_anonymous_voice_samples`]），返回放进响应 `voice_samples` 字段的结果：成功（包括本机从来没有匿名身份、没有样本可删）是 `{"ok":true}`，失败是 `{"ok":false,"error":码,"retry_after_seconds"?:秒}`，形状与 C 响应的信封相同。这一步失败不撤销、也不让前面已经做成的删除失败：宿主据此提示「语音样本没有删掉」，用 `voice_delete` 单独重试，不用把登录账号的分区再删一遍。
pub(crate) fn voice_samples<A, S>(
    anonymous: impl FnOnce() -> Result<Arc<BackendAccountSession<A, S>>, AccountError>,
    identity: AnonymousIdentitySource<'_>,
) -> Value
where
    A: AccountApi,
    S: AccountSessionStorage,
{
    let deleted = anonymous()
        .map_err(AccountFailure::from)
        .and_then(|anonymous| delete_anonymous_voice_samples(&anonymous, identity));
    match deleted.map_err(OperationFailure::from) {
        Ok(()) => json!({ "ok": true }),
        Err(OperationFailure {
            code,
            retry_after_seconds: Some(seconds),
        }) => json!({ "ok": false, "error": code, "retry_after_seconds": seconds }),
        Err(OperationFailure { code, .. }) => json!({ "ok": false, "error": code }),
    }
}

/// 注销账号（已有的 `delete` 和新的 `account_delete`）成功以后的 value：`{"voice_samples": 结果}`，见 [`voice_samples`]。
pub(crate) fn account_deleted<A, S>(
    anonymous: impl FnOnce() -> Result<Arc<BackendAccountSession<A, S>>, AccountError>,
    identity: AnonymousIdentitySource<'_>,
) -> Value
where
    A: AccountApi,
    S: AccountSessionStorage,
{
    json!({ "voice_samples": voice_samples(anonymous, identity) })
}

/// 把匿名账号汇总里的 `voice` 分区加进登录账号的汇总：同名分区的字节数和条数相加，登录账号的汇总里没有这个分区时整个加进去，总字节数跟着加。
fn merge_anonymous_voice(summary: &mut AccountDataSummary, anonymous: &AccountDataSummary) {
    let Some(voice) = anonymous
        .sections
        .iter()
        .find(|section| section.id == "voice")
    else {
        return;
    };
    summary.bytes = summary.bytes.saturating_add(voice.bytes);
    match summary
        .sections
        .iter_mut()
        .find(|section| section.id == "voice")
    {
        Some(own) => {
            own.bytes = own.bytes.saturating_add(voice.bytes);
            own.items = own.items.saturating_add(voice.items);
        }
        None => summary.sections.push(voice.clone()),
    }
}

/// 云端数据汇总（#8201 的决定 12 后续 4）：登录账号的汇总，加上本机匿名账号的语音样本（语音贡献只用匿名账号）。匿名这一次另发一个 `GET /v1/users/me/data`，它失败不让汇总失败；`anonymous_voice` 说明合进来没有：`included` 是合进了 `voice` 分区，`none` 是本机既没有匿名会话也没有匿名身份、没有可合的，`unknown` 是这一次没有成功（读不了匿名会话或身份、请求失败，或者身份还在而会话已经过期清掉，这时名下可能还有样本，汇总不为此重新登录），`voice` 只是登录账号的。
fn data_summary<A, S1, S2>(
    account: &BackendAccountSession<A, S1>,
    anonymous: impl FnOnce() -> Result<Arc<BackendAccountSession<A, S2>>, AccountError>,
    identity: AnonymousIdentitySource<'_>,
) -> Result<Value, OperationFailure>
where
    A: AccountApi,
    S1: AccountSessionStorage,
    S2: AccountSessionStorage,
{
    let mut summary = account.data_summary()?;
    let anonymous_voice = match anonymous().and_then(|anonymous| {
        let signed_in = anonymous.status()?.is_some();
        Ok((anonymous, signed_in))
    }) {
        Err(_) => "unknown",
        // 没有会话不等于没有样本：身份还在时是会话过期被清掉了，名下的样本还在，只是这次取不到。
        Ok((_, false)) => match has_saved_anonymous_identity(identity) {
            Ok(false) => "none",
            Ok(true) | Err(_) => "unknown",
        },
        Ok((anonymous, true)) => match anonymous.data_summary() {
            Ok(theirs) => {
                merge_anonymous_voice(&mut summary, &theirs);
                "included"
            }
            Err(_) => "unknown",
        },
    };
    let mut value = value(summary)?;
    value["anonymous_voice"] = json!(anonymous_voice);
    Ok(value)
}

/// 流式对话（`msime_client_account_chat_stream`）：请求和预检与 `chat` 相同，登录账号优先、没有登录时用匿名账号；回复的增量在调用线程上交给 `on_delta`，`cancel` 可以从任何线程取消。回答完整以后与 `chat` 一样核对给出令牌的账号没有退出或换号，否则是 `account_cancelled`（这时增量已经交出去了，宿主丢掉它们）。
pub(crate) fn chat_stream<A, S1, S2>(
    account: &BackendAccountSession<A, S1>,
    anonymous: impl FnOnce() -> Result<Arc<BackendAccountSession<A, S2>>, AccountError>,
    messages: &[AccountChatMessage],
    model: &str,
    cancel: &ChatStreamCancel,
    on_delta: &mut dyn FnMut(&str),
) -> Result<Value, OperationFailure>
where
    A: AccountApi,
    S1: AccountSessionStorage,
    S2: AccountSessionStorage,
{
    // 不合法和太大的请求在取令牌之前就结束。
    prepare_chat(messages, model)?;
    let anonymous = anonymous()?;
    // 令牌被拒时 with_shared_access_token 会再调一次请求，所以请求写成 Fn，回调经 RefCell 借出；两次调用不会重叠。
    let on_delta = RefCell::new(on_delta);
    value(ChatResponse {
        content: preferred_account_request(account, &anonymous, |api, token| {
            api.chat_stream_detailed(messages, model, token, cancel, &mut **on_delta.borrow_mut())
        })?,
    })
}

/// 执行一次新操作。`account` 是登录会话；`anonymous` 给出本机匿名账号的会话，只有用得到匿名账号的操作（登录账号优先、退回匿名的，只用匿名账号的语音贡献和删除语音样本，绑定账号的诊断操作，注销账号成功后，云端数据的汇总和删除 `voice` 分区）才会调用它；`identity` 是匿名账号的身份存在哪里，只在删除匿名账号的语音样本、匿名会话要重新登录时读。
pub(crate) fn run<A, S1, S2>(
    account: &BackendAccountSession<A, S1>,
    anonymous: impl FnOnce() -> Result<Arc<BackendAccountSession<A, S2>>, AccountError>,
    identity: AnonymousIdentitySource<'_>,
    operation: Operation,
) -> Result<Value, OperationFailure>
where
    A: AccountApi,
    S1: AccountSessionStorage,
    S2: AccountSessionStorage,
{
    match operation {
        Operation::Chat { messages, model } => {
            // 不合法和太大的请求在取令牌之前就结束。
            prepare_chat(&messages, &model)?;
            let anonymous = anonymous()?;
            value(ChatResponse {
                content: preferred_account_request(account, &anonymous, |api, token| {
                    api.chat_detailed(&messages, &model, token)
                })?,
            })
        }
        Operation::ChatModels => {
            let anonymous = anonymous()?;
            value(ChatModelsResponse::from(preferred_account_request(
                account,
                &anonymous,
                |api, token| api.chat_models_detailed(token),
            )?))
        }
        Operation::AccountDelete => {
            account.delete_account_detailed()?;
            Ok(account_deleted(anonymous, identity))
        }
        Operation::Translate {
            texts,
            source_lang,
            target_lang,
            timeout_ms,
        } => {
            let timeout = timeout_ms.unwrap_or(DEFAULT_TRANSLATE_TIMEOUT_MS);
            if !TRANSLATE_TIMEOUT_MS.contains(&timeout) {
                return Err(invalid());
            }
            let request = AccountTranslateRequest {
                texts,
                source_lang,
                target_lang,
            };
            // 不合法和太大的请求在取令牌之前就结束。
            translate_request_body(&request)?;
            let anonymous = anonymous()?;
            let (translations, _) =
                with_shared_access_token(account, &anonymous, None, |api, token| {
                    api.translate(&request, Duration::from_millis(timeout), token)
                })?;
            value(translations)
        }
        Operation::PhrasesLoad => value(account.phrases()?),
        Operation::PhrasesSave { revision, phrases } => {
            value(account.put_phrases(&AccountPhrases { revision, phrases })?)
        }
        Operation::SessionsList => value(account.devices()?),
        Operation::SessionRevoke { id } => Ok(json!({ "signed_out": account.revoke_device(&id)? })),
        Operation::DataSummary => data_summary(account, anonymous, identity),
        Operation::DataExport { destination } => {
            value(account.data_export(absolute_path(&destination)?)?)
        }
        Operation::DataDelete { sections } => {
            if !valid_data_sections(&sections) {
                return Err(invalid());
            }
            // 先删登录账号的分区（#8201 的决定 12 后续，负责人在 PR #8350 上定的顺序）：登录会话要求最近登录、没有登录或别的原因失败时在这里结束，不碰匿名账号，也就不会白白耗掉一次按地址每天限额的匿名重新登录。
            account.data_delete(&sections)?;
            if sections.iter().any(|section| section == "voice") {
                // 语音贡献只用匿名账号，登录账号的 voice 分区删不到它们，所以接着删匿名账号名下的样本。登录账号的分区已经删掉了，这一步失败不让整个操作失败（否则宿主重试会把词库的 reset 等再做一遍），结果放在 voice_samples 里，宿主用 voice_delete 单独重试。
                return Ok(json!({ "voice_samples": voice_samples(anonymous, identity) }));
            }
            Ok(Value::Null)
        }
        Operation::Feedback {
            kind,
            text,
            platform,
            app_version,
            edition,
            diagnostics,
            screenshots,
        } => {
            let screenshots = screenshots
                .unwrap_or_default()
                .iter()
                .map(|path| absolute_path(path).map(PathBuf::from))
                .collect::<Result<Vec<_>, _>>()?;
            let feedback = prepare_feedback(&AccountFeedbackInput {
                kind,
                text,
                platform,
                app_version,
                edition,
                diagnostics: diagnostics.unwrap_or_default(),
                screenshots,
            })?;
            let anonymous = anonymous()?;
            let (receipt, _) =
                with_shared_access_token(account, &anonymous, None, |api, token| {
                    api.feedback(&feedback, token)
                })?;
            value(receipt)
        }
        Operation::DiagnosticsUpload {
            platform,
            app_version,
            ttl,
            bundle,
        } => {
            let include = bundle.include;
            // 读本机记录之前先查不读文件就能判断的条件。
            if !valid_diagnostic_label(&platform)
                || !valid_diagnostic_label(&app_version)
                || !DIAGNOSTIC_TTLS.contains(&ttl.as_str())
                || !(include.crash_logs
                    || include.performance_logs
                    || include.input_events
                    || include.config_snapshot)
            {
                return Err(invalid());
            }
            absolute_path(&bundle.state_root)?;
            let built = build_bundle(&DiagnosticBundleRequest {
                state_root: bundle.state_root,
                include,
                sources: bundle.sources,
                destination: None,
            })
            .map_err(|error| OperationFailure {
                code: error.code(),
                retry_after_seconds: None,
            })?;
            let upload = prepare_upload(&platform, &app_version, &ttl, built)?;
            let anonymous = anonymous()?;
            let (receipt, token) =
                with_shared_access_token(account, &anonymous, None, |api, token| {
                    api.diagnostics_upload(&upload, token)
                })?;
            Ok(json!({
                "id": receipt.id,
                "mcp_url": receipt.mcp_url,
                "token": receipt.token,
                "expires_at": receipt.expires_at,
                "counts": upload.counts,
                "user_id": token.user_id,
                "anonymous": token.anonymous,
            }))
        }
        Operation::DiagnosticsState { user_id } => {
            let user_id = bound_user(&user_id)?;
            let anonymous = anonymous()?;
            let (state, _) =
                with_shared_access_token(account, &anonymous, Some(user_id), |api, token| {
                    api.diagnostics_state(token)
                })?;
            value(state)
        }
        Operation::DiagnosticsRegenerateToken { user_id } => {
            let user_id = bound_user(&user_id)?;
            let anonymous = anonymous()?;
            let (token, _) =
                with_shared_access_token(account, &anonymous, Some(user_id), |api, token| {
                    api.diagnostics_regenerate_token(token)
                })?;
            Ok(json!({ "token": token }))
        }
        Operation::DiagnosticsDelete { user_id } => {
            let user_id = bound_user(&user_id)?;
            let anonymous = anonymous()?;
            with_shared_access_token(account, &anonymous, Some(user_id), |api, token| {
                api.diagnostics_delete(token)
            })?;
            Ok(Value::Null)
        }
        Operation::DownloadLink { platform } => value(account.download_link(&platform)?),
        Operation::VoiceDelete => {
            // 不要求登录：没有匿名会话也没有身份时没有样本可删，直接成功；匿名会话要重新登录时用保存的身份（见 delete_anonymous_voice_samples）。
            delete_anonymous_voice_samples(&*anonymous()?, identity)?;
            Ok(Value::Null)
        }
        Operation::VoiceContribution {
            language,
            provider,
            transcript,
            app_version,
            audio,
        } => {
            // 解码之前按长度挡掉一定超过 2 MiB 的音频。
            if audio.len() > MAX_VOICE_AUDIO_BASE64_BYTES {
                return Err(AccountFailure::too_large().into());
            }
            let audio = base64::engine::general_purpose::STANDARD
                .decode(audio)
                .map_err(|_| invalid())?;
            let contribution = prepare_voice_contribution(&AccountVoiceContributionInput {
                language,
                provider,
                transcript,
                app_version,
                audio,
            })?;
            // 只用本机匿名账号（#8201 的决定 12），登录了也一样，样本不挂在实名账号下；没有匿名会话时是 account_unauthorized。
            let receipt = anonymous()?.voice_contribution(&contribution)?;
            Ok(json!({ "id": receipt.id, "duration_ms": contribution.duration_ms() }))
        }
        Operation::TypingUploadStatus {
            statistics_directory,
        } => {
            let store = statistics_store(&statistics_directory)?;
            typing_upload_state(&store, account.status()?.is_some())
        }
        Operation::TypingUploadSet {
            statistics_directory,
            enabled,
            day,
            platform,
        } => {
            let store = statistics_store(&statistics_directory)?;
            if !valid_day(&day) || !UPLOAD_PLATFORMS.contains(&platform.as_str()) {
                return Err(invalid());
            }
            let _operation = typing_upload_lock();
            if enabled {
                // 打开要求已登录，并立刻上传一次；上传失败时开关保持打开，错误交给宿主，下一次自动上传再试。
                if account.status()?.is_none() {
                    return Err(AccountError::Unauthorized.into());
                }
                store.set_upload_enabled(true).map_err(statistics_storage)?;
                typing_upload(account, &store, &day, &platform, unix_ms(), true)
            } else {
                // 先在本地关掉，再删云端副本；删除失败时开关仍是关的，错误交给宿主。
                store
                    .set_upload_enabled(false)
                    .map_err(statistics_storage)?;
                account.delete_typing_statistics()?;
                typing_upload_state(&store, account.status()?.is_some())
            }
        }
        Operation::TypingUpload {
            statistics_directory,
            day,
            platform,
            force,
        } => {
            let store = statistics_store(&statistics_directory)?;
            if !valid_day(&day) || !UPLOAD_PLATFORMS.contains(&platform.as_str()) {
                return Err(invalid());
            }
            let _operation = typing_upload_lock();
            typing_upload(account, &store, &day, &platform, unix_ms(), force)
        }
        Operation::QqLinks => value(account.qq_links()?),
        Operation::QqLinkRedeem { code } => value(account.redeem_qq_link(&code)?),
        Operation::QqLinkDelete { id } => {
            account.delete_qq_link(&id)?;
            Ok(Value::Null)
        }
    }
}

#[cfg(test)]
mod tests;
