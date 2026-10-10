//! 账号云词库请求的分发，iOS、Android 和 Windows 共用一份。
//!
//! 这些操作只经过账号会话：列表、目录、增删改、导入导出、候选调频和固定排位。函数对会话的后端和存储泛型，各宿主把自己 `AccountState` 持有的会话传进来，测试则用合成的后端驱动它。完整快照（下载预览、备份、恢复、应用到本机）要落盘，还要宿主自己的存储和输入法配合，由各宿主分别实现，这里对它们返回 `None`。

use msime_client_core::account::{
    AccountApi, AccountCandidateQuery, AccountSessionStorage, BackendAccountSession,
};
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
use serde_json::Value;
use std::sync::Arc;

use super::account_helpers::{account_value, call_session};

// 只有 iOS 和 Android 的命令用它；Windows 走 lib.rs 里与 macOS、Linux 共用的桌面命令，在那里按桌面端的错误码自己解析。
#[cfg_attr(not(any(target_os = "ios", target_os = "android")), allow(dead_code))]
pub(crate) fn parse_cloud_dictionary_request(
    action: &Value,
) -> Result<CloudDictionaryRequest, crate::CommandError> {
    let request = serde_json::from_value(action.clone()).map_err(|_| crate::CommandError {
        code: "invalid_cloud_dictionary",
    })?;
    msime_host_api::cloud_dictionary::validate_cloud_request(&request).map_err(|_| {
        crate::CommandError {
            code: "invalid_cloud_dictionary",
        }
    })?;
    Ok(request)
}

pub(crate) fn dictionary_kind(value: &str) -> Result<DictionaryKind, crate::CommandError> {
    match value {
        "pinyin" => Ok(DictionaryKind::Pinyin),
        "wubi" => Ok(DictionaryKind::Wubi),
        "wubi98" => Ok(DictionaryKind::Wubi98),
        "quick" => Ok(DictionaryKind::Quick),
        "english" => Ok(DictionaryKind::English),
        _ => Err(crate::CommandError {
            code: "invalid_cloud_dictionary",
        }),
    }
}

/// 用账号会话处理一条云词库请求。快照类请求留给调用方，返回 `None`。
pub(crate) async fn account_request<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    request: &CloudDictionaryRequest,
) -> Option<Result<Value, crate::CommandError>> {
    match request {
        CloudDictionaryRequest::List {
            kind,
            offset,
            search,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            Some(
                call_session(session, {
                    let search = search.clone();
                    let offset = *offset;
                    move |session| {
                        session
                            .dictionary(kind, &search, offset)
                            .and_then(account_value)
                    }
                })
                .await,
            )
        }
        CloudDictionaryRequest::Catalog {
            kind,
            code,
            offset,
            scheme,
            profile,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let code = code.clone();
            let offset = *offset;
            let scheme = scheme.clone();
            let profile = profile.clone();
            Some(
                call_session(session, move |session| {
                    session
                        .dictionary_catalog(kind, &code, offset, &scheme, &profile)
                        .and_then(|page| {
                            account_value(serde_json::json!({
                                "catalog_entries": page.entries,
                                "has_more": page.has_more,
                                "offset": page.offset,
                                "revision": page.revision,
                                "normalized": page.normalized,
                            }))
                        })
                })
                .await,
            )
        }
        CloudDictionaryRequest::Changes { after, limit } => {
            let after = *after;
            let limit = *limit;
            Some(
                call_session(session, move |session| {
                    session
                        .dictionary_changes(after, limit)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Add {
            kind,
            code,
            word,
            weight,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let code = code.clone();
            let word = word.clone();
            let weight = *weight;
            Some(
                call_session(session, move |session| {
                    session
                        .add_dictionary(kind, &code, &word, weight)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Update {
            kind,
            id,
            code,
            word,
            weight,
            revision,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let id = id.clone();
            let code = code.clone();
            let word = word.clone();
            let weight = *weight;
            let revision = *revision;
            Some(
                call_session(session, move |session| {
                    session
                        .update_dictionary(kind, &id, &code, &word, weight, revision)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::EditCatalog {
            kind,
            code,
            word,
            revision,
            replacement,
        } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let code = code.clone();
            let word = word.clone();
            let revision = *revision;
            let replacement = replacement
                .as_ref()
                .map(|value| (value.code.clone(), value.word.clone(), value.weight));
            Some(
                call_session(session, move |session| {
                    let replacement = replacement
                        .as_ref()
                        .map(|(code, word, weight)| (code.as_str(), word.as_str(), *weight));
                    session
                        .edit_dictionary_catalog(kind, &code, &word, revision, replacement)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Candidates {
            text,
            kind,
            scheme,
            profile,
            limit,
        } => {
            let query = AccountCandidateQuery {
                text: text.clone(),
                kind: kind.clone(),
                scheme: scheme.clone(),
                profile: profile.clone(),
                limit: *limit,
            };
            Some(
                call_session(session, move |session| {
                    session.personal_candidates(&query).and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Rank {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
            mode,
            linear_step,
            trigger_count,
            force_top,
        } => {
            let query = AccountCandidateQuery {
                text: text.clone(),
                kind: kind.clone(),
                scheme: scheme.clone(),
                profile: profile.clone(),
                limit: *limit,
            };
            let code = code.clone();
            let word = word.clone();
            let mode = mode.clone();
            let revision = *revision;
            let linear_step = *linear_step;
            let trigger_count = *trigger_count;
            let force_top = *force_top;
            Some(
                call_session(session, move |session| {
                    session
                        .rank_candidate(
                            &query,
                            &code,
                            &word,
                            revision,
                            &mode,
                            linear_step,
                            trigger_count,
                            force_top,
                        )
                        .map(|result| {
                            serde_json::json!({
                                "revision": result.revision,
                                "changed": result.changed,
                                "selection_count": result.selection.count,
                            })
                        })
                })
                .await,
            )
        }
        CloudDictionaryRequest::RemoveCandidate {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
        } => {
            let query = AccountCandidateQuery {
                text: text.clone(),
                kind: kind.clone(),
                scheme: scheme.clone(),
                profile: profile.clone(),
                limit: *limit,
            };
            let code = code.clone();
            let word = word.clone();
            let revision = *revision;
            Some(
                call_session(session, move |session| {
                    session
                        .remove_candidate(&query, &code, &word, revision)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::FixedPositions { context, offset } => {
            let context = context.clone();
            let offset = *offset;
            Some(
                call_session(session, move |session| {
                    session
                        .fixed_positions(&context, offset)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::SetFixedPosition {
            context,
            code,
            word,
            position,
            revision,
        } => {
            let context = context.clone();
            let code = code.clone();
            let word = word.clone();
            let position = *position;
            let revision = *revision;
            Some(
                call_session(session, move |session| {
                    session
                        .set_fixed_position(&context, &code, &word, position, revision)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Delete { kind, id, revision } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let id = id.clone();
            let revision = *revision;
            Some(
                call_session(session, move |session| {
                    session
                        .delete_dictionary(kind, &id, revision)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Import { kind, format, text } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let format = format.clone();
            let text = text.clone();
            Some(
                call_session(session, move |session| {
                    session
                        .import_dictionary(kind, &format, &text)
                        .and_then(account_value)
                })
                .await,
            )
        }
        CloudDictionaryRequest::Export { kind, format } => {
            let kind = match dictionary_kind(kind) {
                Ok(kind) => kind,
                Err(error) => return Some(Err(error)),
            };
            let format = format.clone();
            Some(
                call_session(session, move |session| {
                    session.export_dictionary(kind, &format).map(|result| {
                        serde_json::json!({
                            "text": result.text,
                            "filename": result.filename,
                        })
                    })
                })
                .await,
            )
        }
        CloudDictionaryRequest::SnapshotPreview
        | CloudDictionaryRequest::SnapshotExport
        | CloudDictionaryRequest::SnapshotRestorePreview { .. }
        | CloudDictionaryRequest::SnapshotRestore { .. }
        | CloudDictionaryRequest::SnapshotRestoreNative { .. }
        | CloudDictionaryRequest::SnapshotRestoreCancel
        | CloudDictionaryRequest::SnapshotChooseRestore
        | CloudDictionaryRequest::SnapshotEnqueue { .. }
        | CloudDictionaryRequest::SnapshotStatus
        | CloudDictionaryRequest::SnapshotCancel => None,
    }
}

pub(crate) fn snapshot_command_error() -> crate::CommandError {
    crate::CommandError {
        code: "snapshot_unavailable",
    }
}

/// 快照队列的状态里带着账号 id，交给页面之前去掉。
pub(crate) fn snapshot_response_without_account(
    mut value: Value,
) -> Result<Value, crate::CommandError> {
    let object = value.as_object_mut().ok_or_else(snapshot_command_error)?;
    if let Some(request) = object.get_mut("request").and_then(Value::as_object_mut) {
        request.remove("accountId");
    }
    Ok(value)
}
