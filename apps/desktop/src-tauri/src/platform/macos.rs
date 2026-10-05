//! macOS host integration.
//!
//! The four modules gated on `test` as well as the target hold logic the host
//! tests exercise on any machine; the rest link against AppKit and only build
//! for macOS.

use msime_client_core::host_surface::{PanelSurface, SurfaceRoute};
#[cfg(target_os = "macos")]
use msime_host_macos::cloud_clipboard::{CloudClipboardError, CloudClipboardSession};
use tauri::utils::config::WindowConfig;

#[cfg(target_os = "macos")]
pub(crate) fn cloud_clipboard_error(error: CloudClipboardError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            CloudClipboardError::Invalid => "invalid",
            CloudClipboardError::Unavailable => "unavailable",
            CloudClipboardError::OutcomeUnknown => "outcome_unknown",
            CloudClipboardError::Conflict => "conflict",
        },
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn native_cloud_session_from_environment(
    variable: &str,
    invalid_message: &'static str,
) -> Result<Option<CloudClipboardSession>, &'static str> {
    match std::env::var(variable) {
        Ok(value) => CloudClipboardSession::parse(&value)
            .map(Some)
            .map_err(|_| invalid_message),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err(invalid_message),
    }
}

pub(crate) fn startup_panel_for_route(
    route: Option<SurfaceRoute>,
    expected: SurfaceRoute,
) -> Option<PanelSurface> {
    route.filter(|route| *route == expected)?.panel()
}

pub(crate) fn prepare_windows_for_route(
    windows: &mut [WindowConfig],
    route: Option<SurfaceRoute>,
    expected: SurfaceRoute,
) {
    prepare_windows_for_panel(windows, startup_panel_for_route(route, expected));
}

pub(crate) fn prepare_windows_for_panel(windows: &mut [WindowConfig], panel: Option<PanelSurface>) {
    if panel.is_some() {
        for window in windows.iter_mut().filter(|window| window.label == "main") {
            window.visible = false;
            window.focus = false;
        }
    }
}

/// 本设置应用所属的版本，取自安装包里的版本声明（`Edition::of_macos_bundle`）：没有声明是 full，开发运行和测试也是 full。声明坏了的包在启动时就退出了（`check_macos_edition`），之后的调用一定读得到。
#[cfg(any(target_os = "macos", test))]
pub(crate) fn macos_edition() -> &'static msime_client_core::edition::Edition {
    msime_client_core::edition::Edition::of_macos_bundle()
        .expect("the package's edition declaration was checked at startup")
}

/// 本设置应用所属版本在 macOS 上的身份标识（版本表 `platforms.macos`）：输入法 bundle、状态目录、偏好域都按它来，同时安装的几个版本互不干扰。
#[cfg(any(target_os = "macos", test))]
pub(crate) fn macos_identity() -> &'static msime_client_core::edition::MacosIdentity {
    macos_edition()
        .macos()
        .expect("every edition in shared/contracts/editions.json has macOS identifiers")
}

/// 启动时检查安装包的版本声明：声明了本构建不认识的版本、或者声明读不了的包不能当成 full 运行，否则它会去读写 full 的状态目录、停掉 full 的输入法。
#[cfg(target_os = "macos")]
pub(crate) fn check_macos_edition() -> Result<(), &'static str> {
    msime_client_core::edition::Edition::of_macos_bundle()?
        .macos()
        .map(|_| ())
        .ok_or("this edition has no macOS identifiers")
}

/// 把版本的身份写进编进二进制的 Tauri 配置。所有版本共用同一个 `msime-desktop` 可执行文件，`generate_context!` 编进去的是 full 的配置；`tauri bundle --config` 按版本写的是包的 Info.plist，运行时的应用数据目录、单实例等仍按这里的 identifier 来，所以两边必须一致。full 什么也不改。
#[cfg(target_os = "macos")]
pub(crate) fn apply_edition_to_config(config: &mut tauri::Config) {
    let edition = macos_edition();
    if edition.is_full() {
        return;
    }
    let full = msime_client_core::edition::Edition::full();
    config.identifier = macos_identity().settings_bundle_id.clone();
    config.product_name = Some(edition.display_name.en.clone());
    for window in &mut config.app.windows {
        window.title = window
            .title
            .replace(&full.display_name.zh_hans, &edition.display_name.zh_hans);
    }
}

#[cfg(target_os = "macos")]
pub(crate) mod macos_account;
#[cfg(target_os = "macos")]
pub(crate) mod macos_cloud_clipboard;
#[cfg(target_os = "macos")]
pub(crate) mod macos_cloud_dictionary;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_data_directory;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_handwriting;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_input_source;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_keyboard;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos_launch;
#[cfg(target_os = "macos")]
pub(crate) mod macos_panel_session;
