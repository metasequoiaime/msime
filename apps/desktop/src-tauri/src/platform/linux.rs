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
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
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
