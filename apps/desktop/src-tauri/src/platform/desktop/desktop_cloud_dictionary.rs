//! Windows 设置应用的云词库。
//!
//! Windows 没有 Linux 那样的 provider socket，也没有 macOS 输入法交给面板的原生会话，所以云词库面板的每个请求都由设置应用用自己登录的账号会话完成：列表、目录、增删改、导入导出、候选调频和固定排位走与 iOS、Android 共用的 [`cloud_dictionary::account_request`]，完整快照在这里。
//!
//! 「完整云词库备份」（导出、选文件预览、恢复到云端）只经过账号会话和本用户私有的临时目录。「应用到本机」把下载并校验过的快照交给 Server 状态目录下的持久队列 `dictionary-snapshots`（与鸿蒙经 `msime_client_snapshot_queue` 用的是同一个队列），随后请 Server 经辅助管道放开输入会话，在本进程里准备并激活新的一代词库（暂存在同级的 `dictionary-snapshot-staging`），最后交还会话。会话没能放开、或激活没有完成时，请求留在队列里，面板下一次查询状态时再处理。
//!
//! 函数对账号会话的后端和存储泛型，测试在每个平台都用合成后端驱动它；只有 Windows 调用它。

use crate::platform::account_helpers::{
    account_command_error, cleanup_stale_snapshot_previews, prepare_snapshot_directory,
    publish_snapshot_preview, read_snapshot_file, remove_snapshot_file, snapshot_text_within_limit,
    take_invalid_snapshot_previews, write_snapshot_file,
};
use crate::platform::cloud_dictionary::{
    self, snapshot_command_error, snapshot_response_without_account,
};
use msime_client_core::account::{
    AccountApi, AccountError, AccountSessionStorage, BackendAccountSession,
};
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_client_core::dictionary::access::DictionaryAccess;
use msime_client_core::dictionary::quiesce::{QuiescedHosts, BUSY};
use msime_client_core::uuid::Uuid;
use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 状态目录下的持久快照队列，名字与鸿蒙相同。
const QUEUE_DIRECTORY: &str = "dictionary-snapshots";
/// 状态目录下准备新一代词库的暂存目录，名字与鸿蒙相同。
const STAGING_DIRECTORY: &str = "dictionary-snapshot-staging";
/// 「保存完整云端备份」下载的文件名，与 Android、iOS 相同。
const EXPORT_FILENAME: &str = "msime-dictionary-snapshot.ndjson";
/// 面板轮询状态时，两次重新处理队列之间至少隔这么久。每次处理都要请 Server 放开输入会话、重新准备一整份词库，一份总是激活不了的快照不能让输入每两秒断一次。
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
/// 处理队列期间续一次 Server 放开会话的间隔，远小于 Server 自己交还会话的 30 秒。
const RELEASE_RENEW_INTERVAL: Duration = Duration::from_secs(10);

/// 已下载、已校验、等用户确认替换本机词库的快照。
#[derive(Clone)]
struct PendingSnapshot {
    account_id: String,
    generation: u64,
    path: PathBuf,
    cloud_revision: i64,
    file_sha256: String,
    /// 预览时队列记下的本机词库版本；入列时原样交回，本机词库在这之后变过时队列会拒绝。
    local_version: String,
}

pub(crate) struct CloudDictionaryState {
    /// 本用户私有的临时目录，放下载的预览，以及导出、恢复时的中转文件。
    directory: PathBuf,
    previews: Arc<Mutex<HashMap<String, PendingSnapshot>>>,
    /// 上一次处理队列的时间，见 [`RETRY_INTERVAL`]。
    last_process: Arc<Mutex<Option<Instant>>>,
}

impl CloudDictionaryState {
    /// 上次运行没来得及删的预览、导出与恢复的中转文件在这里清掉。清不掉不影响启动：下一次照样写新文件。
    pub(crate) fn new(directory: PathBuf) -> Self {
        let _ = prepare_snapshot_directory(&directory)
            .and_then(|()| cleanup_stale_snapshot_previews(&directory))
            .and_then(|()| cleanup_stale_scratch_files(&directory));
        Self {
            directory,
            previews: Arc::default(),
            last_process: Arc::default(),
        }
    }
}

/// 删掉上次运行被打断时留下的导出、恢复中转文件（`export-*.ndjson`、`restore-*.ndjson`）和原子写入没来得及换入的临时文件（`.tmp*`）。这个目录在 `%LOCALAPPDATA%` 下长期存在，导出的中转文件是用户整份云词库的明文，不能一直留着。设置应用单实例，构造状态时本进程还没有导出或恢复在进行，所以这些文件都是上次留下的。
fn cleanup_stale_scratch_files(directory: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let scratch = ((file_name.starts_with("export-") || file_name.starts_with("restore-"))
            && file_name.ends_with(".ndjson"))
            || file_name.starts_with(".tmp");
        if scratch && entry.file_type()?.is_file() {
            let _ = remove_snapshot_file(&entry.path());
        }
    }
    Ok(())
}

/// 记下这一次处理队列；距上一次不到 `interval` 时返回假，调用方这次不处理。
fn process_due(last: &Mutex<Option<Instant>>, now: Instant, interval: Duration) -> bool {
    let Ok(mut last) = last.lock() else {
        return false;
    };
    if last.is_some_and(|previous| now.saturating_duration_since(previous) < interval) {
        return false;
    }
    *last = Some(now);
    true
}

/// 处理面板的一条云词库请求。`options` 是本机的 HostOptions 文档，「应用到本机」用它找到词库和状态目录。
pub(crate) async fn request<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    options: Value,
    request: CloudDictionaryRequest,
) -> Result<Value, crate::CommandError> {
    if let Some(result) = cloud_dictionary::account_request(session, &request).await {
        return result;
    }
    discard_stale_previews(session, state);
    match request {
        CloudDictionaryRequest::SnapshotPreview => preview(session, state, options).await,
        CloudDictionaryRequest::SnapshotEnqueue { token } => {
            enqueue(session, state, options, token).await
        }
        CloudDictionaryRequest::SnapshotStatus => status(session, state, options).await,
        CloudDictionaryRequest::SnapshotCancel => cancel(session, state, options).await,
        CloudDictionaryRequest::SnapshotExport => export(session, state).await,
        CloudDictionaryRequest::SnapshotRestorePreview { text } => {
            restore_preview(session, state, text).await
        }
        CloudDictionaryRequest::SnapshotRestore {
            text,
            expected_sha256,
            revision,
        } => restore(session, state, text, expected_sha256, revision).await,
        // 由宿主保存文件、选文件的那一套是 macOS 的；这里的备份走 WebView2 的下载链接和页面自己的文件选择。其余请求已经由上面的账号分发处理。
        _ => Err(snapshot_command_error()),
    }
}

/// 队列和 host-api 快照接口报的原因换成页面认得的错误码。
fn snapshot_error(reason: &str) -> crate::CommandError {
    crate::CommandError {
        code: match reason {
            "snapshot_busy" => "snapshot_busy",
            "snapshot_conflict" => "snapshot_conflict",
            "snapshot_invalid" => "snapshot_invalid",
            _ => "snapshot_unavailable",
        },
    }
}

/// HostOptions 的用户数据目录和它所在的状态目录（`<状态目录>\user`）。队列和暂存目录放在状态目录下，与词库在同一个卷上，激活时才能用改名换入新的一代。
fn state_paths(options: &Value) -> Result<(String, PathBuf), crate::CommandError> {
    let user_data = options
        .get("user_data")
        .and_then(Value::as_str)
        .filter(|path| Path::new(path).is_absolute())
        .ok_or_else(snapshot_command_error)?;
    let root = Path::new(user_data)
        .parent()
        .ok_or_else(snapshot_command_error)?
        .to_path_buf();
    Ok((user_data.to_owned(), root))
}

fn queue_request(action: Value) -> Result<Value, crate::CommandError> {
    let bytes = serde_json::to_vec(&action).map_err(|_| snapshot_command_error())?;
    msime_host_api::snapshot_queue_json(&bytes).map_err(|reason| snapshot_error(&reason))
}

/// 读出队列状态，同时把本机词库的当前版本记进队列。`acknowledge` 为真时取走已结束请求的结果，与鸿蒙相同。
fn queue_state(options: &Value, acknowledge: bool) -> Result<Value, crate::CommandError> {
    let (_, root) = state_paths(options)?;
    queue_request(json!({
        "operation": "state",
        "directory": root.join(QUEUE_DIRECTORY),
        "options": options,
        "acknowledge": acknowledge,
    }))
}

fn active_request_account(state: &Value) -> Option<&str> {
    let request = state.get("request")?;
    matches!(
        request.get("status").and_then(Value::as_str),
        Some("queued" | "preparing")
    )
    .then(|| request.get("accountId").and_then(Value::as_str))
    .flatten()
}

/// 本机词库现在能不能拿到独占锁。拿不到时报 [`BUSY`]，[`QuiescedHosts`] 据此请 Server 放开会话后重试。
fn maintenance_available(options: &Value) -> Result<(), String> {
    let path = |key: &str| {
        options
            .get(key)
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| "snapshot_unavailable".to_owned())
    };
    match DictionaryAccess::try_maintenance(&path("user_data")?, &path("dictionaries")?) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(BUSY.to_owned()),
        Err(_) => Err("snapshot_unavailable".to_owned()),
    }
}

/// 处理队列里属于 `account_id` 的请求：先请 Server 放开输入会话，确认词库能独占以后再准备并激活，`hosts` 离开作用域时交还会话。激活要拿词库的独占锁，Server 还握着会话时一定失败，失败的激活不会把请求标成失败，但已经准备好的暂存会白白丢掉，所以先确认会话放开了再开始。放不开时请求留在队列里。
///
/// Server 在最后一次 `DictionaryQuiesce` 之后 30 秒自己交还会话，而准备整份词库可能更久，所以处理期间另起一个线程每隔 [`RELEASE_RENEW_INTERVAL`] 续一次放开；否则准备到一半会话就回来了，激活拿不到独占锁，请求每次重试都白白让输入断一阵。
fn process_queue(options: &Value, account_id: &str) -> Result<Value, crate::CommandError> {
    let (user_data, root) = state_paths(options)?;
    let staging = root.join(STAGING_DIRECTORY);
    prepare_snapshot_directory(&staging).map_err(|_| snapshot_command_error())?;
    let mut hosts = QuiescedHosts::new(Some(user_data.as_str()), || {});
    if hosts.run(|| maintenance_available(options)).is_err() {
        return queue_state(options, false);
    }
    let (finished, renewals) = std::sync::mpsc::channel::<()>();
    std::thread::scope(|scope| {
        // `hosts` 交给续期线程，线程结束时它被丢掉，会话随之交还；作用域在返回前等这个线程结束。
        scope.spawn(move || {
            // 处理结束时发送端被丢掉，`recv_timeout` 不再超时，循环随之结束。没有放开过会话（本来就没有会话握着词库）时 `run` 什么也不做。
            while let Err(std::sync::mpsc::RecvTimeoutError::Timeout) =
                renewals.recv_timeout(RELEASE_RENEW_INTERVAL)
            {
                let _ = hosts.run(|| Ok::<(), String>(()));
            }
        });
        let result = queue_request(json!({
            "operation": "process",
            "directory": root.join(QUEUE_DIRECTORY),
            "staging_root": staging,
            "options": options,
            "account_id": account_id,
        }));
        drop(finished);
        result
    })
}

/// 快照元数据里只给页面看的部分：文件校验和与 Engine 记录数只在宿主内部比对。
fn public_metadata(mut metadata: Value) -> Value {
    if let Some(object) = metadata.as_object_mut() {
        object.remove("fileSha256");
        object.remove("engineRecords");
    }
    metadata
}

fn inspect(path: &Path) -> Result<Value, AccountError> {
    msime_host_api::inspect_snapshot_json(path).map_err(|_| AccountError::Invalid)
}

async fn status<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    options: Value,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(session);
    let last_process = Arc::clone(&state.last_process);
    let state = tauri::async_runtime::spawn_blocking(move || {
        let current = queue_state(&options, false)?;
        // 入列之后没能马上处理完的请求（会话没放开、激活被打断），在面板轮询状态时接着处理，但不超过 RETRY_INTERVAL 一次；只处理当前登录账号的请求。
        let signed_in = session.status().ok().flatten().map(|user| user.id);
        if let Some(account_id) = signed_in
            .as_deref()
            .filter(|id| active_request_account(&current) == Some(*id))
            .filter(|_| process_due(&last_process, Instant::now(), RETRY_INTERVAL))
        {
            // 这次没处理成也照常报告状态，请求还在队列里，下一次轮询再试。
            let _ = process_queue(&options, account_id);
        }
        queue_state(&options, true)
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    snapshot_response_without_account(state)
}

async fn preview<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    options: Value,
) -> Result<Value, crate::CommandError> {
    let worker_session = Arc::clone(session);
    let directory = state.directory.clone();
    let token = Uuid::new_v4().to_string();
    let file_token = token.clone();
    let pending = tauri::async_runtime::spawn_blocking(move || {
        let session = worker_session;
        let local = queue_state(&options, false)?;
        let local_version = local
            .get("localVersion")
            .and_then(Value::as_str)
            .ok_or_else(snapshot_command_error)?
            .to_owned();
        prepare_snapshot_directory(&directory).map_err(|_| snapshot_command_error())?;
        let profile = session.profile().map_err(account_command_error)?;
        let (_, _, generation) = session
            .credentials_with_generation(None, Some(&profile.user.id))
            .map_err(account_command_error)?;
        let path = directory.join(format!("download-{file_token}.ndjson"));
        let downloaded = session
            .dictionary_snapshot_to_file(&path)
            .and_then(|_| inspect(&path));
        let metadata = match downloaded {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = remove_snapshot_file(&path);
                return Err(account_command_error(error));
            }
        };
        let fields = (
            metadata.get("cloudRevision").and_then(Value::as_i64),
            metadata.get("fileSha256").and_then(Value::as_str),
        );
        let (Some(cloud_revision), Some(file_sha256)) = fields else {
            let _ = remove_snapshot_file(&path);
            return Err(snapshot_command_error());
        };
        Ok((
            PendingSnapshot {
                account_id: profile.user.id,
                generation,
                path,
                cloud_revision,
                file_sha256: file_sha256.to_owned(),
                local_version,
            },
            metadata,
        ))
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    let (pending, metadata) = pending;
    let path = pending.path.clone();
    let account_id = pending.account_id.clone();
    let published = publish_snapshot_preview(
        session,
        pending.generation,
        &account_id,
        &state.previews,
        token.clone(),
        pending,
    );
    match published {
        Ok(replaced) => {
            for preview in replaced {
                let _ = remove_snapshot_file(&preview.path);
            }
        }
        Err(error) => {
            let _ = remove_snapshot_file(&path);
            return Err(account_command_error(error));
        }
    }
    Ok(json!({
        "previewToken": token,
        "snapshot": public_metadata(metadata),
    }))
}

fn take_pending(
    previews: &Mutex<HashMap<String, PendingSnapshot>>,
    token: &str,
) -> Result<PendingSnapshot, crate::CommandError> {
    let invalid = || crate::CommandError {
        code: "snapshot_invalid",
    };
    let token = Uuid::parse_str(token).map_err(|_| invalid())?;
    previews
        .lock()
        .map_err(|_| snapshot_command_error())?
        .remove(&token.to_string())
        .ok_or_else(invalid)
}

/// 预览之后账号没换、云端词库也没再改动，才允许替换本机词库。
fn validate_pending<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    pending: &PendingSnapshot,
) -> Result<(), AccountError> {
    session.with_generation(pending.generation, Some(&pending.account_id), || Ok(()))?;
    if session.profile()?.user.id != pending.account_id {
        return Err(AccountError::Conflict);
    }
    if !session
        .dictionary_changes(pending.cloud_revision, 1)?
        .changes
        .is_empty()
    {
        return Err(AccountError::Conflict);
    }
    Ok(())
}

async fn enqueue<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    options: Value,
    token: String,
) -> Result<Value, crate::CommandError> {
    let pending = take_pending(&state.previews, &token)?;
    let session = Arc::clone(session);
    let last_process = Arc::clone(&state.last_process);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let queued = (|| {
            validate_pending(&session, &pending).map_err(account_command_error)?;
            let (_, root) = state_paths(&options)?;
            let action = json!({
                "operation": "enqueue",
                "directory": root.join(QUEUE_DIRECTORY),
                "source": pending.path,
                "account_id": pending.account_id,
                "cloud_revision": pending.cloud_revision,
                "expected_local_version": pending.local_version,
                "file_sha256": pending.file_sha256,
            });
            session
                .with_generation(pending.generation, Some(&pending.account_id), || {
                    Ok(queue_request(action))
                })
                .map_err(account_command_error)?
        })();
        // 队列已经复制了一份，预览文件不再需要。
        let _ = remove_snapshot_file(&pending.path);
        queued?;
        // 已经入列，马上处理；这次没能应用时报告排队中的状态，面板轮询时再处理。
        if let Ok(mut last) = last_process.lock() {
            *last = Some(Instant::now());
        }
        process_queue(&options, &pending.account_id).or_else(|_| queue_state(&options, false))
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    snapshot_response_without_account(result)
}

async fn cancel<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    options: Value,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(session);
    let previews = Arc::clone(&state.previews);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let account_id = session
            .status()
            .map_err(account_command_error)?
            .ok_or(crate::CommandError {
                code: "account_unauthorized",
            })?
            .id;
        if let Ok(mut pending) = previews.lock() {
            for (_, preview) in pending.drain() {
                let _ = remove_snapshot_file(&preview.path);
            }
        }
        let (_, root) = state_paths(&options)?;
        queue_request(json!({
            "operation": "cancel",
            "directory": root.join(QUEUE_DIRECTORY),
            "account_id": account_id,
        }))?;
        queue_state(&options, false)
    })
    .await
    .map_err(|_| snapshot_command_error())??;
    snapshot_response_without_account(result)
}

/// 下载完整云端备份交给页面，页面用下载链接保存。
async fn export<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
) -> Result<Value, crate::CommandError> {
    let session = Arc::clone(session);
    let directory = state.directory.clone();
    let token = Uuid::new_v4();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
        let path = directory.join(format!("export-{token}.ndjson"));
        let result = session
            .dictionary_snapshot_to_file(&path)
            .and_then(|_| inspect(&path))
            .and_then(|metadata| {
                let text = read_snapshot_file(&path).map_err(|_| AccountError::Unavailable)?;
                Ok(json!({
                    "text": text,
                    "filename": EXPORT_FILENAME,
                    "snapshot": public_metadata(metadata),
                }))
            });
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)
}

/// 页面选的备份文件先在本机校验，再给出云端当前的 revision，恢复时用它防止覆盖别处刚做的修改。
async fn restore_preview<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    text: String,
) -> Result<Value, crate::CommandError> {
    if !snapshot_text_within_limit(text.len()) {
        return Err(crate::CommandError {
            code: "snapshot_invalid",
        });
    }
    let session = Arc::clone(session);
    let directory = state.directory.clone();
    let token = Uuid::new_v4();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
        let path = directory.join(format!("restore-{token}.ndjson"));
        let result = write_snapshot_file(&path, text.as_bytes())
            .map_err(|_| AccountError::Unavailable)
            .and_then(|_| inspect(&path))
            .and_then(|metadata| {
                session
                    .dictionary_catalog(DictionaryKind::Quick, "", 0, "pinyin", "xiaohe")
                    .map(|page| {
                        json!({
                            "snapshot": public_metadata(metadata),
                            "expectedRevision": page.revision,
                        })
                    })
            });
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)
}

/// 用预览过的备份替换云端词库。`expected_sha256` 是预览给出的内容校验和，文件在预览之后变了就拒绝。
async fn restore<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    state: &CloudDictionaryState,
    text: String,
    expected_sha256: String,
    revision: i64,
) -> Result<Value, crate::CommandError> {
    if !snapshot_text_within_limit(text.len()) {
        return Err(crate::CommandError {
            code: "snapshot_invalid",
        });
    }
    let session = Arc::clone(session);
    let directory = state.directory.clone();
    let token = Uuid::new_v4();
    tauri::async_runtime::spawn_blocking(move || {
        prepare_snapshot_directory(&directory).map_err(|_| AccountError::Unavailable)?;
        let path = directory.join(format!("restore-{token}.ndjson"));
        let result = write_snapshot_file(&path, text.as_bytes())
            .map_err(|_| AccountError::Unavailable)
            .and_then(|_| inspect(&path))
            .and_then(|metadata| {
                if metadata.get("sha256").and_then(Value::as_str) != Some(expected_sha256.as_str())
                {
                    return Err(AccountError::Invalid);
                }
                session
                    .restore_dictionary_snapshot(text.as_bytes(), revision)
                    .map(|result| {
                        json!({
                            "revision": result.revision,
                            "reset": result.reset,
                        })
                    })
            });
        let _ = remove_snapshot_file(&path);
        result
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(account_command_error)
}

/// 退出登录、换号以后，丢掉属于旧会话的预览。
fn discard_stale_previews<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
) {
    if let Ok(stale) = take_invalid_snapshot_previews(session, &state.previews, |preview| {
        (&preview.account_id, preview.generation)
    }) {
        for preview in stale {
            let _ = remove_snapshot_file(&preview.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{process_due, request, state_paths, CloudDictionaryState};
    use msime_client_core::account::{
        AccountApi, AccountChallenge, AccountDictionaryCatalogPage, AccountDictionaryEntry,
        AccountDictionaryPage, AccountDictionarySnapshotRestore, AccountError, AccountProfile,
        AccountSessionStorage, AccountTokens, AccountUser, BackendAccountSession,
        SavedAccountSession,
    };
    use msime_client_core::cloud::dictionary::DictionaryKind;
    use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// 会话按 64 位小写十六进制校验保存的令牌，合成令牌也是这个形状。
    const ACCESS_TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// 只有表头的合成快照，云端 revision 是 7。
    fn snapshot() -> String {
        let body = concat!(
            r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}"#,
            "\n"
        );
        let digest = hex::encode(Sha256::digest(body.as_bytes()));
        format!("{body}{{\"type\":\"footer\",\"records\":1,\"sha256\":\"{digest}\"}}\n")
    }

    fn body_sha256() -> String {
        let body = concat!(
            r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}"#,
            "\n"
        );
        hex::encode(Sha256::digest(body.as_bytes()))
    }

    /// 记下收到的云词库调用，并用合成数据回答；云词库以外的调用都算测试失败。
    struct FakeApi {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl FakeApi {
        fn record(&self, call: String, token: &str) {
            assert_eq!(token, ACCESS_TOKEN);
            self.calls.lock().unwrap().push(call);
        }
    }

    impl AccountApi for FakeApi {
        fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
            unreachable!("providers")
        }
        fn challenge(&self, _: &str, _: &str) -> Result<AccountChallenge, AccountError> {
            unreachable!("challenge")
        }
        fn login(&self, _: &str, _: &str) -> Result<AccountTokens, AccountError> {
            unreachable!("login")
        }
        fn refresh(&self, _: &str) -> Result<AccountTokens, AccountError> {
            unreachable!("refresh")
        }
        fn profile(&self, _: &str) -> Result<AccountProfile, AccountError> {
            unreachable!("profile")
        }
        fn rename(&self, _: &str, _: &str) -> Result<(), AccountError> {
            unreachable!("rename")
        }
        fn logout(&self, _: &str, _: bool) -> Result<(), AccountError> {
            unreachable!("logout")
        }
        fn delete_account(&self, _: &str) -> Result<(), AccountError> {
            unreachable!("delete_account")
        }
        fn dictionary(
            &self,
            kind: DictionaryKind,
            search: &str,
            offset: usize,
            token: &str,
        ) -> Result<AccountDictionaryPage, AccountError> {
            self.record(format!("list:{search}:{offset}"), token);
            Ok(AccountDictionaryPage {
                entries: vec![AccountDictionaryEntry {
                    id: "synthetic-entry".into(),
                    kind,
                    code: "shui shan".into(),
                    word: "水杉".into(),
                    weight: 10,
                    revision: 3,
                }],
                has_more: false,
                offset,
            })
        }
        fn dictionary_catalog(
            &self,
            _: DictionaryKind,
            _: &str,
            _: usize,
            _: &str,
            _: &str,
            token: &str,
        ) -> Result<AccountDictionaryCatalogPage, AccountError> {
            self.record("catalog".into(), token);
            Ok(AccountDictionaryCatalogPage {
                entries: Vec::new(),
                has_more: false,
                offset: 0,
                revision: 9,
                normalized: String::new(),
            })
        }
        fn dictionary_snapshot_to_file(
            &self,
            destination: &Path,
            token: &str,
        ) -> Result<u64, AccountError> {
            self.record("snapshot".into(), token);
            let text = snapshot();
            std::fs::write(destination, &text).map_err(|_| AccountError::Storage)?;
            Ok(text.len() as u64)
        }
        fn restore_dictionary_snapshot(
            &self,
            snapshot: &[u8],
            revision: i64,
            token: &str,
        ) -> Result<AccountDictionarySnapshotRestore, AccountError> {
            self.record(format!("restore:{}:{revision}", snapshot.len()), token);
            Ok(AccountDictionarySnapshotRestore {
                revision: revision + 1,
                reset: true,
            })
        }
    }

    struct MemoryStorage(Option<SavedAccountSession>);

    impl AccountSessionStorage for MemoryStorage {
        fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
            Ok(self.0.clone())
        }
        fn save(&self, _: &SavedAccountSession) -> Result<(), AccountError> {
            Ok(())
        }
        fn clear(&self) -> Result<(), AccountError> {
            Ok(())
        }
    }

    fn signed_in() -> SavedAccountSession {
        SavedAccountSession {
            tokens: AccountTokens {
                access_token: ACCESS_TOKEN.into(),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "synthetic-user".into(),
                    display_name: "合成用户".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            // 一小时以后才过期，会话不会去刷新。
            expires_at_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
                + 3_600_000,
            session_id: None,
        }
    }

    type Session = BackendAccountSession<FakeApi, MemoryStorage>;
    type Calls = Arc<Mutex<Vec<String>>>;

    fn fixture() -> (Arc<Session>, Calls, CloudDictionaryState, tempfile::TempDir) {
        let calls = Calls::default();
        let session = Arc::new(BackendAccountSession::new(
            FakeApi {
                calls: Arc::clone(&calls),
            },
            MemoryStorage(Some(signed_in())),
        ));
        let directory = tempfile::tempdir().unwrap();
        let state = CloudDictionaryState::new(directory.path().join("dictionary-snapshots"));
        (session, calls, state, directory)
    }

    fn send(
        session: &Arc<Session>,
        state: &CloudDictionaryState,
        action: Value,
    ) -> Result<Value, &'static str> {
        let request: CloudDictionaryRequest = serde_json::from_value(action).unwrap();
        tauri::async_runtime::block_on(request_for(session, state, request))
    }

    async fn request_for(
        session: &Arc<Session>,
        state: &CloudDictionaryState,
        action: CloudDictionaryRequest,
    ) -> Result<Value, &'static str> {
        request(session, state, json!({}), action)
            .await
            .map_err(|error| error.code)
    }

    #[test]
    fn startup_removes_scratch_files_left_by_an_interrupted_run() {
        let directory = tempfile::tempdir().unwrap();
        let snapshots = directory.path().join("dictionary-snapshots");
        std::fs::create_dir_all(&snapshots).unwrap();
        for name in [
            "download-old.ndjson",
            "export-old.ndjson",
            "restore-old.ndjson",
            ".tmpA1b2C3",
            "export-in-progress",
            "notes.ndjson",
        ] {
            std::fs::write(snapshots.join(name), b"stale").unwrap();
        }
        let _state = CloudDictionaryState::new(snapshots.clone());
        let mut left: Vec<_> = std::fs::read_dir(&snapshots)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["export-in-progress", "notes.ndjson"]);
    }

    #[test]
    fn personal_entries_are_served_through_the_shared_account_dispatcher() {
        let (session, calls, state, _directory) = fixture();
        let page = send(
            &session,
            &state,
            json!({ "operation": "list", "kind": "pinyin", "offset": 0, "search": "水" }),
        )
        .unwrap();
        assert_eq!(page["entries"][0]["word"], "水杉");
        assert_eq!(*calls.lock().unwrap(), ["list:水:0"]);
    }

    #[test]
    fn a_full_backup_reaches_the_page_without_the_host_only_digests() {
        let (session, calls, state, directory) = fixture();
        let exported = send(&session, &state, json!({ "operation": "snapshot_export" })).unwrap();
        assert_eq!(exported["text"], snapshot());
        assert_eq!(exported["filename"], "msime-dictionary-snapshot.ndjson");
        assert_eq!(exported["snapshot"]["cloudRevision"], 7);
        assert_eq!(exported["snapshot"]["sha256"], body_sha256());
        assert!(exported["snapshot"].get("fileSha256").is_none());
        assert!(exported["snapshot"].get("engineRecords").is_none());
        assert_eq!(*calls.lock().unwrap(), ["snapshot"]);
        // 中转文件用完就删。
        let left = std::fs::read_dir(directory.path().join("dictionary-snapshots"))
            .unwrap()
            .count();
        assert_eq!(left, 0);
    }

    #[test]
    fn a_chosen_backup_is_previewed_and_then_restored_against_the_cloud_revision() {
        let (session, calls, state, _directory) = fixture();
        let preview = send(
            &session,
            &state,
            json!({ "operation": "snapshot_restore_preview", "text": snapshot() }),
        )
        .unwrap();
        assert_eq!(preview["expectedRevision"], 9);
        assert_eq!(preview["snapshot"]["sha256"], body_sha256());

        let restored = send(
            &session,
            &state,
            json!({
                "operation": "snapshot_restore",
                "text": snapshot(),
                "expected_sha256": body_sha256(),
                "revision": 9,
            }),
        )
        .unwrap();
        assert_eq!(restored, json!({ "revision": 10, "reset": true }));
        assert_eq!(
            *calls.lock().unwrap(),
            [
                "catalog".to_owned(),
                format!("restore:{}:9", snapshot().len())
            ]
        );
    }

    #[test]
    fn a_backup_that_changed_since_the_preview_is_not_uploaded() {
        let (session, calls, state, _directory) = fixture();
        let result = send(
            &session,
            &state,
            json!({
                "operation": "snapshot_restore",
                "text": snapshot(),
                "expected_sha256": "0".repeat(64),
                "revision": 9,
            }),
        );
        assert_eq!(result, Err("account_invalid"));
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn the_macos_file_picker_operations_are_unavailable() {
        let (session, calls, state, _directory) = fixture();
        for operation in ["snapshot_choose_restore", "snapshot_restore_cancel"] {
            assert_eq!(
                send(&session, &state, json!({ "operation": operation })),
                Err("snapshot_unavailable")
            );
        }
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn the_queue_lives_beside_the_user_dictionary_in_the_state_directory() {
        let user_data = std::env::temp_dir().join("state").join("user");
        let (_, root) = state_paths(&json!({ "user_data": user_data })).unwrap();
        assert_eq!(root, std::env::temp_dir().join("state"));
        assert!(state_paths(&json!({ "user_data": "relative/user" })).is_err());
        assert!(state_paths(&json!({})).is_err());
    }

    #[test]
    fn status_polls_retry_the_queue_at_most_once_per_interval() {
        let last = Mutex::new(None);
        let start = Instant::now();
        let interval = Duration::from_secs(60);
        assert!(process_due(&last, start, interval));
        assert!(!process_due(
            &last,
            start + Duration::from_secs(2),
            interval
        ));
        assert!(process_due(&last, start + interval, interval));
    }
}
