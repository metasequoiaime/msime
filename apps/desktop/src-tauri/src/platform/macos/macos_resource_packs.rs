//! macOS 按需下载的资源包：日文词典、粤拼/注音词库和手写模型不再打进发布包，由设置页在用户选用对应方案或第一次打开手写面板时下载。
//!
//! 资源包装在 `<state_root>/resource-packs/<id>/`，state_root 就是偏好目录（`PreferencesStore::directory`，macOS 上即 `macos_launch` 解析出的 preferences_directory），也是 host-api 查找已下载资源包的同一个目录。下载、校验和整体发布由 `msime_client_core::resource_packs` 完成；这里只负责在命令线程之外运行它、把进度作为 `resource-pack-progress` 事件发给页面，并与语音模型共用 `LocalModelInstalls` 的取消和互斥登记。

use crate::voice::local_models::{run_install, saved_model_mirror, LocalModelInstalls};
use crate::{HostActionError, PreferencesStore};
use msime_client_core::preferences::{ChineseScheme, InputScheme};
use msime_client_core::resource_packs::{self, PackState, ResourcePack, ResourcePackStatus};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

/// 安装进行中以 `LocalModelProgress` 为载荷发出的事件，`id` 是资源包 id。
pub(crate) const RESOURCE_PACK_PROGRESS_EVENT: &str = "resource-pack-progress";

/// 资源包所在的 state_root：偏好目录。
fn state_root(store: &PreferencesStore) -> PathBuf {
    store.directory().to_path_buf()
}

/// 在共享的 `LocalModelInstalls` 里登记用的 key，加前缀以免与语音模型 id 冲突。
fn install_key(pack: ResourcePack) -> String {
    format!("resource-pack:{}", pack.id())
}

fn known_pack(id: &str) -> Result<ResourcePack, HostActionError> {
    ResourcePack::from_id(id).ok_or(HostActionError {
        code: "local_model_unknown",
    })
}

/// 下载、校验并发布一个资源包，返回安装目录。命令和启动时的自动补齐都走这里，所以页面能看到同样的 busy 状态和进度。
async fn install_pack<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    installs: &LocalModelInstalls,
    store: Arc<PreferencesStore>,
    pack: ResourcePack,
) -> Result<PathBuf, HostActionError> {
    let root = state_root(&store);
    let mirror = saved_model_mirror(store).await?;
    run_install(
        app,
        installs,
        &install_key(pack),
        RESOURCE_PACK_PROGRESS_EVENT,
        pack.id().to_owned(),
        move |progress, cancel| resource_packs::install(&root, pack, &mirror, progress, cancel),
    )
    .await
}

/// 每个资源包的安装状态、下载大小和对应的输入方案。
#[tauri::command]
pub(crate) async fn resource_packs(
    store: tauri::State<'_, Arc<PreferencesStore>>,
) -> Result<Vec<ResourcePackStatus>, HostActionError> {
    let root = state_root(store.inner());
    tauri::async_runtime::spawn_blocking(move || resource_packs::list(&root))
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })
}

/// 下载并安装一个资源包，返回安装目录。镜像沿用已保存的 `voice_input.asr_model_mirror`。
#[tauri::command]
pub(crate) async fn resource_pack_install<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    installs: tauri::State<'_, LocalModelInstalls>,
    store: tauri::State<'_, Arc<PreferencesStore>>,
    id: String,
) -> Result<String, HostActionError> {
    let pack = known_pack(&id)?;
    let path = install_pack(&app, &installs, store.inner().clone(), pack).await?;
    Ok(path.to_string_lossy().into_owned())
}

/// 停止正在进行的资源包安装，返回是否有安装在跑；安装命令随后以 `local_model_cancelled` 失败。
#[tauri::command]
pub(crate) fn resource_pack_cancel(
    installs: tauri::State<'_, LocalModelInstalls>,
    id: String,
) -> Result<bool, HostActionError> {
    let pack = known_pack(&id)?;
    Ok(installs.cancel(&install_key(pack)))
}

/// 已保存的方案需要的资源包：当前方案是日文时要日文词典；当前方案或上次使用的中文方案是粤拼/注音/笔画时要语言词库。手写不对应方案，临时日语也不触发下载。
fn needed_packs(
    scheme: InputScheme,
    last_chinese_scheme: Option<ChineseScheme>,
) -> Vec<ResourcePack> {
    let mut needed = Vec::with_capacity(2);
    if scheme == InputScheme::Japanese {
        needed.push(ResourcePack::Japanese);
    }
    let language = |scheme: InputScheme| {
        matches!(
            scheme,
            InputScheme::Cantonese | InputScheme::Zhuyin | InputScheme::Stroke
        )
    };
    if language(scheme) || last_chinese_scheme.is_some_and(|last| language(last.into())) {
        needed.push(ResourcePack::LanguageDictionaries);
    }
    needed
}

/// 启动时补齐已保存方案需要的资源包，用于从内置这些资源的旧版本升级上来、或安装被中断的情况。在后台运行一次；失败只记录固定的消息，不含路径，用户之后仍可在设置页手动下载。
pub(crate) fn ensure_saved_scheme_packs<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let store = app.state::<Arc<PreferencesStore>>().inner().clone();
        let loaded = {
            let store = store.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let preferences = store.load().ok()?.preferences;
                let root = state_root(&store);
                Some((
                    needed_packs(preferences.scheme, preferences.last_chinese_scheme),
                    resource_packs::list(&root),
                ))
            })
            .await
        };
        let Ok(Some((needed, statuses))) = loaded else {
            eprintln!("msime: resource packs: cannot read preferences");
            return;
        };
        let installs = app.state::<LocalModelInstalls>();
        for pack in needed {
            let state = statuses
                .iter()
                .find(|status| status.id == pack.id())
                .map(|status| status.state);
            if state == Some(PackState::Installed) {
                continue;
            }
            if let Err(error) = install_pack(&app, &installs, store.clone(), pack).await {
                eprintln!(
                    "msime: resource pack {} was not installed: {}",
                    pack.id(),
                    error.code
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_pack_needs_follow_the_saved_schemes() {
        use ChineseScheme as Last;
        use InputScheme as Scheme;
        assert_eq!(needed_packs(Scheme::Quanpin, None), []);
        assert_eq!(needed_packs(Scheme::Quanpin, Some(Last::Wubi)), []);
        assert_eq!(needed_packs(Scheme::Korean, None), []);
        assert_eq!(
            needed_packs(Scheme::Japanese, None),
            [ResourcePack::Japanese]
        );
        assert_eq!(
            needed_packs(Scheme::Cantonese, None),
            [ResourcePack::LanguageDictionaries]
        );
        assert_eq!(
            needed_packs(Scheme::Zhuyin, Some(Last::Zhuyin)),
            [ResourcePack::LanguageDictionaries]
        );
        // 当前是日文、上次的中文方案是粤拼：切回中文时也要用到语言词库，两个都要。
        let both = needed_packs(Scheme::Japanese, Some(Last::Cantonese));
        assert_eq!(
            both,
            [ResourcePack::Japanese, ResourcePack::LanguageDictionaries]
        );
        assert_eq!(both.capacity(), 2);
        assert_eq!(
            needed_packs(Scheme::Quanpin, Some(Last::Zhuyin)),
            [ResourcePack::LanguageDictionaries]
        );
        // 笔画也读语言词库里的 stroke.db。
        assert_eq!(
            needed_packs(Scheme::Stroke, None),
            [ResourcePack::LanguageDictionaries]
        );
        assert_eq!(
            needed_packs(Scheme::Korean, Some(Last::Stroke)),
            [ResourcePack::LanguageDictionaries]
        );
    }

    /// macOS 发布包对每个版本都不内置日文词典。水杉日语只有日文一个方案，用户不会去切换方案：它的状态目录第一次准备好时偏好就是日文（`Preferences::for_edition`），设置应用第一次启动就由这里补下词典。越南文、藏文版不需要任何资源包。
    #[test]
    fn single_language_editions_fetch_exactly_their_own_packs_on_first_launch() {
        use msime_client_core::edition::Edition;
        use msime_client_core::preferences::Preferences;
        let first_launch = |id: &str| {
            let preferences = Preferences::for_edition(Edition::by_id(id).unwrap());
            needed_packs(preferences.scheme, preferences.last_chinese_scheme)
        };
        assert_eq!(first_launch("japanese"), [ResourcePack::Japanese]);
        assert_eq!(first_launch("vietnamese"), []);
        assert_eq!(first_launch("tibetan"), []);
    }

    #[test]
    fn resource_pack_install_keys_never_collide_with_voice_model_ids() {
        for pack in ResourcePack::ALL {
            let key = install_key(pack);
            assert!(key.starts_with("resource-pack:"));
            assert!(!msime_client_core::is_bounded_ascii_identifier(&key, 128));
        }
        assert!(known_pack("japanese").is_ok());
        assert_eq!(known_pack("voice").unwrap_err().code, "local_model_unknown");
    }
}
