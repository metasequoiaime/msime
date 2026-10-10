//! Windows host integration.

pub(crate) mod windows_account;
pub(crate) mod windows_settings_sync;
pub(crate) mod windows_voice;

/// 启动时检查 Server 目录里的版本声明（`Edition::of_windows_package`）：声明了本构建不认识的版本、或者声明读不了的包不能当成 full 运行，否则它会去读写 full 的状态目录、叫 full 的 Server 重启。
pub(crate) fn check_windows_edition() -> Result<(), &'static str> {
    msime_client_core::edition::Edition::windows_package_identity().map(|_| ())
}

/// 把版本的 identifier 写进编进二进制的 Tauri 配置。所有版本共用同一个 `msime-desktop` 可执行文件，`generate_context!` 编进去的是 full 的配置；应用数据目录（`%APPDATA%\<identifier>`）和单实例互斥量都按这里的 identifier 来，几个版本的 MSIME.exe 才能同时运行而不把启动参数转给另一个版本。full 什么也不改。
pub(crate) fn apply_edition_to_config(config: &mut tauri::Config) {
    let Ok(edition) = msime_client_core::edition::Edition::of_windows_package() else {
        return;
    };
    let (false, Some(identity)) = (edition.is_full(), edition.windows()) else {
        return;
    };
    let full = msime_client_core::edition::Edition::full();
    config.identifier = identity.tauri_identifier.clone();
    for window in &mut config.app.windows {
        window.title = window
            .title
            .replace(&full.display_name.zh_hans, &edition.display_name.zh_hans);
    }
}
