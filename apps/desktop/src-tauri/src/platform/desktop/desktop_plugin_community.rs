//! macOS、Windows 和 Linux 外壳的社区插件包命令。
//!
//! 页面只按 id 浏览、评分和删除发布。发布和安装都在宿主端针对 [`PluginsState`] 里的插件根目录进行：页面只按类型和 id 指定一个已安装的包，从不提供路径或任何字节；发布的归档由 `msime_client_services::plugins::community::pack` 从该包构建，下载的归档经过校验后，走与本地 `.zip` 相同的导入流程安装。包的失败以 `PluginFailure` 返回，其 detail 是 client-services 报告的规则；账号失败以 `community_*` 代码返回，因此页面能准确说出是哪一种。

use crate::platform::account_helpers::{community_id, community_service_call};
use crate::platform::desktop::desktop_account::Storage;
use crate::platform::desktop::desktop_plugins::PluginsState;
use crate::CommandError;
use msime_client_services::account::BackendAccountClient;
use msime_client_services::plugins::community::{
    self, BackendCommunityPluginService, CommunityPlugin, CommunityPluginPackPreview,
    CommunityPluginPage,
};
use msime_client_services::plugins::{PluginFailure, PluginKind, PluginSummary};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

pub(crate) type PluginCommunityService =
    BackendCommunityPluginService<BackendAccountClient, Storage>;

pub(crate) struct PluginCommunityState {
    pub(crate) service: Arc<PluginCommunityService>,
}

#[derive(Serialize)]
pub struct PluginCommunityRatingResponse {
    stars: u8,
}

#[derive(Serialize)]
pub struct PluginCommunityDeleteResponse {
    deleted: bool,
}

/// A request failure in the vocabulary the pack commands answer with.
fn failure(error: CommandError) -> PluginFailure {
    PluginFailure::code(error.code)
}

#[tauri::command]
pub async fn plugin_community_list(
    state: State<'_, PluginCommunityState>,
    offset: usize,
    search: String,
    kind: Option<PluginKind>,
    mine: Option<bool>,
) -> Result<CommunityPluginPage, CommandError> {
    community_service_call(Arc::clone(&state.service), move |service| {
        service.list(offset, &search, kind, mine.unwrap_or(false))
    })
    .await
}

#[tauri::command]
pub async fn plugin_community_detail(
    state: State<'_, PluginCommunityState>,
    id: String,
) -> Result<CommunityPlugin, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.detail(id)
    })
    .await
}

/// Check an installed pack against the sharing rules without uploading it.
#[tauri::command]
pub async fn plugin_community_pack_preview(
    plugins: State<'_, PluginsState>,
    kind: PluginKind,
    plugin_id: String,
) -> Result<CommunityPluginPackPreview, PluginFailure> {
    let root = plugins.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || community::pack_preview(&root, kind, &plugin_id))
        .await
        .map_err(|_| PluginFailure::code("storage"))?
}

/// Pack the installed pack `plugin_id` of `kind` and publish it under the page's publication id `id`. The archive is deterministic, so a retry with the same id after a lost response carries identical content and the server answers with the item it already stored.
#[tauri::command]
pub async fn plugin_community_publish(
    state: State<'_, PluginCommunityState>,
    plugins: State<'_, PluginsState>,
    kind: PluginKind,
    plugin_id: String,
    id: String,
    name: String,
    description: String,
) -> Result<CommunityPlugin, PluginFailure> {
    let id = community_id(&id).map_err(failure)?;
    let service = Arc::clone(&state.service);
    let root = plugins.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        service.publish_installed(&root, kind, &plugin_id, id, name, description)
    })
    .await
    .map_err(|_| PluginFailure::code("community_unavailable"))?
}

/// Download a publication and install it under the plugins root, replacing an installed pack of the same kind and id whole; the page asks before that happens. `kind` and `plugin_id` are the listing's, the pack the page asked about, and a download naming any other pack is refused.
#[tauri::command]
pub async fn plugin_community_install(
    state: State<'_, PluginCommunityState>,
    plugins: State<'_, PluginsState>,
    id: String,
    kind: PluginKind,
    plugin_id: String,
) -> Result<PluginSummary, PluginFailure> {
    let id = community_id(&id).map_err(failure)?;
    let service = Arc::clone(&state.service);
    // The host chooses the root; the webview names a pack only by its publication id.
    let root = plugins.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        service.install_publication(&root, id, kind, &plugin_id)
    })
    .await
    .map_err(|_| PluginFailure::code("community_unavailable"))?
}

#[tauri::command]
pub async fn plugin_community_rate(
    state: State<'_, PluginCommunityState>,
    id: String,
    stars: u8,
) -> Result<PluginCommunityRatingResponse, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.rate(id, stars)
    })
    .await?;
    Ok(PluginCommunityRatingResponse { stars })
}

/// Take down one of the user's own publications. The installed pack stays.
#[tauri::command]
pub async fn plugin_community_delete(
    state: State<'_, PluginCommunityState>,
    id: String,
) -> Result<PluginCommunityDeleteResponse, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.delete(id)
    })
    .await?;
    Ok(PluginCommunityDeleteResponse { deleted: true })
}
