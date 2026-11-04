//! 账号云词库请求的分发，三个桌面宿主共用一份。
//!
//! 这些操作只经过账号会话：列表、目录、增删改、导入导出、候选调频和固定排位。分发本身在 host-api 的 [`msime_host_api::cloud_dictionary::account`]，与 `msime_client_cloud_dictionary` 共用；这里只把它放到阻塞线程上跑，再把错误码包成 [`crate::CommandError`]。完整快照（下载预览、备份、恢复、应用到本机）要落盘，还要宿主自己的存储和输入法配合，由各宿主分别实现，这里对它们返回 `None`。

use msime_client_services::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
use msime_host_api::cloud_dictionary::account as dictionary_account;
use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
use serde_json::Value;
use std::sync::Arc;

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
