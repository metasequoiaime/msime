//! 桌面宿主的后台更新检查：多久问一次、问过以后记下什么、哪个新版本该提醒、关于页打开时先显示什么。
//!
//! 各宿主只负责什么时候调用、怎么提醒：Linux 的设置应用在打开设置窗口时调用 [`run_background_check`]，有新版本时发桌面通知；Windows 的常驻进程定期调用它，有新版本时弹系统通知；关于页打开时调用 [`pending_update`] 先显示记录里找到的新版本。原生宿主经由 host-api 的 `msime_client_update_background_check` 和 `msime_client_update_pending` 调到这里。
//!
//! 记录是宿主状态目录里的 `update-check.json`（[`RECORD_FILE`]），写的时候持有同目录的 `update-check.lock`。成功检查后一天内不再联网，连不上时记下尝试的时间、六小时后再试；同一个构建（版本号和运行 ID）只提醒一次。通道变了，旧记录作废：立刻重查，旧通道找到的新版本不再显示。
//!
//! 构建时或运行时设置了 `MSIME_DISABLE_UPDATE_CHECK`（值非空且不是 `0`）时不做后台检查，开发构建（`debug_assertions`）也不做；关于页的「检查更新」按钮不受影响。

use super::{
    check_for_update, is_newer_build, parse_version, ReleaseUpdate, UpdateChannel, UpdateCheck,
    UpdateCheckError, UpdateCheckRequest, BUILD_RUN,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// 状态目录里的记录文件名。
pub const RECORD_FILE: &str = "update-check.json";
/// 后台检查持有的锁文件名；另一个进程正在检查时这次直接跳过。
const LOCK_FILE: &str = "update-check.lock";
/// 两次成功的后台检查之间至少隔多久，与 macOS 输入法的每日检查一致。
pub const INTERVAL_SECONDS: u64 = 24 * 60 * 60;
/// 连不上 GitHub 时隔多久再试。失败也记下尝试的时间，断网期间不会每次都再问一次。
pub const RETRY_SECONDS: u64 = 6 * 60 * 60;
/// 关掉后台检查的环境变量：发行版打包时在构建环境里设置（编进二进制），或者在运行宿主的环境里设置。值非空且不是 `0` 才算关掉。
pub const DISABLE_ENV: &str = "MSIME_DISABLE_UPDATE_CHECK";
/// 记录只有几个短字段和一个发布描述，读的时候限制大小，坏文件当作从没检查过。
const RECORD_LIMIT: u64 = 16 * 1024;

/// 上次后台检查的记录。
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundRecord {
    /// 上次成功检查的 Unix 时间（秒）。
    #[serde(default)]
    pub checked_at: u64,
    /// 上次尝试检查的 Unix 时间（秒），连不上时也记；旧记录没有这一项，按从没尝试过处理。
    #[serde(default)]
    pub attempted_at: u64,
    /// 这份记录是按哪个通道检查的。以前的记录没有这一项，那时只有稳定版。
    #[serde(default)]
    pub channel: UpdateChannel,
    /// 已经提醒过的版本号；和 `notified_run` 一起标识提醒过的那个构建。
    #[serde(default)]
    pub notified_version: Option<String>,
    /// 已经提醒过的构建的运行 ID；以前的记录只有 `notified_version`，按没有运行 ID 处理。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notified_run: Option<u64>,
    /// 上次检查找到的新版本。关于页打开时显示它，不必等下一次检查；运行的构建追上它以后不再显示。
    #[serde(default)]
    pub available: Option<ReleaseUpdate>,
}

/// 一次后台检查做了什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundStatus {
    /// 后台检查被关掉了（开发构建或 `MSIME_DISABLE_UPDATE_CHECK`），什么也没做。
    Disabled,
    /// 还没到时间，或者另一个进程正在检查，没有联网。
    NotDue,
    /// 联网了但连不上或 GitHub 返回了错误，记下了这次尝试。
    Unavailable,
    /// 检查成功，记录已更新。
    Checked,
}

/// 一次后台检查的结果，宿主序列化后的形状为 `{"status": "disabled" | "not_due" | "unavailable" | "checked", "notify": ReleaseUpdate | null}`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackgroundOutcome {
    pub status: BackgroundStatus,
    /// 这次该提醒的新版本：找到了比运行的构建新、而且还没提醒过的构建。返回时记录里已经把它记成提醒过了，宿主发一次通知即可。
    pub notify: Option<ReleaseUpdate>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BackgroundError {
    /// 请求没有给出平台，当前运行的版本号或通道不合法，或者状态目录不是绝对路径。
    #[error("invalid update check request")]
    Invalid,
    /// 状态目录建不起来或记录写不进去。
    #[error("the update check record is unavailable")]
    Storage,
}

impl From<std::io::Error> for BackgroundError {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}

/// 现在是否该再检查一次。通道和记录的不同时立刻检查；否则要距上次成功检查满一天、距上次尝试满 [`RETRY_SECONDS`]。时钟被往回拨过（记录在将来）时按已经满了处理。
pub fn background_check_due(record: &BackgroundRecord, now: u64, channel: UpdateChannel) -> bool {
    if record.channel != channel {
        return true;
    }
    let elapsed = |since: u64, interval: u64| now < since || now - since >= interval;
    elapsed(record.checked_at, INTERVAL_SECONDS) && elapsed(record.attempted_at, RETRY_SECONDS)
}

/// 后台检查是否被关掉：`build` 是构建时的 `MSIME_DISABLE_UPDATE_CHECK`，`runtime` 是运行时的；任何一个非空且不是 `0` 就关掉。
pub fn background_check_disabled(build: Option<&str>, runtime: Option<&OsStr>) -> bool {
    let set = |value: &OsStr| !value.is_empty() && value != "0";
    build.is_some_and(|value| set(OsStr::new(value))) || runtime.is_some_and(set)
}

/// 这个进程里后台检查是否被关掉：开发构建（`debug_assertions`），或者构建时、运行时设置了 [`DISABLE_ENV`]。宿主可以先问它，决定要不要起后台线程。
pub fn automatic_checks_disabled() -> bool {
    cfg!(debug_assertions)
        || background_check_disabled(
            option_env!("MSIME_DISABLE_UPDATE_CHECK"),
            std::env::var_os(DISABLE_ENV).as_deref(),
        )
}

/// 这次检查结果要不要提醒：有新版本，且这个构建（版本号和运行 ID）还没提醒过。只有 `notified_version` 的旧记录按运行 ID 为空比较，所以升级后对同一个版本不会再提醒一次。
pub fn background_notification(
    check: &UpdateCheck,
    record: &BackgroundRecord,
) -> Option<ReleaseUpdate> {
    match check {
        UpdateCheck::Available(update)
            if record.notified_version.as_deref() != Some(update.version.display.as_str())
                || record.notified_run.unwrap_or(0) != update.build_run.unwrap_or(0) =>
        {
            Some(update.clone())
        }
        _ => None,
    }
}

/// 关于页要显示的后台检查结果：记录是按 `channel` 检查的，且找到的新版本仍比运行的构建（版本号 `current_version`、运行 ID `current_run`）新时才算；升级以后、换了通道以后旧记录不再提示。
pub fn pending_from_record(
    record: &BackgroundRecord,
    current_version: &str,
    channel: UpdateChannel,
    current_run: Option<u64>,
) -> Option<UpdateCheck> {
    if record.channel != channel {
        return None;
    }
    let update = record.available.as_ref()?;
    let current = parse_version(current_version)?;
    is_newer_build(update, &current, current_run).then(|| UpdateCheck::Available(update.clone()))
}

/// 在 `directory`（宿主的状态目录，必须是绝对路径）里做一次后台检查：未到时间、被关掉或另一个进程正在检查时什么也不做，否则联网检查、写回记录，并返回该提醒的新版本。会在网络上阻塞，不要在 UI 线程或输入线程上调用。
pub fn run_background_check(
    directory: &Path,
    request: &UpdateCheckRequest,
) -> Result<BackgroundOutcome, BackgroundError> {
    validate(directory, request)?;
    if automatic_checks_disabled() {
        return Ok(BackgroundOutcome {
            status: BackgroundStatus::Disabled,
            notify: None,
        });
    }
    run_background_check_with(directory, request, unix_now(), check_for_update)
}

/// [`run_background_check`] 去掉开关判断后的部分：`now` 是当前 Unix 时间（秒），`check` 代替联网检查。
pub fn run_background_check_with(
    directory: &Path,
    request: &UpdateCheckRequest,
    now: u64,
    check: impl FnOnce(&UpdateCheckRequest) -> Result<UpdateCheck, UpdateCheckError>,
) -> Result<BackgroundOutcome, BackgroundError> {
    let channel = validate(directory, request)?;
    if !crate::storage::create_directory_and_check(directory)? {
        return Err(BackgroundError::Storage);
    }
    let private = crate::file_lock::open_private_directory(directory)?;
    let lock = crate::file_lock::open_private_lock_file_at(&private, OsStr::new(LOCK_FILE))?;
    let Some(_lock) = crate::file_lock::ScopedFileLock::try_exclusive(lock)? else {
        return Ok(BackgroundOutcome {
            status: BackgroundStatus::NotDue,
            notify: None,
        });
    };
    let record = read_record_at(&private);
    if !background_check_due(&record, now, channel) {
        return Ok(BackgroundOutcome {
            status: BackgroundStatus::NotDue,
            notify: None,
        });
    }
    // 换了通道：旧通道的检查时间、找到的版本都不再算数，只保留提醒过哪个构建（同一个构建不会因为换通道再提醒一次）。
    let base = if record.channel == channel {
        record
    } else {
        BackgroundRecord {
            channel,
            notified_version: record.notified_version,
            notified_run: record.notified_run,
            ..BackgroundRecord::default()
        }
    };
    let write = |record: &BackgroundRecord| -> Result<(), BackgroundError> {
        let bytes = serde_json::to_vec(record).map_err(|_| BackgroundError::Storage)?;
        crate::file_lock::write_private_file_at(&private, OsStr::new(RECORD_FILE), &bytes)?;
        Ok(())
    };
    let Ok(result) = check(request) else {
        // 连不上时只记下这次尝试，其余照旧：过 `RETRY_SECONDS` 再试，检查成功才算这一天检查过了。
        write(&BackgroundRecord {
            attempted_at: now,
            ..base
        })?;
        return Ok(BackgroundOutcome {
            status: BackgroundStatus::Unavailable,
            notify: None,
        });
    };
    let notify = background_notification(&result, &base);
    let (notified_version, notified_run) = match &notify {
        Some(update) => (Some(update.version.display.clone()), update.build_run),
        None => (base.notified_version, base.notified_run),
    };
    write(&BackgroundRecord {
        checked_at: now,
        attempted_at: now,
        channel,
        notified_version,
        notified_run,
        available: match result {
            UpdateCheck::Available(update) => Some(update),
            UpdateCheck::Current(_) | UpdateCheck::None => None,
        },
    })?;
    Ok(BackgroundOutcome {
        status: BackgroundStatus::Checked,
        notify,
    })
}

/// 关于页打开时先显示的、后台检查已经找到的新版本（见 [`pending_from_record`]），当前运行构建的运行 ID 取 [`BUILD_RUN`]。没有记录、记录坏了或已经不新时为 `None`。只读不写，不拿锁，不联网。
pub fn pending_update(
    directory: &Path,
    request: &UpdateCheckRequest,
) -> Result<Option<UpdateCheck>, BackgroundError> {
    let channel = validate(directory, request)?;
    Ok(pending_from_record(
        &read_record(directory),
        &request.current_version,
        channel,
        BUILD_RUN,
    ))
}

/// 读 `directory` 里的记录；没有、读不了或不是记录时为空记录。
pub fn read_record(directory: &Path) -> BackgroundRecord {
    crate::file_lock::open_private_directory(directory)
        .map(|private| read_record_at(&private))
        .unwrap_or_default()
}

fn read_record_at(directory: &crate::file_lock::PrivateDirectory) -> BackgroundRecord {
    crate::file_lock::open_private_file_at(directory, OsStr::new(RECORD_FILE))
        .ok()
        .and_then(|file| crate::bounded_io::read_bounded(file, RECORD_LIMIT).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn validate(
    directory: &Path,
    request: &UpdateCheckRequest,
) -> Result<UpdateChannel, BackgroundError> {
    if !directory.is_absolute()
        || request.platform.is_empty()
        || request.platform.len() > 32
        || parse_version(&request.current_version).is_none()
    {
        return Err(BackgroundError::Invalid);
    }
    request.effective_channel().ok_or(BackgroundError::Invalid)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests;
