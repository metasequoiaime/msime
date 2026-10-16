//! `msime_client_account` 在 #8201 新增的操作：对话、释义、常用语、我的设备、云端数据、反馈、诊断上传、发送下载链接和语音贡献。请求形状、返回值和错误码见头文件；规则和传输都在 client-services，这里只按操作把它们接起来。
//!
//! 写成对 [`AccountApi`] 和 [`AccountSessionStorage`] 泛型的函数，测试用假的后端驱动；C 导出（`super::ffi::account`）把按目录或注入存储建好的两个会话交进来。
//!
//! 令牌从哪个会话来：对话、常用语、我的设备、云端数据和发送下载链接只用登录账号；释义、反馈、诊断上传和语音贡献登录账号优先、没有登录时用本机匿名账号；查看、换令牌和删除诊断快照绑定到上传时返回的 `user_id`，登录会话和匿名会话都不是它时是 `account_identity_changed`，不会退回另一个账号。

use base64::Engine as _;
use msime_client_services::account::page::{ChatModelsResponse, ChatResponse};
use msime_client_services::account::{
    prepare_feedback, prepare_voice_contribution, translate_request_body, with_shared_access_token,
    AccountApi, AccountChatMessage, AccountError, AccountFailure, AccountFeedbackInput,
    AccountPhrase, AccountPhrases, AccountSessionStorage, AccountTranslateRequest,
    AccountVoiceContributionInput, BackendAccountSession, DEFAULT_TRANSLATE_TIMEOUT_MS,
    MAX_VOICE_CONTRIBUTION_AUDIO_BYTES, TRANSLATE_TIMEOUT_MS,
};
use msime_client_services::diagnostics::upload::{
    prepare_upload, valid_diagnostic_label, DIAGNOSTIC_TTLS,
};
use msime_client_services::diagnostics::{
    build_bundle, DiagnosticBundleRequest, DiagnosticInclude, DiagnosticSources,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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
    VoiceContribution {
        language: String,
        provider: String,
        transcript: String,
        app_version: String,
        /// WAV 文件的标准 base64。
        audio: String,
    },
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

/// 执行一次新操作。`account` 是登录会话；`anonymous` 给出本机匿名账号的会话，只有登录账号优先、退回匿名的操作和绑定账号的诊断操作才会用到它。
pub(crate) fn run<A, S1, S2>(
    account: &BackendAccountSession<A, S1>,
    anonymous: impl FnOnce() -> Result<Arc<BackendAccountSession<A, S2>>, AccountError>,
    operation: Operation,
) -> Result<Value, OperationFailure>
where
    A: AccountApi,
    S1: AccountSessionStorage,
    S2: AccountSessionStorage,
{
    match operation {
        Operation::Chat { messages, model } => value(ChatResponse {
            content: account.chat_checked(&messages, &model)?,
        }),
        Operation::ChatModels => value(ChatModelsResponse::from(account.chat_models()?)),
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
        Operation::DataSummary => value(account.data_summary()?),
        Operation::DataExport { destination } => {
            value(account.data_export(absolute_path(&destination)?)?)
        }
        Operation::DataDelete { sections } => {
            account.data_delete(&sections)?;
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
            let anonymous = anonymous()?;
            let (receipt, _) =
                with_shared_access_token(account, &anonymous, None, |api, token| {
                    api.voice_contribution(&contribution, token)
                })?;
            Ok(json!({ "id": receipt.id, "duration_ms": contribution.duration_ms() }))
        }
    }
}

#[cfg(test)]
mod tests;
