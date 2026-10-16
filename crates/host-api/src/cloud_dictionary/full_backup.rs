//! 完整云词库：下载预览、「应用到本机」、完整备份的导出与恢复。Windows 设置应用和 `msime_client_cloud_dictionary` 共用。
//!
//! 列表、目录、增删改、导入导出、候选调频和固定排位走 [`super::account::account_request`]，完整快照在这里。
//!
//! 「完整云词库备份」（导出、选文件预览、恢复到云端）只经过账号会话和本用户私有的临时目录。「应用到本机」把下载并校验过的快照交给状态目录下的持久队列 `dictionary-snapshots`（与鸿蒙经 `msime_client_snapshot_queue` 用的是同一个队列），随后请输入会话放开词库（[`QuiescedHosts`]），在本进程里准备并激活新的一代词库（暂存在同级的 `dictionary-snapshot-staging`），最后交还会话。会话没能放开、或激活没有完成时，请求留在队列里，面板下一次查询状态时再处理。
//!
//! 函数对账号会话的后端和存储泛型，测试用合成后端驱动它。函数会联网、读写文件，调用方在工作线程上调用；错误是页面认得的稳定错误码。

use super::account::account_request;
use super::snapshot_files::{
    cleanup_stale_scratch_files, cleanup_stale_snapshot_previews, copy_snapshot_file,
    prepare_snapshot_directory, publish_snapshot_preview, read_snapshot_file, remove_snapshot_file,
    take_invalid_snapshot_previews, write_snapshot_file, MAX_SNAPSHOT_BYTES,
};
use super::CloudDictionaryRequest;
use msime_client_core::cloud::dictionary::DictionaryKind;
use msime_client_core::uuid::Uuid;
use msime_client_services::account::{
    AccountApi, AccountError, AccountSessionStorage, BackendAccountSession,
};
use msime_client_services::dictionary::access::DictionaryAccess;
use msime_client_services::dictionary::quiesce::{QuiescedHosts, BUSY};
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
pub const EXPORT_FILENAME: &str = "msime-dictionary-snapshot.ndjson";
/// 面板轮询状态时，两次重新处理队列之间至少隔这么久。每次处理都要请输入会话放开词库、重新准备一整份词库，一份总是激活不了的快照不能让输入每两秒断一次。
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
/// 处理队列期间续一次放开会话的间隔，远小于输入会话自己收回词库的 30 秒。
const RELEASE_RENEW_INTERVAL: Duration = Duration::from_secs(10);

const UNAVAILABLE: &str = "snapshot_unavailable";
const INVALID: &str = "snapshot_invalid";

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

/// 一个临时目录的预览表和重试节拍。复制出来的值共用同一张表。
#[derive(Clone)]
pub struct CloudDictionaryState {
    /// 本用户私有的临时目录，放下载的预览，以及导出、恢复时的中转文件。
    directory: PathBuf,
    previews: Arc<Mutex<HashMap<String, PendingSnapshot>>>,
    /// 上一次处理队列的时间，见 [`RETRY_INTERVAL`]。
    last_process: Arc<Mutex<Option<Instant>>>,
}

impl CloudDictionaryState {
    /// 上次运行没来得及删的预览、导出与恢复的中转文件在这里清掉，所以一个目录在进程里只该建一次状态，也不该被另一个同时在跑的进程共用。清不掉不影响使用：下一次照样写新文件。
    pub fn new(directory: PathBuf) -> Self {
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

/// 处理面板的一条云词库请求。`options` 是本机的 HostOptions 文档，「应用到本机」用它找到词库和状态目录。宿主自己保存文件、选文件的那几条（`snapshot_restore_native`、`snapshot_restore_cancel`、`snapshot_choose_restore`）是 macOS 的，这里报 `snapshot_unavailable`。
pub fn request<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    options: Value,
    request: CloudDictionaryRequest,
) -> Result<Value, &'static str> {
    if let Some(result) = account_request(session, &request) {
        return result;
    }
    discard_stale_previews(session, state);
    match request {
        CloudDictionaryRequest::SnapshotPreview => preview(session, state, &options),
        CloudDictionaryRequest::SnapshotEnqueue { token } => {
            enqueue(session, state, &options, &token)
        }
        CloudDictionaryRequest::SnapshotStatus => status(session, state, &options),
        CloudDictionaryRequest::SnapshotCancel => cancel(session, state, &options),
        CloudDictionaryRequest::SnapshotExport => export(session, state),
        CloudDictionaryRequest::SnapshotRestorePreview { text } => {
            restore_preview(session, state, &text)
        }
        CloudDictionaryRequest::SnapshotRestore {
            text,
            expected_sha256,
            revision,
        } => restore(session, state, &text, &expected_sha256, revision),
        // 其余请求已经由上面的账号分发处理。
        _ => Err(UNAVAILABLE),
    }
}

/// 队列和 host-api 快照接口报的原因换成页面认得的错误码。
fn snapshot_error(reason: &str) -> &'static str {
    match reason {
        "snapshot_busy" => "snapshot_busy",
        "snapshot_conflict" => "snapshot_conflict",
        "snapshot_invalid" => INVALID,
        _ => UNAVAILABLE,
    }
}

/// HostOptions 的用户数据目录和它所在的状态目录（`<状态目录>/user`）。队列和暂存目录放在状态目录下，与词库在同一个卷上，激活时才能用改名换入新的一代。
fn state_paths(options: &Value) -> Result<(String, PathBuf), &'static str> {
    let user_data = options
        .get("user_data")
        .and_then(Value::as_str)
        .filter(|path| Path::new(path).is_absolute())
        .ok_or(UNAVAILABLE)?;
    let root = Path::new(user_data)
        .parent()
        .ok_or(UNAVAILABLE)?
        .to_path_buf();
    Ok((user_data.to_owned(), root))
}

fn queue_request(action: Value) -> Result<Value, &'static str> {
    let bytes = serde_json::to_vec(&action).map_err(|_| UNAVAILABLE)?;
    crate::snapshot_queue_json(&bytes).map_err(|reason| snapshot_error(&reason))
}

/// 读出队列状态，同时把本机词库的当前版本记进队列。`acknowledge` 为真时取走已结束请求的结果，与鸿蒙相同。
fn queue_state(options: &Value, acknowledge: bool) -> Result<Value, &'static str> {
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

/// 快照队列的状态里带着账号 id，交给页面之前去掉。
pub fn snapshot_response_without_account(mut value: Value) -> Result<Value, &'static str> {
    let object = value.as_object_mut().ok_or(UNAVAILABLE)?;
    if let Some(request) = object.get_mut("request").and_then(Value::as_object_mut) {
        request.remove("accountId");
    }
    Ok(value)
}

/// 本机词库现在能不能拿到独占锁。拿不到时报 [`BUSY`]，[`QuiescedHosts`] 据此请输入会话放开词库后重试。
fn maintenance_available(options: &Value) -> Result<(), String> {
    let path = |key: &str| {
        options
            .get(key)
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .ok_or_else(|| UNAVAILABLE.to_owned())
    };
    match DictionaryAccess::try_maintenance(&path("user_data")?, &path("dictionaries")?) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(BUSY.to_owned()),
        Err(_) => Err(UNAVAILABLE.to_owned()),
    }
}

/// 处理队列里属于 `account_id` 的请求：先请输入会话放开词库，确认词库能独占以后再准备并激活，`hosts` 离开作用域时交还会话。激活要拿词库的独占锁，输入会话还握着词库时一定失败，失败的激活不会把请求标成失败，但已经准备好的暂存会白白丢掉，所以先确认会话放开了再开始。放不开时请求留在队列里。
///
/// 输入会话在最后一次放开请求之后 30 秒自己收回词库，而准备整份词库可能更久，所以处理期间另起一个线程每隔 [`RELEASE_RENEW_INTERVAL`] 续一次放开；否则准备到一半会话就回来了，激活拿不到独占锁，请求每次重试都白白让输入断一阵。
fn process_queue(options: &Value, account_id: &str) -> Result<Value, &'static str> {
    let (user_data, root) = state_paths(options)?;
    let staging = root.join(STAGING_DIRECTORY);
    prepare_snapshot_directory(&staging).map_err(|_| UNAVAILABLE)?;
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
    crate::inspect_snapshot_json(path).map_err(|_| AccountError::Invalid)
}

fn status<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    options: &Value,
) -> Result<Value, &'static str> {
    let current = queue_state(options, false)?;
    // 入列之后没能马上处理完的请求（会话没放开、激活被打断），在面板轮询状态时接着处理，但不超过 RETRY_INTERVAL 一次；只处理当前登录账号的请求。
    let signed_in = session.status().ok().flatten().map(|user| user.id);
    if let Some(account_id) = signed_in
        .as_deref()
        .filter(|id| active_request_account(&current) == Some(*id))
        .filter(|_| process_due(&state.last_process, Instant::now(), RETRY_INTERVAL))
    {
        // 这次没处理成也照常报告状态，请求还在队列里，下一次轮询再试。
        let _ = process_queue(options, account_id);
    }
    snapshot_response_without_account(queue_state(options, true)?)
}

fn preview<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    options: &Value,
) -> Result<Value, &'static str> {
    let token = Uuid::new_v4().to_string();
    let local = queue_state(options, false)?;
    let local_version = local
        .get("localVersion")
        .and_then(Value::as_str)
        .ok_or(UNAVAILABLE)?
        .to_owned();
    prepare_snapshot_directory(&state.directory).map_err(|_| UNAVAILABLE)?;
    let profile = session.profile().map_err(|error| error.code())?;
    let (_, _, generation) = session
        .credentials_with_generation(None, Some(&profile.user.id))
        .map_err(|error| error.code())?;
    let path = state.directory.join(format!("download-{token}.ndjson"));
    let downloaded = session
        .dictionary_snapshot_to_file(&path)
        .and_then(|_| inspect(&path));
    let metadata = match downloaded {
        Ok(metadata) => metadata,
        Err(error) => {
            let _ = remove_snapshot_file(&path);
            return Err(error.code());
        }
    };
    let fields = (
        metadata.get("cloudRevision").and_then(Value::as_i64),
        metadata.get("fileSha256").and_then(Value::as_str),
    );
    let (Some(cloud_revision), Some(file_sha256)) = fields else {
        let _ = remove_snapshot_file(&path);
        return Err(UNAVAILABLE);
    };
    let pending = PendingSnapshot {
        account_id: profile.user.id,
        generation,
        path: path.clone(),
        cloud_revision,
        file_sha256: file_sha256.to_owned(),
        local_version,
    };
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
            return Err(error.code());
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
) -> Result<PendingSnapshot, &'static str> {
    let token = Uuid::parse_str(token).map_err(|_| INVALID)?;
    previews
        .lock()
        .map_err(|_| UNAVAILABLE)?
        .remove(&token.to_string())
        .ok_or(INVALID)
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

fn enqueue<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    options: &Value,
    token: &str,
) -> Result<Value, &'static str> {
    let pending = take_pending(&state.previews, token)?;
    let queued = (|| {
        validate_pending(session, &pending).map_err(|error| error.code())?;
        let (_, root) = state_paths(options)?;
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
            .map_err(|error| error.code())?
    })();
    // 队列已经复制了一份，预览文件不再需要。
    let _ = remove_snapshot_file(&pending.path);
    queued?;
    // 已经入列，马上处理；这次没能应用时报告排队中的状态，面板轮询时再处理。
    if let Ok(mut last) = state.last_process.lock() {
        *last = Some(Instant::now());
    }
    let result =
        process_queue(options, &pending.account_id).or_else(|_| queue_state(options, false))?;
    snapshot_response_without_account(result)
}

fn cancel<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    options: &Value,
) -> Result<Value, &'static str> {
    let account_id = session
        .status()
        .map_err(|error| error.code())?
        .ok_or("account_unauthorized")?
        .id;
    if let Ok(mut pending) = state.previews.lock() {
        for (_, preview) in pending.drain() {
            let _ = remove_snapshot_file(&preview.path);
        }
    }
    let (_, root) = state_paths(options)?;
    queue_request(json!({
        "operation": "cancel",
        "directory": root.join(QUEUE_DIRECTORY),
        "account_id": account_id,
    }))?;
    snapshot_response_without_account(queue_state(options, false)?)
}

/// 下载完整云端备份交给页面，页面用下载链接保存。
fn export<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
) -> Result<Value, &'static str> {
    let token = Uuid::new_v4();
    prepare_snapshot_directory(&state.directory).map_err(|_| AccountError::Unavailable.code())?;
    let path = state.directory.join(format!("export-{token}.ndjson"));
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
    result.map_err(|error| error.code())
}

/// 把完整云端备份保存到宿主给的 `destination`（绝对路径，宿主自己选的保存位置），返回 `{path, filename, snapshot}`。先下载到临时目录里的 `export-<token>.ndjson` 并校验，通过以后才复制过去换入 `destination`；未登录、令牌过期、联网失败、校验不过或复制失败时，`destination` 上原有的文件原样保留。默认文件名是固定的，覆盖上一份备份是最常见的用法，所以绝不能在失败时删掉目标。整份快照可能有 512 MiB，C ABI 用它代替把文本放进响应的 `snapshot_export`。
pub fn export_to_file<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    destination: &Path,
) -> Result<Value, &'static str> {
    // 保存位置不可用（上级目录不存在、不是目录或经过符号链接）时在下载之前就拒绝，不白下载一整份快照。
    let valid_destination = destination.is_absolute()
        && destination.file_name().is_some()
        && destination.parent().is_some_and(|parent| {
            msime_path_trust::reject_symlinked_components(parent).is_ok() && parent.is_dir()
        });
    if !valid_destination {
        return Err(AccountError::Invalid.code());
    }
    let token = Uuid::new_v4();
    prepare_snapshot_directory(&state.directory).map_err(|_| AccountError::Unavailable.code())?;
    let path = state.directory.join(format!("export-{token}.ndjson"));
    let result = session
        .dictionary_snapshot_to_file(&path)
        .and_then(|_| inspect(&path))
        .and_then(|metadata| {
            copy_snapshot_file(&path, destination).map_err(|_| AccountError::Unavailable)?;
            Ok(json!({
                "path": destination,
                "filename": EXPORT_FILENAME,
                "snapshot": public_metadata(metadata),
            }))
        });
    let _ = remove_snapshot_file(&path);
    result.map_err(|error| error.code())
}

/// 页面选的备份文件先在本机校验，再给出云端当前的 revision，恢复时用它防止覆盖别处刚做的修改。
fn restore_preview<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    text: &str,
) -> Result<Value, &'static str> {
    if text.len() > MAX_SNAPSHOT_BYTES {
        return Err(INVALID);
    }
    let token = Uuid::new_v4();
    prepare_snapshot_directory(&state.directory).map_err(|_| AccountError::Unavailable.code())?;
    let path = state.directory.join(format!("restore-{token}.ndjson"));
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
    result.map_err(|error| error.code())
}

/// 用预览过的备份替换云端词库。`expected_sha256` 是预览给出的内容校验和，文件在预览之后变了就拒绝。
fn restore<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    text: &str,
    expected_sha256: &str,
    revision: i64,
) -> Result<Value, &'static str> {
    if text.len() > MAX_SNAPSHOT_BYTES {
        return Err(INVALID);
    }
    let token = Uuid::new_v4();
    prepare_snapshot_directory(&state.directory).map_err(|_| AccountError::Unavailable.code())?;
    let path = state.directory.join(format!("restore-{token}.ndjson"));
    let result = write_snapshot_file(&path, text.as_bytes())
        .map_err(|_| AccountError::Unavailable)
        .and_then(|_| inspect(&path))
        .and_then(|metadata| {
            if metadata.get("sha256").and_then(Value::as_str) != Some(expected_sha256) {
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
    result.map_err(|error| error.code())
}

/// 读宿主选的备份文件（不跟随符号链接，最多 512 MiB，必须是 UTF-8），再按文本版请求的规则校验正文。之后与文本版一样先复制进临时目录再检查，检查的和上传的是同一份内容。
fn read_backup(path: &Path) -> Result<String, &'static str> {
    if !path.is_absolute() {
        return Err(INVALID);
    }
    let text = read_snapshot_file(path).map_err(|_| INVALID)?;
    if !super::valid_snapshot_text(&text) {
        return Err(INVALID);
    }
    Ok(text)
}

/// [`restore_preview`] 的文件版：`path` 是宿主选的备份文件。
pub fn restore_preview_from_file<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    path: &Path,
) -> Result<Value, &'static str> {
    discard_stale_previews(session, state);
    let text = read_backup(path)?;
    restore_preview(session, state, &text)
}

/// [`restore`] 的文件版：`path` 是宿主选的备份文件。
pub fn restore_from_file<A: AccountApi, S: AccountSessionStorage>(
    session: &BackendAccountSession<A, S>,
    state: &CloudDictionaryState,
    path: &Path,
    expected_sha256: &str,
    revision: i64,
) -> Result<Value, &'static str> {
    discard_stale_previews(session, state);
    let text = read_backup(path)?;
    restore(session, state, &text, expected_sha256, revision)
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
    use super::{
        export_to_file, process_due, request, restore_from_file, restore_preview_from_file,
        state_paths, CloudDictionaryState,
    };
    use crate::cloud_dictionary::CloudDictionaryRequest;
    use msime_client_core::cloud::dictionary::DictionaryKind;
    use msime_client_services::account::{
        AccountApi, AccountChallenge, AccountDictionaryCatalogPage, AccountDictionaryEntry,
        AccountDictionaryPage, AccountDictionarySnapshotRestore, AccountError, AccountProfile,
        AccountSessionStorage, AccountTokens, AccountUser, BackendAccountSession,
        SavedAccountSession,
    };
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

    /// 记下收到的云词库调用，并用合成数据回答；云词库以外的调用都算测试失败。`snapshot` 是下载完整快照时写出的正文，`None` 表示联网失败。
    struct FakeApi {
        calls: Arc<Mutex<Vec<String>>>,
        snapshot: Option<String>,
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
            let text = self.snapshot.clone().ok_or(AccountError::Unavailable)?;
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

    fn fixture() -> (Session, Calls, CloudDictionaryState, tempfile::TempDir) {
        fixture_with(Some(signed_in()), Some(snapshot()))
    }

    fn fixture_with(
        saved: Option<SavedAccountSession>,
        snapshot: Option<String>,
    ) -> (Session, Calls, CloudDictionaryState, tempfile::TempDir) {
        let calls = Calls::default();
        let session = BackendAccountSession::new(
            FakeApi {
                calls: Arc::clone(&calls),
                snapshot,
            },
            MemoryStorage(saved),
        );
        let directory = tempfile::tempdir().unwrap();
        let state = CloudDictionaryState::new(directory.path().join("dictionary-snapshots"));
        (session, calls, state, directory)
    }

    fn send(
        session: &Session,
        state: &CloudDictionaryState,
        action: Value,
    ) -> Result<Value, &'static str> {
        let action: CloudDictionaryRequest = serde_json::from_value(action).unwrap();
        request(session, state, json!({}), action)
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
            ".msime-private-1-2",
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
    fn a_full_backup_can_be_written_straight_to_the_chosen_file() {
        let (session, calls, state, directory) = fixture();
        let destination = directory.path().join("backup.ndjson");
        std::fs::write(&destination, "上一份合成备份\n").unwrap();
        let exported = export_to_file(&session, &state, &destination).unwrap();
        assert_eq!(exported["path"], json!(destination));
        assert_eq!(exported["filename"], "msime-dictionary-snapshot.ndjson");
        assert_eq!(exported["snapshot"]["sha256"], body_sha256());
        assert!(exported["snapshot"].get("fileSha256").is_none());
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), snapshot());
        assert_eq!(*calls.lock().unwrap(), ["snapshot"]);
        // 中转文件用完就删。
        let left = std::fs::read_dir(directory.path().join("dictionary-snapshots"))
            .unwrap()
            .count();
        assert_eq!(left, 0);
        assert_eq!(
            export_to_file(&session, &state, Path::new("relative.ndjson")),
            Err("account_invalid")
        );
    }

    #[test]
    fn a_missing_save_location_is_refused_before_downloading() {
        let (session, calls, state, directory) = fixture();
        let destination = directory.path().join("missing").join("backup.ndjson");
        assert_eq!(
            export_to_file(&session, &state, &destination),
            Err("account_invalid")
        );
        let file = directory.path().join("file");
        std::fs::write(&file, "合成").unwrap();
        assert_eq!(
            export_to_file(&session, &state, &file.join("backup.ndjson")),
            Err("account_invalid")
        );
        assert!(calls.lock().unwrap().is_empty());
        assert!(!directory.path().join("missing").exists());
    }

    #[test]
    fn a_copy_left_beside_the_save_location_by_a_killed_export_is_removed_next_time() {
        let (session, _calls, state, directory) = fixture();
        let stale = directory.path().join(".msime-export-synthetic-stale");
        let fresh = directory.path().join(".msime-export-synthetic-fresh");
        let unrelated = directory.path().join("synthetic-stale.ndjson");
        for path in [&stale, &fresh, &unrelated] {
            std::fs::write(path, "合成词库明文\n").unwrap();
        }
        let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
        for path in [&stale, &unrelated] {
            std::fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(two_hours_ago)
                .unwrap();
        }
        let destination = directory.path().join("backup.ndjson");
        export_to_file(&session, &state, &destination).unwrap();
        assert_eq!(std::fs::read_to_string(&destination).unwrap(), snapshot());
        // 只删超过一小时没动过的、自己前缀的那份；可能还在复制的新文件和别的文件都留着。
        assert!(!stale.exists());
        assert!(fresh.exists());
        assert!(unrelated.exists());
    }

    #[test]
    fn a_failed_full_backup_keeps_the_file_already_at_the_chosen_location() {
        let previous = "上一份合成备份\n";
        for (saved, snapshot, code) in [
            // 没登录：还没联网就失败。
            (None, Some(snapshot()), "account_unauthorized"),
            // 联网失败。
            (Some(signed_in()), None, "account_unavailable"),
            // 下载到的不是合法快照，校验不过。
            (
                Some(signed_in()),
                Some("{\"type\":\"header\"}\n".to_owned()),
                "account_invalid",
            ),
        ] {
            let (session, _calls, state, directory) = fixture_with(saved, snapshot);
            let destination = directory.path().join("msime-dictionary-snapshot.ndjson");
            std::fs::write(&destination, previous).unwrap();
            assert_eq!(export_to_file(&session, &state, &destination), Err(code));
            assert_eq!(std::fs::read_to_string(&destination).unwrap(), previous);
            // 中转文件和目标旁边的临时文件都不留下。
            let mut left: Vec<_> = std::fs::read_dir(directory.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect();
            left.sort();
            assert_eq!(
                left,
                ["dictionary-snapshots", "msime-dictionary-snapshot.ndjson"]
            );
            assert_eq!(std::fs::read_dir(&state.directory).unwrap().count(), 0);
        }
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
    fn a_backup_file_is_previewed_and_restored_like_the_text() {
        let (session, calls, state, directory) = fixture();
        let backup = directory.path().join("chosen.ndjson");
        std::fs::write(&backup, snapshot()).unwrap();
        let preview = restore_preview_from_file(&session, &state, &backup).unwrap();
        assert_eq!(preview["expectedRevision"], 9);
        assert_eq!(preview["snapshot"]["sha256"], body_sha256());
        let restored = restore_from_file(&session, &state, &backup, &body_sha256(), 9).unwrap();
        assert_eq!(restored, json!({ "revision": 10, "reset": true }));
        assert_eq!(
            restore_from_file(&session, &state, &backup, &"0".repeat(64), 9),
            Err("account_invalid")
        );
        assert_eq!(
            restore_preview_from_file(&session, &state, Path::new("chosen.ndjson")),
            Err("snapshot_invalid")
        );
        assert_eq!(
            *calls.lock().unwrap(),
            [
                "catalog".to_owned(),
                format!("restore:{}:9", snapshot().len())
            ]
        );
        // 原文件不动，中转文件用完就删。
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), snapshot());
        let left = std::fs::read_dir(directory.path().join("dictionary-snapshots"))
            .unwrap()
            .count();
        assert_eq!(left, 0);
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
