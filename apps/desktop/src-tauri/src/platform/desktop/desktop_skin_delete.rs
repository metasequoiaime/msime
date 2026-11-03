//! 「主题」页皮肤卡片上的「删除」。页面只给文件夹名，皮肤根目录和偏好都由宿主决定；能不能删、删了以后同步怎么办都是 client-services 的 `candidate_sync::delete_unselected`，与 MCP 的 `delete_candidate_skin` 和 Windows 原生设置经 `msime_client_skin_delete` 走的是同一条规则。

use crate::{CommandError, RuntimeOptionsState, SkinCatalogResponse, SkinDirectoryState};
use msime_client_core::preferences::PreferencesStore;
use msime_client_services::skin::candidate_sync;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

/// 删掉以后重新扫描的目录，另带这款皮肤是否和账号皮肤库同步过：同步过的，页面说明私有副本会在下次同步时一起删掉。
#[derive(Serialize)]
pub struct SkinDeleteResponse {
    synced: bool,
    #[serde(flatten)]
    catalog: SkinCatalogResponse,
}

/// 删掉 `root` 下的皮肤 `id` 并重扫；Linux 上重扫也把新目录发布给 IBus 和 Fcitx5 宿主，与装皮肤之后相同。失败时的码原样交给页面：`candidate_skin_in_use`（偏好的槽位还放着它）、`candidate_skin_not_installed`、`candidate_skin_package`、`storage`。
pub(crate) fn delete_skin_at(
    root: &Path,
    preferences: &PreferencesStore,
    runtime: &RuntimeOptionsState,
    id: &str,
) -> Result<SkinDeleteResponse, CommandError> {
    let synced =
        candidate_sync::delete_unselected(root, &candidate_sync::state_path(root), preferences, id)
            .map_err(|code| CommandError { code })?;
    Ok(SkinDeleteResponse {
        synced,
        catalog: crate::rescan_skin_catalog(root.to_path_buf(), runtime),
    })
}

#[tauri::command]
pub async fn delete_skin(
    directory: State<'_, SkinDirectoryState>,
    store: State<'_, Arc<PreferencesStore>>,
    runtime: State<'_, RuntimeOptionsState>,
    id: String,
) -> Result<SkinDeleteResponse, CommandError> {
    let root = directory.0.clone();
    let store = Arc::clone(store.inner());
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || delete_skin_at(&root, &store, &runtime, &id))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::Mutex;

    fn write_skin(root: &Path, id: &str) {
        let package = root.join(id);
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("skin.toml"),
            format!("schema_version = 1\nid = '{id}'\nname = '{id}'\nversion = '1.0'\nbase = 'system'\n[supports]\nlayouts = ['vertical']\nthemes = ['light']\n[candidate_window]\nmin_width_dip = 10\n"),
        )
        .unwrap();
    }

    #[test]
    fn the_settings_page_deletes_an_unselected_skin_and_gets_the_rescanned_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("skins");
        write_skin(&root, "sakura");
        write_skin(&root, "dusk");
        let store = PreferencesStore::new(directory.path());
        let current = store.load().unwrap();
        let mut preferences = current.preferences;
        preferences.custom_theme.candidate_skin = Some("dusk".into());
        store.save(current.revision, preferences).unwrap();
        let runtime = RuntimeOptionsState {
            path: None,
            document: Arc::new(Mutex::new(Value::Null)),
            skins: None,
        };

        // 槽位里的那款原样留着，码交给页面。
        assert_eq!(
            delete_skin_at(&root, &store, &runtime, "dusk")
                .err()
                .map(|error| error.code),
            Some("candidate_skin_in_use")
        );
        let response = delete_skin_at(&root, &store, &runtime, "sakura").unwrap();
        assert!(!response.synced);
        let listed: Vec<&str> = response
            .catalog
            .catalog
            .packages
            .iter()
            .map(|package| package.id.as_str())
            .collect();
        assert_eq!(listed, ["dusk"]);
        assert!(root.join("dusk/skin.toml").is_file());
    }
}
