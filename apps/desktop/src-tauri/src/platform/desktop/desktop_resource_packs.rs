//! 桌面按需下载的资源包。macOS 发布包不再内置日文词典和粤拼/注音/笔画词库，由设置页在用户选用对应方案时下载；手写模型在第一次打开手写面板时下载（Windows 只在 Ink 没有中文识别器、Linux 只在安装前缀里没有随包模型时才需要）；桌面落定重排模型在打开「桌面神经联想」时下载，三个桌面平台都是这样。
//!
//! 资源包装在 `<state_root>/resource-packs/<id>/`，state_root 就是偏好目录（`PreferencesStore::directory`：macOS 上是 `macos_launch` 解析出的 preferences_directory，Windows 上是 Server 的 DataDir，Linux 上是运行时选项里的状态目录），也是 host-api 查找已下载资源包的同一个目录。下载、校验和整体发布由 `msime_client_core::resource_packs` 完成；这里只负责决定本机提供哪些资源包、在命令线程之外运行安装、把进度作为 `resource-pack-progress` 事件发给页面，并与语音模型共用 `LocalModelInstalls` 的取消和互斥登记。

use crate::voice::local_models::{run_install, saved_model_mirror, LocalModelInstalls};
use crate::{DictionaryHostOptions, HostActionError, PreferencesStore};
use msime_client_core::edition::Edition;
use msime_client_core::preferences::{ChineseScheme, InputScheme, Preferences};
use msime_client_core::resource_packs::{self, PackState, ResourcePack, ResourcePackStatus};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::Manager;

/// 安装进行中以 `LocalModelProgress` 为载荷发出的事件，`id` 是资源包 id。
pub(crate) const RESOURCE_PACK_PROGRESS_EVENT: &str = "resource-pack-progress";

/// 运行设置应用的桌面平台。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DesktopPlatform {
    Macos,
    Windows,
    Linux,
}

impl DesktopPlatform {
    /// 本次构建的平台。用 `cfg!` 而不是三个 `#[cfg]` 常量，三个分支在每个平台上都参与编译。
    fn current() -> DesktopPlatform {
        if cfg!(target_os = "macos") {
            DesktopPlatform::Macos
        } else if cfg!(target_os = "windows") {
            DesktopPlatform::Windows
        } else {
            DesktopPlatform::Linux
        }
    }
}

/// 决定本机提供哪些资源包的安装情况。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Installation {
    pub(crate) platform: DesktopPlatform,
    /// 不靠资源包也有手写模型：HostOptions 或环境变量指定了一个，或者安装布局里随包带着。
    pub(crate) handwriting_model: bool,
    /// Windows Ink 装有中文手写识别器；其他平台总是 false。
    pub(crate) ink_chinese_recognizer: bool,
    /// 安装布局里随包带着落定重排模型。
    pub(crate) settled_model: bool,
    /// 安装布局里随包带齐了粤拼、注音和笔画词库。
    pub(crate) language_dictionaries: bool,
}

impl Installation {
    /// 读本机的安装情况。在 Windows 上会枚举 Ink 识别器，要在阻塞线程上调用。
    fn current(host_options: Option<&Value>) -> Installation {
        Installation {
            platform: DesktopPlatform::current(),
            handwriting_model: crate::handwriting_model_without_pack(host_options),
            ink_chinese_recognizer: ink_chinese_recognizer(),
            settled_model: host_options
                .and_then(msime_host_api::packaged_settled_model)
                .is_some(),
            // 还没有运行时选项（Linux 首次运行）时不知道装了什么，按带齐处理，与设置页可选方案的判断一致。
            language_dictionaries: host_options
                .is_none_or(msime_host_api::packaged_language_dictionaries),
        }
    }
}

#[cfg(target_os = "windows")]
fn ink_chinese_recognizer() -> bool {
    msime_host_windows::ink::has_chinese_recognizer()
}

#[cfg(not(target_os = "windows"))]
fn ink_chinese_recognizer() -> bool {
    false
}

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

/// 版本表里中文主词库的组件名。
const CHINESE_MAIN_COMPONENT: &str = "chinese-main";

/// 本机是否提供这个资源包。
///
/// - 日文词典和语言词库只有 macOS 发布包不内置，macOS 上总是列出。Windows 随包带着它们，不列出也不下载。Linux 随包带着日文词典；语言词库只在用得到它的版本（版本表 `language_dictionaries` 非空）没有随包带齐时才列出，例如打包时没有备好词库、或不带它们的 Nix 包，否则粤拼、注音和笔画在这台机器上无从启用。
/// - 手写模型只认汉字，不提供手写的版本（版本表 `features.handwriting` 为 false：日文、越南文和藏文版）不列出也不下载它。macOS 上提供手写的版本都列出它；Windows 上手写面板先用 Ink，只有 Ink 没有中文识别器、且没有随包或指定的模型时才列出；Linux 上没有随包或指定的模型时才列出。没列出时手写面板照常识别，不等下载。
/// - 落定重排模型只给中文整句重排，不带中文主词库（版本表 `resources.components` 没有 `chinese-main`：日文、越南文和藏文版）的版本不列出也不下载它，与打包时不装它的规则相同。其他版本在没有随包模型时列出：会话总是优先用随包的那份，有它时下载的用不上。
fn offered_by(pack: ResourcePack, edition: &Edition, installation: &Installation) -> bool {
    match pack {
        ResourcePack::Japanese => installation.platform == DesktopPlatform::Macos,
        ResourcePack::LanguageDictionaries => match installation.platform {
            DesktopPlatform::Macos => true,
            DesktopPlatform::Windows => false,
            DesktopPlatform::Linux => {
                !edition.language_dictionaries.is_empty() && !installation.language_dictionaries
            }
        },
        ResourcePack::Handwriting => {
            edition.features.handwriting
                && match installation.platform {
                    DesktopPlatform::Macos => true,
                    DesktopPlatform::Windows => {
                        !installation.ink_chinese_recognizer && !installation.handwriting_model
                    }
                    DesktopPlatform::Linux => !installation.handwriting_model,
                }
        }
        ResourcePack::SettledModel => {
            edition
                .resources
                .components
                .iter()
                .any(|component| component == CHINESE_MAIN_COMPONENT)
                && !installation.settled_model
        }
    }
}

/// HostOptions 文档；还没准备好运行时选项（Linux 首次运行）时为 `None`。
fn host_options<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<Value> {
    app.try_state::<DictionaryHostOptions>()
        .and_then(|options| options.snapshot().ok())
}

/// 在阻塞线程上读本机的安装情况。
async fn installation<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Installation, HostActionError> {
    let document = host_options(app);
    tauri::async_runtime::spawn_blocking(move || Installation::current(document.as_ref()))
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
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

/// 本机提供的每个资源包的安装状态、下载大小和对应的输入方案。
#[tauri::command]
pub(crate) async fn resource_packs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    store: tauri::State<'_, Arc<PreferencesStore>>,
) -> Result<Vec<ResourcePackStatus>, HostActionError> {
    let root = state_root(store.inner());
    let edition = crate::package_edition();
    let document = host_options(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let installation = Installation::current(document.as_ref());
        let mut statuses = resource_packs::list(&root);
        statuses.retain(|status| {
            ResourcePack::from_id(status.id)
                .is_some_and(|pack| offered_by(pack, edition, &installation))
        });
        statuses
    })
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
    if !offered_by(pack, crate::package_edition(), &installation(&app).await?) {
        return Err(HostActionError {
            code: "unavailable",
        });
    }
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
    let mut needed = Vec::with_capacity(3);
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

/// 已保存的偏好需要的资源包：方案需要的词库（[`needed_packs`]），打开了桌面神经联想时还有落定重排模型。
fn needed_by_preferences(preferences: &Preferences) -> Vec<ResourcePack> {
    let mut needed = needed_packs(preferences.scheme, preferences.last_chinese_scheme);
    if preferences.sentence_association.neural_desktop {
        needed.push(ResourcePack::SettledModel);
    }
    needed
}

/// 启动时补齐已保存偏好需要、本机也提供的资源包，用于从内置这些资源的旧版本升级上来、或安装被中断的情况。在后台运行一次；失败只记录固定的消息，不含路径，用户之后仍可在设置页手动下载。
pub(crate) fn ensure_saved_packs<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let store = app.state::<Arc<PreferencesStore>>().inner().clone();
        let edition = crate::package_edition();
        let document = host_options(&app);
        let loaded = {
            let store = store.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let preferences = store.load().ok()?.preferences;
                let installation = Installation::current(document.as_ref());
                let root = state_root(&store);
                let mut needed = needed_by_preferences(&preferences);
                needed.retain(|pack| offered_by(*pack, edition, &installation));
                Some((needed, resource_packs::list(&root)))
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

    fn installation(platform: DesktopPlatform) -> Installation {
        Installation {
            platform,
            handwriting_model: false,
            ink_chinese_recognizer: false,
            settled_model: false,
            language_dictionaries: true,
        }
    }

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
        assert_eq!(
            needed_packs(Scheme::Quanpin, Some(Last::Zhuyin)),
            [ResourcePack::LanguageDictionaries]
        );
        // 笔画也读语言词库里的 msime-stroke.db。
        assert_eq!(
            needed_packs(Scheme::Stroke, None),
            [ResourcePack::LanguageDictionaries]
        );
        assert_eq!(
            needed_packs(Scheme::Korean, Some(Last::Stroke)),
            [ResourcePack::LanguageDictionaries]
        );
    }

    /// 打开桌面神经联想时，启动补齐落定重排模型；关着时不下载。
    #[test]
    fn the_desktop_neural_switch_needs_the_settled_model() {
        let mut preferences = Preferences::default();
        preferences.sentence_association.neural_desktop = false;
        assert!(!needed_by_preferences(&preferences).contains(&ResourcePack::SettledModel));
        preferences.sentence_association.neural_desktop = true;
        assert_eq!(
            needed_by_preferences(&preferences),
            [ResourcePack::SettledModel]
        );
        preferences.scheme = InputScheme::Japanese;
        assert_eq!(
            needed_by_preferences(&preferences),
            [ResourcePack::Japanese, ResourcePack::SettledModel]
        );
    }

    /// macOS 发布包对每个版本都不内置日文词典。水杉日语只有日文一个方案，用户不会去切换方案：它的状态目录第一次准备好时偏好就是日文（`Preferences::for_edition`），设置应用第一次启动就由这里补下词典。越南文、藏文版不需要任何资源包。
    #[test]
    fn single_language_editions_fetch_exactly_their_own_packs_on_first_launch() {
        let first_launch = |id: &str| {
            let preferences = Preferences::for_edition(Edition::by_id(id).unwrap());
            needed_packs(preferences.scheme, preferences.last_chinese_scheme)
        };
        assert_eq!(first_launch("japanese"), [ResourcePack::Japanese]);
        assert_eq!(first_launch("vietnamese"), []);
        assert_eq!(first_launch("tibetan"), []);
    }

    /// 手写模型只给提供手写的版本：日文、越南文和藏文版不列出、不下载它，日文词典和语言词库不受影响。
    #[test]
    fn only_editions_with_handwriting_offer_the_handwriting_pack() {
        let macos = installation(DesktopPlatform::Macos);
        for edition in Edition::all() {
            assert_eq!(
                offered_by(ResourcePack::Handwriting, edition, &macos),
                edition.features.handwriting,
                "{}",
                edition.id
            );
            assert!(
                offered_by(ResourcePack::Japanese, edition, &macos),
                "{}",
                edition.id
            );
            assert!(
                offered_by(ResourcePack::LanguageDictionaries, edition, &macos),
                "{}",
                edition.id
            );
        }
        for id in ["japanese", "vietnamese", "tibetan"] {
            for platform in [
                DesktopPlatform::Macos,
                DesktopPlatform::Windows,
                DesktopPlatform::Linux,
            ] {
                assert!(
                    !offered_by(
                        ResourcePack::Handwriting,
                        Edition::by_id(id).unwrap(),
                        &installation(platform)
                    ),
                    "{id}"
                );
            }
        }
        assert!(offered_by(
            ResourcePack::Handwriting,
            Edition::full(),
            &macos
        ));
    }

    /// macOS 的提供规则与按需下载上线时一致：随包的旧手写模型不影响列出手写资源包。
    #[test]
    fn macos_offers_every_pack_it_offered_before() {
        let mut macos = installation(DesktopPlatform::Macos);
        macos.handwriting_model = true;
        for pack in [
            ResourcePack::Japanese,
            ResourcePack::LanguageDictionaries,
            ResourcePack::Handwriting,
        ] {
            assert!(offered_by(pack, Edition::full(), &macos), "{}", pack.id());
        }
    }

    /// Windows 和 Linux 随包带着日文词典和语言词库时，从不下载它们。
    #[test]
    fn only_macos_downloads_dictionaries() {
        for platform in [DesktopPlatform::Windows, DesktopPlatform::Linux] {
            for pack in [ResourcePack::Japanese, ResourcePack::LanguageDictionaries] {
                assert!(
                    !offered_by(pack, Edition::full(), &installation(platform)),
                    "{platform:?} {}",
                    pack.id()
                );
            }
        }
    }

    /// Linux 安装没有随包带齐语言词库时，用得到它们的版本列出语言词库资源包，否则粤拼、注音和笔画无从启用；日文词典照旧随包，不列出。Windows 不受影响。
    #[test]
    fn linux_offers_language_dictionaries_only_when_the_package_lacks_them() {
        let mut linux = installation(DesktopPlatform::Linux);
        linux.language_dictionaries = false;
        for edition in Edition::all() {
            assert_eq!(
                offered_by(ResourcePack::LanguageDictionaries, edition, &linux),
                !edition.language_dictionaries.is_empty(),
                "{}",
                edition.id
            );
            assert!(
                !offered_by(ResourcePack::Japanese, edition, &linux),
                "{}",
                edition.id
            );
        }
        assert!(offered_by(
            ResourcePack::LanguageDictionaries,
            Edition::full(),
            &linux
        ));
        let mut windows = installation(DesktopPlatform::Windows);
        windows.language_dictionaries = false;
        assert!(!offered_by(
            ResourcePack::LanguageDictionaries,
            Edition::full(),
            &windows
        ));
    }

    /// Windows 上 Ink 有中文识别器时手写面板直接用它，不列出手写模型，面板也就不会等下载；Ink 没有、也没有随包模型时才列出。
    #[test]
    fn windows_offers_the_handwriting_model_only_without_a_chinese_ink_recognizer() {
        let mut windows = installation(DesktopPlatform::Windows);
        assert!(offered_by(
            ResourcePack::Handwriting,
            Edition::full(),
            &windows
        ));
        windows.ink_chinese_recognizer = true;
        assert!(!offered_by(
            ResourcePack::Handwriting,
            Edition::full(),
            &windows
        ));
        windows.ink_chinese_recognizer = false;
        windows.handwriting_model = true;
        assert!(!offered_by(
            ResourcePack::Handwriting,
            Edition::full(),
            &windows
        ));

        let mut linux = installation(DesktopPlatform::Linux);
        assert!(offered_by(
            ResourcePack::Handwriting,
            Edition::full(),
            &linux
        ));
        linux.handwriting_model = true;
        assert!(!offered_by(
            ResourcePack::Handwriting,
            Edition::full(),
            &linux
        ));
    }

    /// 落定重排模型在三个桌面平台上都按需下载，随包带着时不列出；不带中文主词库的版本（日文、越南文、藏文）不列出。
    #[test]
    fn the_settled_model_is_offered_unless_it_is_bundled() {
        for platform in [
            DesktopPlatform::Macos,
            DesktopPlatform::Windows,
            DesktopPlatform::Linux,
        ] {
            let mut current = installation(platform);
            for edition in Edition::all() {
                let chinese = edition
                    .resources
                    .components
                    .iter()
                    .any(|component| component == "chinese-main");
                assert_eq!(
                    offered_by(ResourcePack::SettledModel, edition, &current),
                    chinese,
                    "{platform:?} {}",
                    edition.id
                );
            }
            for id in ["japanese", "vietnamese", "tibetan"] {
                assert!(
                    !offered_by(
                        ResourcePack::SettledModel,
                        Edition::by_id(id).unwrap(),
                        &current
                    ),
                    "{platform:?} {id}"
                );
            }
            for id in ["full", "pinyin", "wubi"] {
                assert!(
                    offered_by(
                        ResourcePack::SettledModel,
                        Edition::by_id(id).unwrap(),
                        &current
                    ),
                    "{platform:?} {id}"
                );
            }
            current.settled_model = true;
            assert!(
                !offered_by(ResourcePack::SettledModel, Edition::full(), &current),
                "{platform:?}"
            );
        }
    }

    #[test]
    fn resource_pack_install_keys_never_collide_with_voice_model_ids() {
        for pack in ResourcePack::ALL {
            let key = install_key(pack);
            assert!(key.starts_with("resource-pack:"));
            assert!(!msime_client_core::is_bounded_ascii_identifier(&key, 128));
        }
        assert!(known_pack("japanese").is_ok());
        assert!(known_pack("settled-model").is_ok());
        assert_eq!(known_pack("voice").unwrap_err().code, "local_model_unknown");
    }

    /// state_root 下已完整安装的手写资源包里的模型才算数：缺少 `msime-model.json` 的目录可能是中断的安装。
    #[test]
    fn downloaded_handwriting_model_needs_a_published_pack() {
        let state = tempfile::tempdir().unwrap();
        let document = serde_json::json!({ "preferences_directory": state.path() });
        assert_eq!(crate::downloaded_handwriting_model(Some(&document)), None);
        let pack = resource_packs::root(state.path()).join(ResourcePack::Handwriting.id());
        std::fs::create_dir_all(&pack).unwrap();
        let model = pack.join("handwriting-zh_CN.model");
        std::fs::write(&model, b"placeholder").unwrap();
        assert_eq!(crate::downloaded_handwriting_model(Some(&document)), None);
        std::fs::write(
            pack.join(msime_client_core::voice::local_models::MANIFEST_FILE),
            serde_json::to_vec(&ResourcePack::Handwriting.manifest()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            crate::downloaded_handwriting_model(Some(&document)),
            Some(model)
        );
        // 相对的 preferences_directory 一律忽略。
        let relative = serde_json::json!({ "preferences_directory": "state" });
        assert_eq!(crate::downloaded_handwriting_model(Some(&relative)), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_downloaded_handwriting_model_is_ignored() {
        let state = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let document = serde_json::json!({ "preferences_directory": state.path() });
        let pack = resource_packs::root(state.path()).join(ResourcePack::Handwriting.id());
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(
            pack.join(msime_client_core::voice::local_models::MANIFEST_FILE),
            serde_json::to_vec(&ResourcePack::Handwriting.manifest()).unwrap(),
        )
        .unwrap();
        let target = outside.path().join("handwriting-zh_CN.model");
        std::fs::write(&target, b"placeholder").unwrap();
        std::os::unix::fs::symlink(&target, pack.join("handwriting-zh_CN.model")).unwrap();
        assert_eq!(crate::downloaded_handwriting_model(Some(&document)), None);
    }
}
