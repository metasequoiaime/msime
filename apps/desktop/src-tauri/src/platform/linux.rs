//! Linux host integration.

use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

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

/// Refuse storage paths that resolve through a symlinked ancestor. The only
/// compatibility exception is an absolute macOS-style `/tmp` or `/var` alias;
/// Linux normally has real directories there, but accepting the alias keeps
/// profiles shared with macOS usable.
pub(crate) fn reject_symlink_ancestors(path: &Path) -> io::Result<()> {
    let mut current = PathBuf::new();
    let mut saw_prefix_alias = false;
    let mut saw_real_component = false;
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::Prefix(_) | Component::RootDir => current.push(component),
            Component::CurDir => continue,
            Component::ParentDir => current.push(component),
            Component::Normal(_) => {
                current.push(component);
                match std::fs::symlink_metadata(&current) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        let system_alias = path.is_absolute()
                            && !saw_real_component
                            && !saw_prefix_alias
                            && matches!(component, Component::Normal(name) if *name == OsStr::new("tmp") || *name == OsStr::new("var"));
                        if index + 1 == components.len()
                            || saw_real_component
                            || saw_prefix_alias
                            || !system_alias
                        {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                "storage path has a symbolic-link ancestor",
                            ));
                        }
                        saw_prefix_alias = true;
                    }
                    Ok(_) => saw_real_component = true,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
    }
    Ok(())
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
