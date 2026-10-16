//! 账号云词库请求的分发，iOS、Android 和 Windows 共用一份。
//!
//! 这些操作只经过账号会话：列表、目录、增删改、导入导出、候选调频和固定排位。分发本身在 host-api 的 [`msime_host_api::cloud_dictionary::account`]，与 `msime_client_cloud_dictionary` 共用；这里只把它放到阻塞线程上跑，再把错误码包成 [`crate::CommandError`]。完整快照（下载预览、备份、恢复、应用到本机）要落盘，还要宿主自己的存储和输入法配合，由各宿主分别实现，这里对它们返回 `None`。

use msime_client_services::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
use msime_host_api::cloud_dictionary::account as dictionary_account;
use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
use serde_json::Value;
use std::sync::Arc;

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

/// 用账号会话处理一条云词库请求。快照类请求留给调用方，返回 `None`。
pub(crate) async fn account_request<A: AccountApi, S: AccountSessionStorage>(
    session: &Arc<BackendAccountSession<A, S>>,
    request: &CloudDictionaryRequest,
) -> Option<Result<Value, crate::CommandError>> {
    // 先判断再复制：快照请求里可能带着整份快照的文本。
    if !dictionary_account::handled_by_account(request) {
        return None;
    }
    let session = Arc::clone(session);
    let request = request.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        dictionary_account::account_request(&session, &request)
    })
    .await;
    Some(match result {
        Ok(Some(result)) => result.map_err(|code| crate::CommandError { code }),
        // 上面已经排除了快照请求，`None` 不会出现；线程没能跑完时与原来一样报账号服务不可用。
        Ok(None) | Err(_) => Err(crate::CommandError {
            code: "account_unavailable",
        }),
    })
}

pub(crate) fn snapshot_command_error() -> crate::CommandError {
    crate::CommandError {
        code: "snapshot_unavailable",
    }
}

/// 快照队列的状态里带着账号 id，交给页面之前去掉。只有 iOS 和 Android 用；Windows 的完整快照在 host-api 的 `cloud_dictionary::full_backup`，那里自己去掉。
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub(crate) fn snapshot_response_without_account(
    mut value: Value,
) -> Result<Value, crate::CommandError> {
    let object = value.as_object_mut().ok_or_else(snapshot_command_error)?;
    if let Some(request) = object.get_mut("request").and_then(Value::as_object_mut) {
        request.remove("accountId");
    }
    Ok(value)
}
