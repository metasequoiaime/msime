//! Community plugin-pack commands for the macOS, Windows and Linux shells.
//!
//! The page browses, rates and deletes publications by id only. Publishing and installing both happen host-side against the plugins root in [`PluginsState`]: the page names an installed pack by its kind and id and never supplies a path or a byte, a published archive is built from that pack by `msime_client_core::plugins::community::pack`, and a downloaded one is verified and installed through the same import a local `.zip` goes through. Pack failures come back as `PluginFailure`, with the rule client-core reports as its detail, and account failures as the `community_*` codes, so the page can name either precisely.

use crate::platform::account_helpers::{community_error, community_id, community_service_call};
use crate::platform::desktop::desktop_account::Storage;
use crate::platform::desktop::desktop_plugins::PluginsState;
use crate::CommandError;
use msime_client_core::account::BackendAccountClient;
use msime_client_core::plugins::community::{
    self, BackendCommunityPluginService, CommunityPlugin, CommunityPluginPage,
    CommunityPluginPublishRequest,
};
use msime_client_core::plugins::{PluginFailure, PluginKind, PluginSummary};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

pub(crate) type PluginCommunityService =
    BackendCommunityPluginService<BackendAccountClient, Storage>;

pub(crate) struct PluginCommunityState {
    pub(crate) service: Arc<PluginCommunityService>,
}

/// What the packer made of an installed pack before anything is uploaded, so the publish dialog can show a refusal, or the archive's size against the limit, before the user fills in the form.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommunityPackPreview {
    suggested_name: String,
    suggested_description: String,
    version: String,
    license: String,
    file_count: usize,
    size: usize,
}

#[derive(Serialize)]
pub struct PluginCommunityRatingResponse {
    stars: u8,
}

#[derive(Serialize)]
pub struct PluginCommunityDeleteResponse {
    deleted: bool,
}

/// An account or request failure in the vocabulary the pack commands answer with.
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
) -> Result<PluginCommunityPackPreview, PluginFailure> {
    let root = plugins.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        community::pack(&root, kind, &plugin_id).map(|packed| PluginCommunityPackPreview {
            size: packed.archive.len(),
            file_count: packed.file_count,
            version: packed.version,
            license: packed.license,
            suggested_name: packed.suggested_name,
            suggested_description: packed.suggested_description,
        })
    })
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
        let packed = community::pack(&root, kind, &plugin_id)?;
        let request = CommunityPluginPublishRequest::new(id, name, description, &packed);
        service
            .publish(&request)
            .map_err(|error| failure(community_error(error)))
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
        let download = service
            .download(id)
            .map_err(|error| failure(community_error(error)))?;
        community::install(&root, &download, kind, &plugin_id)
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
