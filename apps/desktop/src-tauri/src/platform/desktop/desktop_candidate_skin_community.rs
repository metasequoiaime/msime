//! Community candidate-window skin commands for the macOS, Windows and Linux shells.
//!
//! The page browses, rates and unpublishes packages by publication id only. Publishing and installing both happen host-side against the skin root in [`crate::SkinDirectoryState`]: the page names an installed package by its folder id and never supplies a path or a byte, and an installed package is swapped into the root whole and the catalog rescanned, which on Linux is also what republishes `candidate_skin_catalog` to the IBus and Fcitx hosts. Package rule failures come back as their own `candidate_skin_*` codes and account failures as the `community_*` codes, so the page can name either precisely.

use crate::platform::account_helpers::{community_error, community_id, community_service_call};
use crate::platform::desktop::desktop_account::Storage;
use crate::{CommandError, RuntimeOptionsState, SkinCatalogResponse, SkinDirectoryState};
use msime_client_core::account::AccountError;
use msime_client_core::account::BackendAccountClient;
use msime_client_core::skin::candidate_community::{
    self, BackendCandidateSkinCommunityService, CandidateSkinCategory, CandidateSkinItem,
    CandidateSkinPackage, CandidateSkinPage, CandidateSkinVisibility,
};
use msime_client_core::skin::candidate_sync::{
    self, CandidateSkinPublishError, CandidateSkinSyncReport,
};
use msime_client_core::skin::catalog::SkinLicense;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Manager, State};

pub(crate) type CandidateSkinCommunityService =
    BackendCandidateSkinCommunityService<BackendAccountClient, Storage>;

pub(crate) struct CandidateSkinCommunityState {
    pub(crate) service: Arc<CandidateSkinCommunityService>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSkinPreviewResponse {
    data_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSkinPackPreview {
    suggested_name: String,
    license: SkinLicense,
    file_count: usize,
    size: usize,
}

#[derive(Serialize)]
pub struct CandidateSkinRatingResponse {
    stars: u8,
}

#[derive(Serialize)]
pub struct CandidateSkinUnpublishResponse {
    deleted: bool,
}

fn package_error(code: &'static str) -> CommandError {
    CommandError { code }
}

/// Where sync remembers what it last saw of the skin root `root`: a sibling of it, so the root itself holds only packages.
fn sync_state(root: &Path) -> PathBuf {
    root.with_file_name(candidate_sync::STATE_FILE)
}

/// Install a downloaded package into `root` and rescan it, so the page and (on Linux) the input method see the new list at once.
fn install_and_rescan(
    package: &CandidateSkinPackage,
    replace: bool,
    root: PathBuf,
    runtime: &RuntimeOptionsState,
) -> Result<SkinCatalogResponse, CommandError> {
    candidate_community::install(&root, package, replace).map_err(package_error)?;
    // Sync then knows where the package came from: someone else's publication stays out of the user's library.
    candidate_sync::record_install(&sync_state(&root), &package.package_id, package.id);
    Ok(crate::rescan_skin_catalog(root, runtime))
}

#[tauri::command]
pub async fn candidate_skin_community_list(
    state: State<'_, CandidateSkinCommunityState>,
    offset: usize,
    search: String,
    mine: bool,
    // 不传或为 null 时列出全部分类。
    category: Option<CandidateSkinCategory>,
) -> Result<CandidateSkinPage, CommandError> {
    community_service_call(Arc::clone(&state.service), move |service| {
        service.list(offset, &search, mine, category)
    })
    .await
}

#[tauri::command]
pub async fn candidate_skin_community_detail(
    state: State<'_, CandidateSkinCommunityState>,
    id: String,
) -> Result<CandidateSkinItem, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.detail(id)
    })
    .await
}

/// The preview as a `data:` URL, built only from an image whose type and leading bytes client-core has already checked.
#[tauri::command]
pub async fn candidate_skin_community_preview(
    state: State<'_, CandidateSkinCommunityState>,
    id: String,
) -> Result<CandidateSkinPreviewResponse, CommandError> {
    let id = community_id(&id)?;
    let preview = community_service_call(Arc::clone(&state.service), move |service| {
        service.preview(id)
    })
    .await?;
    Ok(CandidateSkinPreviewResponse {
        data_url: format!("data:{};base64,{}", preview.content_type, preview.data),
    })
}

#[tauri::command]
pub async fn candidate_skin_community_install(
    state: State<'_, CandidateSkinCommunityState>,
    directory: State<'_, SkinDirectoryState>,
    runtime: State<'_, RuntimeOptionsState>,
    id: String,
    replace: bool,
) -> Result<SkinCatalogResponse, CommandError> {
    let id = community_id(&id)?;
    let service = Arc::clone(&state.service);
    // The host chooses the root; the webview names a package only by its publication id.
    let root = directory.0.clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let package = service.download(id).map_err(community_error)?;
        install_and_rescan(&package, replace, root, &runtime)
    })
    .await
    .map_err(|_| CommandError {
        code: "community_unavailable",
    })?
}

/// Check an installed package against the sharing rules without uploading it, so the publish dialog can show a rejection before the user fills in the form.
#[tauri::command]
pub async fn candidate_skin_community_pack_preview(
    directory: State<'_, SkinDirectoryState>,
    skin_id: String,
    // A private package may leave out the asset license; none checks as public.
    visibility: Option<CandidateSkinVisibility>,
) -> Result<CandidateSkinPackPreview, CommandError> {
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        candidate_community::pack_as(&root, &skin_id, visibility.unwrap_or_default())
            .map(|packed| CandidateSkinPackPreview {
                suggested_name: packed.suggested_name,
                license: packed.license,
                file_count: packed.file_count,
                size: packed.size,
            })
            .map_err(package_error)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Save a preview the page drew for an installed package that has none, then rescan, so the package can be shared and every listing shows its new preview. The page sends only the image; the host decides where it goes and adds it to the manifest.
#[tauri::command]
pub async fn candidate_skin_community_add_preview(
    directory: State<'_, SkinDirectoryState>,
    runtime: State<'_, RuntimeOptionsState>,
    skin_id: String,
    bytes: Vec<u8>,
) -> Result<SkinCatalogResponse, CommandError> {
    let root = directory.0.clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        candidate_community::add_preview(&root, &skin_id, &bytes).map_err(package_error)?;
        Ok(crate::rescan_skin_catalog(root, &runtime))
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Write the asset license the user chose for an installed package that has none into its manifest, then rescan, so a public share is no longer refused for it and the user never edits skin.toml by hand.
#[tauri::command]
pub async fn candidate_skin_community_add_license(
    directory: State<'_, SkinDirectoryState>,
    runtime: State<'_, RuntimeOptionsState>,
    skin_id: String,
    assets: String,
) -> Result<SkinCatalogResponse, CommandError> {
    let root = directory.0.clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        candidate_community::add_license(&root, &skin_id, &assets).map_err(package_error)?;
        Ok(crate::rescan_skin_catalog(root, &runtime))
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
pub async fn candidate_skin_community_publish(
    state: State<'_, CandidateSkinCommunityState>,
    directory: State<'_, SkinDirectoryState>,
    skin_id: String,
    id: String,
    name: String,
    description: String,
    // None publishes publicly, as every page did before private packages.
    visibility: Option<CandidateSkinVisibility>,
    // 不传时不发送分类，由服务端归入默认分类。
    category: Option<CandidateSkinCategory>,
) -> Result<CandidateSkinItem, CommandError> {
    let id = community_id(&id)?;
    let service = Arc::clone(&state.service);
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        candidate_sync::publish(
            &root,
            &sync_state(&root),
            service.as_ref(),
            &skin_id,
            id,
            name,
            description,
            visibility.unwrap_or_default(),
            category,
        )
        .map_err(|error| match error {
            CandidateSkinPublishError::Package(code) => package_error(code),
            CandidateSkinPublishError::Account(error) => community_error(error),
        })
    })
    .await
    .map_err(|_| CommandError {
        code: "community_unavailable",
    })?
}

#[tauri::command]
pub async fn candidate_skin_community_rate(
    state: State<'_, CandidateSkinCommunityState>,
    id: String,
    stars: u8,
) -> Result<CandidateSkinRatingResponse, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.rate(id, stars)
    })
    .await?;
    Ok(CandidateSkinRatingResponse { stars })
}

#[tauri::command]
pub async fn candidate_skin_community_set_visibility(
    state: State<'_, CandidateSkinCommunityState>,
    id: String,
    visibility: CandidateSkinVisibility,
) -> Result<CandidateSkinItem, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.set_visibility(id, visibility)
    })
    .await
}

#[tauri::command]
pub async fn candidate_skin_community_set_category(
    state: State<'_, CandidateSkinCommunityState>,
    id: String,
    category: CandidateSkinCategory,
) -> Result<CandidateSkinItem, CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.set_category(id, category)
    })
    .await
}

/// Taking a package down ends its sync too: the local copy stays, and the next run uploads it again as a new private row rather than deleting it as removed from the library.
#[tauri::command]
pub async fn candidate_skin_community_unpublish(
    state: State<'_, CandidateSkinCommunityState>,
    directory: State<'_, SkinDirectoryState>,
    id: String,
) -> Result<CandidateSkinUnpublishResponse, CommandError> {
    let id = community_id(&id)?;
    let state_path = sync_state(&directory.0);
    community_service_call(Arc::clone(&state.service), move |service| {
        candidate_sync::unpublish(&state_path, service, id)
    })
    .await?;
    Ok(CandidateSkinUnpublishResponse { deleted: true })
}

/// Bring the skin root and the signed-in user's library in step, rescanning the root when the run changed it.
fn sync_and_rescan(
    service: &CandidateSkinCommunityService,
    root: PathBuf,
    runtime: &RuntimeOptionsState,
) -> Result<CandidateSkinSyncReport, AccountError> {
    let report = candidate_sync::sync_candidate_skins(&root, &sync_state(&root), service)?;
    if report.changed_local() {
        crate::rescan_skin_catalog(root, runtime);
    }
    Ok(report)
}

#[tauri::command]
pub async fn candidate_skin_community_sync(
    state: State<'_, CandidateSkinCommunityState>,
    directory: State<'_, SkinDirectoryState>,
    runtime: State<'_, RuntimeOptionsState>,
) -> Result<CandidateSkinSyncReport, CommandError> {
    let service = Arc::clone(&state.service);
    let root = directory.0.clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        sync_and_rescan(&service, root, &runtime).map_err(community_error)
    })
    .await
    .map_err(|_| CommandError {
        code: "community_unavailable",
    })?
}

/// One sync as the app starts, so skins made or changed while it was closed (by hand, or by the MCP server) reach the library without the community page being opened. Signed out it ends at the local session check, before any request.
pub(crate) fn start_sync(app: &tauri::AppHandle) {
    let service = Arc::clone(&app.state::<CandidateSkinCommunityState>().service);
    let root = app.state::<SkinDirectoryState>().0.clone();
    let runtime = app.state::<RuntimeOptionsState>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = sync_and_rescan(&service, root, &runtime);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];

    /// A package as another user would have published it: packed from a folder in a separate root.
    fn published_package() -> CandidateSkinPackage {
        let source = tempfile::tempdir().expect("temporary directory");
        let skin = source.path().join("sakura");
        std::fs::create_dir_all(&skin).unwrap();
        std::fs::write(
            skin.join("skin.toml"),
            "schema_version = 1\nid = 'sakura'\nname = '樱花'\nversion = '1.0'\nbase = 'paper'\npreview = 'preview.png'\n[supports]\nlayouts = ['vertical', 'horizontal']\nthemes = ['light', 'dark']\n[candidate_window]\nmin_width_dip = 10\n[candidate_window.decoration]\ntop_inset_dip = 0\nwidth_dip = 0\n[candidate.light]\nsurface = '#fff0f5'\n[license]\nassets = 'CC-BY-4.0'\n",
        )
        .unwrap();
        std::fs::write(skin.join("preview.png"), PNG).unwrap();
        let packed = candidate_community::pack(source.path(), "sakura").unwrap();
        CandidateSkinPackage {
            id: msime_client_core::uuid::Uuid::new_v4(),
            package_id: packed.package_id,
            manifest: packed.manifest,
            files: packed.files,
        }
    }

    #[test]
    fn install_lists_the_package_in_the_rescanned_catalog() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let skins = directory.path().join("skins");
        let path = directory.path().join("runtime-options.json");
        let document =
            serde_json::json!({"api_version": 1, "resources": "/resources", "preferences": {}});
        std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let runtime = RuntimeOptionsState {
            path: Some(path.clone()),
            document: Arc::new(Mutex::new(document)),
            skins: Some(skins.clone()),
        };
        let package = published_package();

        let response = install_and_rescan(&package, false, skins.clone(), &runtime).unwrap();
        assert_eq!(response.catalog.packages.len(), 1);
        assert_eq!(response.catalog.packages[0].id, "sakura");
        assert!(response.catalog.issues.is_empty());
        // The install is recorded beside the root, never inside it where the catalog would scan it.
        assert!(directory.path().join(candidate_sync::STATE_FILE).is_file());
        #[cfg(target_os = "linux")]
        {
            let published: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!(
                published["candidate_skin_catalog"]["packages"][0]["id"],
                "sakura"
            );
        }

        // Installing again without consent keeps the installed package and says why.
        let error = install_and_rescan(&package, false, skins.clone(), &runtime)
            .err()
            .expect("an existing package is not replaced without consent");
        assert_eq!(error.code, "candidate_skin_exists");
        let response = install_and_rescan(&package, true, skins, &runtime).unwrap();
        assert_eq!(response.catalog.packages.len(), 1);
    }
}
