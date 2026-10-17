//! 打字统计的「上传到云端」和「绑定 QQ 群助手」：三个桌面宿主共用的命令。
//!
//! 上传的构建、节流和状态文件在 client-services（`typing_statistics::upload`），这里只把统计目录和账号会话接起来。设置应用没有常驻进程，所以不定时上传：页面在应用启动和打开统计页时调一次 [`upload_typing_statistics`]，距上次不到十分钟时什么也不做。打开开关时立刻上传一次；关掉时先在本地关掉，再删云端副本，删除失败也保持关闭，把错误交给页面说明。

use crate::platform::account_helpers::call_session_detailed;
use crate::platform::desktop::desktop_account::{AccountState, Session};
use crate::TypingStatisticsState;
use msime_client_services::account::{AccountError, AccountQqLink, AccountQqLinks};
use msime_client_services::typing_statistics::upload::{UploadError, UploadStatus};
use msime_client_services::typing_statistics::TypingStatisticsStore;
use std::sync::Arc;

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

/// 统计目录读写失败的码，和统计页其他命令的 `storage` 一致。
const STORAGE: &str = "storage";

/// 设置页读的上传状态：开关、上次上传的时间，以及本机是否登录了账号（没登录时开关不能打开）。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypingUploadResponse {
    enabled: bool,
    last_upload_ms: Option<u64>,
    signed_in: bool,
}

fn signed_in(session: &Session) -> Result<bool, &'static str> {
    session
        .status()
        .map(|user| user.is_some())
        .map_err(|error| error.code())
}

fn response(
    store: &TypingStatisticsStore,
    signed_in: bool,
) -> Result<TypingUploadResponse, &'static str> {
    let status = store
        .upload_state()
        .map(|state| UploadStatus::from(&state))
        .map_err(|_| STORAGE)?;
    Ok(TypingUploadResponse {
        enabled: status.enabled,
        last_upload_ms: status.last_upload_ms,
        signed_in,
    })
}

/// 打开了上传、已登录时上传最近几天；`force` 时不看十分钟的间隔。没有登录时不上传，也不报错：开关还开着，登录后下一次照常上传。
fn upload(
    store: &TypingStatisticsStore,
    session: &Session,
    force: bool,
) -> Result<TypingUploadResponse, &'static str> {
    let signed_in = signed_in(session)?;
    if signed_in {
        match store.upload_if_due(&today(), PLATFORM, now_ms(), force, |payload| {
            session.put_typing_statistics(payload)
        }) {
            Ok(_) => {}
            Err(UploadError::Send(failure)) => return Err(failure.code()),
            Err(UploadError::Storage(_)) => return Err(STORAGE),
        }
    }
    response(store, signed_in)
}

/// 在阻塞线程上跑一个上传命令，错误码原样交给页面。
async fn blocking<T, F>(operation: F) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, &'static str> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| crate::CommandError { code: STORAGE })?
        .map_err(|code| crate::CommandError { code })
}

#[tauri::command]
pub async fn typing_statistics_upload_status(
    statistics: tauri::State<'_, TypingStatisticsState>,
    account: tauri::State<'_, AccountState>,
) -> Result<TypingUploadResponse, crate::CommandError> {
    let store = statistics.0.clone();
    let session = Arc::clone(&account.session);
    blocking(move || response(&store, signed_in(&session)?)).await
}

/// 打开时要求已登录，并立刻上传一次；上传失败时开关保持打开，错误交给页面，下一次自动上传再试。关闭时先在本地关掉，再删除云端副本；删除失败时开关仍是关的，错误交给页面。
#[tauri::command]
pub async fn set_typing_statistics_upload(
    statistics: tauri::State<'_, TypingStatisticsState>,
    account: tauri::State<'_, AccountState>,
    enabled: bool,
) -> Result<TypingUploadResponse, crate::CommandError> {
    let store = statistics.0.clone();
    let session = Arc::clone(&account.session);
    blocking(move || {
        let _operation = store.lock_upload_operation();
        if enabled {
            if !signed_in(&session)? {
                return Err(AccountError::Unauthorized.code());
            }
            store.set_upload_enabled(true).map_err(|_| STORAGE)?;
            upload(&store, &session, true)
        } else {
            store.set_upload_enabled(false).map_err(|_| STORAGE)?;
            session
                .delete_typing_statistics()
                .map_err(|failure| failure.code())?;
            response(&store, signed_in(&session)?)
        }
    })
    .await
}

/// 页面在应用启动和打开统计页时调用；距上次上传不到十分钟时不发请求。
#[tauri::command]
pub async fn upload_typing_statistics(
    statistics: tauri::State<'_, TypingStatisticsState>,
    account: tauri::State<'_, AccountState>,
) -> Result<TypingUploadResponse, crate::CommandError> {
    let store = statistics.0.clone();
    let session = Arc::clone(&account.session);
    blocking(move || {
        let _operation = store.lock_upload_operation();
        upload(&store, &session, false)
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
