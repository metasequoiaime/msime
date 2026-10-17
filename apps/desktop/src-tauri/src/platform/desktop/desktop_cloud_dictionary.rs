//! 桌面设置应用经共享账号会话访问云词库。
//!
//! Windows 的面板和 macOS 缺少原生会话或 provider 的面板由设置应用用共享账号会话完成请求：列表、目录、增删改、导入导出、候选调频和固定排位走与 iOS、Android 共用的 [`cloud_dictionary::account_request`]，完整快照（下载预览、应用到本机、备份与恢复）在 host-api 的 [`msime_host_api::cloud_dictionary::full_backup`]，与 `msime_client_cloud_dictionary` 共用；这里只把它放到阻塞线程上跑，再把错误码包成 [`crate::CommandError`]。

use crate::platform::cloud_dictionary::{self, snapshot_command_error};
use msime_client_services::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
use msime_host_api::cloud_dictionary::full_backup;
use msime_host_api::cloud_dictionary::CloudDictionaryRequest;
use serde_json::Value;
use std::sync::Arc;

/// 预览表和重试节拍，见 host-api。复制出来的值共用同一张表。
pub(crate) use full_backup::CloudDictionaryState;

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
    let session = Arc::clone(session);
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || {
        full_backup::request(&session, &state, options, request)
    })
    .await
    .map_err(|_| snapshot_command_error())?
    .map_err(|code| crate::CommandError { code })
}
