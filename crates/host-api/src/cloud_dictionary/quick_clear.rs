//! 原生桌面的云端快捷短语清空：正文留在共享层，界面只确认条数和一次性令牌。

use super::{full_backup, quick_phrase, snapshot_files::publish_snapshot_preview};
use full_backup::CloudDictionaryState;
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_client_core::uuid::Uuid;
use msime_client_services::account::{
    AccountApi, AccountError, AccountSessionStorage, BackendAccountSession,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) struct Pending {
    account: String,
    generation: u64,
    text: String,
    revision: i64,
    entries: u64,
    created: Instant,
}

/// 复用完整快照下载和校验；一次只保留一个预览，不把整份词库经 C ABI 交给界面。
pub(super) fn preview<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
) -> Result<Value, &'static str> {
    cancel(state)?;
    let (account, generation) = session
        .with_current_identity(|identity| {
            identity
                .map(|(id, generation)| (id.to_owned(), generation))
                .ok_or(AccountError::Unauthorized)
        })
        .map_err(|error| error.code())?;
    let exported = full_backup::export(session, state)?;
    let filtered =
        quick_phrase::filter_snapshot(exported["text"].as_str().ok_or("snapshot_invalid")?)?;
    let entries = filtered["entries"].as_u64().ok_or("snapshot_invalid")?;
    let revision = filtered["revision"].as_i64().ok_or("snapshot_invalid")?;
    let token = Uuid::new_v4().to_string();
    let pending = Pending {
        account: account.clone(),
        generation,
        text: filtered["text"]
            .as_str()
            .ok_or("snapshot_invalid")?
            .to_owned(),
        revision,
        entries,
        created: Instant::now(),
    };
    // 下载期间退出、换号或重新登录，旧结果不能重新成为可提交的预览。
    publish_snapshot_preview(
        session,
        generation,
        &account,
        &state.quick_clear,
        token.clone(),
        pending,
    )
    .map_err(|error| error.code())?;
    Ok(json!({"token": token, "entries": entries, "revision": revision}))
}

/// 确认只消费一次预览；上传绑定原登录和导出 revision，服务端 CAS 拒绝并发覆盖。
pub(super) fn apply<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    token: &str,
) -> Result<Value, &'static str> {
    let token = Uuid::parse_str(token)
        .map_err(|_| "snapshot_invalid")?
        .to_string();
    let pending = state
        .quick_clear
        .lock()
        .map_err(|_| "snapshot_unavailable")?
        .remove(&token)
        .ok_or("snapshot_invalid")?;
    if pending.created.elapsed() > Duration::from_secs(15 * 60) {
        return Err("snapshot_invalid");
    }
    if pending.entries == 0 {
        session
            .with_generation(pending.generation, Some(&pending.account), || Ok(()))
            .map_err(|error| error.code())?;
        return Ok(json!({"cleared": 0, "revision": pending.revision}));
    }
    let current = session
        .authenticated_as(&pending.account, pending.generation, |api, token| {
            api.dictionary_catalog(DictionaryKind::Quick, "", 0, "pinyin", "xiaohe", token)
                .map_err(Into::into)
        })
        .map_err(|failure| failure.code())?;
    if current.revision != pending.revision {
        return Err("account_conflict");
    }
    let result = session
        .authenticated_as(&pending.account, pending.generation, |api, token| {
            api.restore_dictionary_snapshot(pending.text.as_bytes(), pending.revision, token)
                .map_err(Into::into)
        })
        .map_err(|failure| failure.code())?;
    Ok(json!({"cleared": pending.entries, "revision": result.revision}))
}

/// 取消只丢弃当前进程的预览，不访问或修改云端。
pub(super) fn cancel(state: &CloudDictionaryState) -> Result<Value, &'static str> {
    state
        .quick_clear
        .lock()
        .map_err(|_| "snapshot_unavailable")?
        .clear();
    Ok(Value::Null)
}
