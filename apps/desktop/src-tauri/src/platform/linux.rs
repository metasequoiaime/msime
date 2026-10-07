//! Linux host integration.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub(crate) mod linux_account;
pub(crate) mod linux_audio_devices;
pub(crate) mod linux_clipboard;
pub(crate) mod linux_data_directory;
pub(crate) mod linux_dictionary_quiesce;
pub(crate) mod linux_process;
pub(crate) mod linux_program_handover;
pub(crate) mod linux_provider_credentials;
pub(crate) mod linux_setup;

/// 启动时检查前缀 bin 目录里的版本声明（`Edition::of_linux_package`）：声明了本构建不认识的版本、或者声明读不了的包不能当成 full 运行，否则它会去读写 full 的状态目录、连 full 的 socket、启停 full 的用户服务。
pub(crate) fn check_linux_edition() -> Result<(), &'static str> {
    msime_client_core::edition::Edition::linux_package_identity().map(|_| ())
}

/// 把版本的 identifier 写进编进二进制的 Tauri 配置。所有版本共用同一个 `msime-desktop` 可执行文件，`generate_context!` 编进去的是 full 的配置；应用数据目录（`$XDG_DATA_HOME/<identifier>`）和单实例的 D-Bus 名都按这里的 identifier 来，几个版本的设置窗口才能同时运行，而不把启动参数转给另一个版本。窗口标题里的产品名换成本版本的。full 什么也不改。
pub(crate) fn apply_edition_to_config(config: &mut tauri::Config) {
    let Ok(edition) = msime_client_core::edition::Edition::of_linux_package() else {
        return;
    };
    let (false, Some(identity)) = (edition.is_full(), edition.linux()) else {
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

/// Read at most `max_bytes + 1` bytes so callers can distinguish an accepted
/// file from one that crossed its bound after its metadata was inspected.
pub(crate) fn read_bounded_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let initial_size = file.metadata()?.len().min(max_bytes.saturating_add(1));
    let mut bytes = Vec::with_capacity(usize::try_from(initial_size).unwrap_or(0));
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// 拒绝经过符号链接祖先目录解析的存储路径，`msime-path-trust` 信任的系统链接除外。
pub(crate) fn reject_symlink_ancestors(path: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(path)
}

pub(crate) fn create_directory_and_check(path: &Path) -> io::Result<bool> {
    reject_symlink_ancestors(path)?;
    std::fs::create_dir_all(path)?;
    reject_symlink_ancestors(path)?;
    Ok(std::fs::symlink_metadata(path)?.file_type().is_dir())
}

pub(crate) fn config_home(xdg: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    xdg.map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".config"))
        })
}
