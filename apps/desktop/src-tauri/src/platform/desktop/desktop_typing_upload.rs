//! 打字统计的「上传到云端」和「绑定 QQ 群助手」：三个桌面宿主共用的命令。
//!
//! 上传的构建、节流和状态文件在 client-services（`typing_statistics::upload`），这里只把统计目录和账号会话接起来。设置应用没有常驻进程，所以不定时上传：页面在应用启动和打开统计页时调一次 [`upload_typing_statistics`]，距上次不到十分钟时什么也不做。打开开关时立刻上传一次；关掉时先在本地关掉，再删云端副本，删除失败也保持关闭，把错误交给页面说明。

use crate::platform::account_helpers::call_session_detailed;
use crate::platform::desktop::desktop_account::AccountState;
use crate::TypingStatisticsState;
use msime_client_services::account::{AccountError, AccountFailure, AccountQqLink, AccountQqLinks};
use msime_client_services::typing_statistics::upload::{UploadError, UploadStatus};
use msime_client_services::typing_statistics::TypingStatisticsStore;

/// 上传带上的平台，和服务端认的 id 一致。
const PLATFORM: &str = if cfg!(target_os = "macos") {
    "macos"
} else if cfg!(target_os = "windows") {
    "windows"
} else {
    "linux"
};

/// 本机的本地日期。统计按用户本地的日子记，取不到本地时区时退回 UTC，与统计页的其他命令相同。
fn today() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let date = now.date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

fn storage_error() -> AccountFailure {
    AccountError::Storage.into()
}

/// 打开了上传、已登录时上传最近几天；`force` 时不看十分钟的间隔。没有登录时不上传，也不报错：开关还开着，登录后下一次照常上传。
fn upload(
    store: &TypingStatisticsStore,
    session: &crate::platform::desktop::desktop_account::Session,
    force: bool,
) -> Result<UploadStatus, AccountFailure> {
    if session.status().map_err(AccountFailure::from)?.is_some() {
        match store.upload_if_due(&today(), PLATFORM, now_ms(), force, |payload| {
            session.put_typing_statistics(payload)
        }) {
            Ok(_) => {}
            Err(UploadError::Send(failure)) => return Err(failure),
            Err(UploadError::Storage(_)) => return Err(storage_error()),
        }
    }
    store
        .upload_state()
        .map(|state| UploadStatus::from(&state))
        .map_err(|_| storage_error())
}

#[tauri::command]
pub async fn typing_statistics_upload_status(
    state: tauri::State<'_, TypingStatisticsState>,
) -> Result<UploadStatus, crate::CommandError> {
    let store = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        store
            .upload_state()
            .map(|state| UploadStatus::from(&state))
            .map_err(|_| crate::CommandError { code: "storage" })
    })
    .await
    .map_err(|_| crate::CommandError { code: "storage" })?
}

/// 打开时要求已登录，并立刻上传一次；上传失败时开关保持打开，错误交给页面。关闭时先在本地关掉，再删除云端副本。
#[tauri::command]
pub async fn set_typing_statistics_upload(
    statistics: tauri::State<'_, TypingStatisticsState>,
    account: tauri::State<'_, AccountState>,
    enabled: bool,
) -> Result<UploadStatus, crate::CommandError> {
    let store = statistics.0.clone();
    call_session_detailed(&account.session, move |session| {
        if enabled {
            if session.status().map_err(AccountFailure::from)?.is_none() {
                return Err(AccountError::Unauthorized.into());
            }
            store
                .set_upload_enabled(true)
                .map_err(|_| storage_error())?;
            upload(&store, session, true)
        } else {
            let state = store
                .set_upload_enabled(false)
                .map_err(|_| storage_error())?;
            session.delete_typing_statistics()?;
            Ok(UploadStatus::from(&state))
        }
    })
    .await
}

/// 页面在应用启动和打开统计页时调用；距上次上传不到十分钟时不发请求。
#[tauri::command]
pub async fn upload_typing_statistics(
    statistics: tauri::State<'_, TypingStatisticsState>,
    account: tauri::State<'_, AccountState>,
) -> Result<UploadStatus, crate::CommandError> {
    let store = statistics.0.clone();
    call_session_detailed(&account.session, move |session| {
        upload(&store, session, false)
    })
    .await
}

#[tauri::command]
pub async fn account_qq_links(
    state: tauri::State<'_, AccountState>,
) -> Result<AccountQqLinks, crate::CommandError> {
    call_session_detailed(&state.session, |session| session.qq_links()).await
}

#[tauri::command]
pub async fn account_qq_link_redeem(
    state: tauri::State<'_, AccountState>,
    code: String,
) -> Result<AccountQqLink, crate::CommandError> {
    call_session_detailed(&state.session, move |session| session.redeem_qq_link(&code)).await
}

#[tauri::command]
pub async fn account_qq_link_delete(
    state: tauri::State<'_, AccountState>,
    id: String,
) -> Result<(), crate::CommandError> {
    call_session_detailed(&state.session, move |session| session.delete_qq_link(&id)).await
}
