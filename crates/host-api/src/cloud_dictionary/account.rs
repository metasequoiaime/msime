//! 只经账号会话的云词库请求：列表、目录、增删改、导入导出、候选调频和固定排位。iOS、Android 和 Windows 的 Tauri 命令与 `msime_client_cloud_dictionary` 共用这一份分发。
//!
//! 函数对会话的后端和存储泛型，各宿主把自己持有的会话传进来，测试则用合成的后端驱动它。完整快照（下载预览、备份、恢复、应用到本机）要落盘，还要宿主的存储和输入法配合，见 [`super::full_backup`]；这里对它们返回 `None`。函数会联网阻塞，调用方在工作线程上调用。

use super::CloudDictionaryRequest;
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_client_services::account::{
    AccountApi, AccountCandidateQuery, AccountError, AccountSessionStorage, BackendAccountSession,
};
use serde_json::Value;

/// 请求里的词库种类。前面的 [`super::validate_cloud_request`] 已经校验过，这里仍按不合法处理未知值。
pub fn dictionary_kind(value: &str) -> Result<DictionaryKind, &'static str> {
    match value {
        "pinyin" => Ok(DictionaryKind::Pinyin),
        "wubi" => Ok(DictionaryKind::Wubi),
        "wubi98" => Ok(DictionaryKind::Wubi98),
        "quick" => Ok(DictionaryKind::Quick),
        "english" => Ok(DictionaryKind::English),
        _ => Err("invalid_cloud_dictionary"),
    }
}

/// 这条请求是否由 [`account_request`] 处理；快照类请求为假。调用方要把请求交给别的线程时，先用它判断，免得为一条快照请求复制最多 512 MiB 的文本。
pub fn handled_by_account(request: &CloudDictionaryRequest) -> bool {
    !matches!(
        request,
        CloudDictionaryRequest::SnapshotPreview
            | CloudDictionaryRequest::SnapshotExport
            | CloudDictionaryRequest::SnapshotRestorePreview { .. }
            | CloudDictionaryRequest::SnapshotRestore { .. }
            | CloudDictionaryRequest::SnapshotRestoreNative { .. }
            | CloudDictionaryRequest::SnapshotRestoreCancel
            | CloudDictionaryRequest::SnapshotChooseRestore
            | CloudDictionaryRequest::SnapshotEnqueue { .. }
            | CloudDictionaryRequest::SnapshotStatus
            | CloudDictionaryRequest::SnapshotCancel
    )
}

/// 把账号响应序列化给页面。序列化不了时按服务不可用报告，这是页面已经会处理的失败。
fn account_value<T: serde::Serialize>(value: T) -> Result<Value, AccountError> {
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable)
}

fn query(
    text: &str,
    kind: &str,
    scheme: &str,
    profile: &str,
    limit: usize,
) -> AccountCandidateQuery {
    AccountCandidateQuery {
        text: text.to_owned(),
        kind: kind.to_owned(),
        scheme: scheme.to_owned(),
        profile: profile.to_owned(),
        limit,
    }
}

/// 用账号会话处理一条云词库请求。快照类请求留给调用方，返回 `None`；错误是账号错误码（[`AccountError::code`]），词库种类不认识时是 `invalid_cloud_dictionary`。
pub fn account_request<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    request: &CloudDictionaryRequest,
) -> Option<Result<Value, &'static str>> {
    let result = match request {
        CloudDictionaryRequest::List {
            kind,
            offset,
            search,
        } => dictionary_kind(kind).and_then(|kind| {
            session
                .dictionary(kind, search, *offset)
                .and_then(account_value)
                .map_err(|error| error.code())
        }),
        CloudDictionaryRequest::Catalog {
            kind,
            code,
            offset,
            scheme,
            profile,
        } => dictionary_kind(kind).and_then(|kind| {
            session
                .dictionary_catalog(kind, code, *offset, scheme, profile)
                .and_then(|page| {
                    account_value(serde_json::json!({
                        "catalog_entries": page.entries,
                        "has_more": page.has_more,
                        "offset": page.offset,
                        "revision": page.revision,
                        "normalized": page.normalized,
                    }))
                })
                .map_err(|error| error.code())
        }),
        CloudDictionaryRequest::Changes { after, limit } => session
            .dictionary_changes(*after, *limit)
            .and_then(account_value)
            .map_err(|error| error.code()),
        CloudDictionaryRequest::Add {
            kind,
            code,
            word,
            weight,
        } => dictionary_kind(kind).and_then(|kind| {
            session
                .add_dictionary(kind, code, word, *weight)
                .and_then(account_value)
                .map_err(|error| error.code())
        }),
        CloudDictionaryRequest::Update {
            kind,
            id,
            code,
            word,
            weight,
            revision,
        } => dictionary_kind(kind).and_then(|kind| {
            session
                .update_dictionary(kind, id, code, word, *weight, *revision)
                .and_then(account_value)
                .map_err(|error| error.code())
        }),
        CloudDictionaryRequest::EditCatalog {
            kind,
            code,
            word,
            revision,
            replacement,
        } => dictionary_kind(kind).and_then(|kind| {
            let replacement = replacement
                .as_ref()
                .map(|value| (value.code.as_str(), value.word.as_str(), value.weight));
            session
                .edit_dictionary_catalog(kind, code, word, *revision, replacement)
                .and_then(account_value)
                .map_err(|error| error.code())
        }),
        CloudDictionaryRequest::Candidates {
            text,
            kind,
            scheme,
            profile,
            limit,
        } => session
            .personal_candidates(&query(text, kind, scheme, profile, *limit))
            .and_then(account_value)
            .map_err(|error| error.code()),
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
        } => session
            .rank_candidate(
                &query(text, kind, scheme, profile, *limit),
                code,
                word,
                *revision,
                mode,
                *linear_step,
                *trigger_count,
                *force_top,
            )
            .map(|result| {
                serde_json::json!({
                    "revision": result.revision,
                    "changed": result.changed,
                    "selection_count": result.selection.count,
                })
            })
            .map_err(|error| error.code()),
        CloudDictionaryRequest::RemoveCandidate {
            text,
            kind,
            scheme,
            profile,
            limit,
            code,
            word,
            revision,
        } => session
            .remove_candidate(
                &query(text, kind, scheme, profile, *limit),
                code,
                word,
                *revision,
            )
            .and_then(account_value)
            .map_err(|error| error.code()),
        CloudDictionaryRequest::FixedPositions { context, offset } => session
            .fixed_positions(context, *offset)
            .and_then(account_value)
            .map_err(|error| error.code()),
        CloudDictionaryRequest::SetFixedPosition {
            context,
            code,
            word,
            position,
            revision,
        } => session
            .set_fixed_position(context, code, word, *position, *revision)
            .and_then(account_value)
            .map_err(|error| error.code()),
        CloudDictionaryRequest::Delete { kind, id, revision } => {
            dictionary_kind(kind).and_then(|kind| {
                session
                    .delete_dictionary(kind, id, *revision)
                    .and_then(account_value)
                    .map_err(|error| error.code())
            })
        }
        CloudDictionaryRequest::Import { kind, format, text } => {
            dictionary_kind(kind).and_then(|kind| {
                session
                    .import_dictionary(kind, format, text)
                    .and_then(account_value)
                    .map_err(|error| error.code())
            })
        }
        CloudDictionaryRequest::Export { kind, format } => dictionary_kind(kind).and_then(|kind| {
            session
                .export_dictionary(kind, format)
                .map(|result| {
                    serde_json::json!({
                        "text": result.text,
                        "filename": result.filename,
                    })
                })
                .map_err(|error| error.code())
        }),
        CloudDictionaryRequest::SnapshotPreview
        | CloudDictionaryRequest::SnapshotExport
        | CloudDictionaryRequest::SnapshotRestorePreview { .. }
        | CloudDictionaryRequest::SnapshotRestore { .. }
        | CloudDictionaryRequest::SnapshotRestoreNative { .. }
        | CloudDictionaryRequest::SnapshotRestoreCancel
        | CloudDictionaryRequest::SnapshotChooseRestore
        | CloudDictionaryRequest::SnapshotEnqueue { .. }
        | CloudDictionaryRequest::SnapshotStatus
        | CloudDictionaryRequest::SnapshotCancel => return None,
    };
    Some(result)
}
