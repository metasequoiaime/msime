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
pub(crate) mod linux_provider_credentials;
pub(crate) mod linux_setup;

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
