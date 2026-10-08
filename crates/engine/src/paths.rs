//! The four runtime roots a session reads and writes. Paths are captured at construction, so changing the process environment cannot redirect a live session. The working dictionaries under `dictionaries` are durable user data, never a disposable cache.
//!
//! `prepare_runtime_paths`, which stages a generation directory, lives in `user_dictionary::generation` because it replays the journal.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimePaths {
    /// The immutable packaged bundle.
    pub resources: PathBuf,
    /// The journal, learned glosses and hand-written translations.
    pub user_data: PathBuf,
    pub cache: PathBuf,
    /// The generation directory holding the mutable working copies (`user_data/dictionaries/<content id>`).
    pub dictionaries: PathBuf,
}

impl RuntimePaths {
    /// 资源包里的文件。名字是 `assets` 里的常量，或已被 `helpcode::custom_schema_stem` 化成单个普通路径成分的自定义辅助码名。现名文件缺席、`assets::LEGACY_NAMES` 里的旧名文件在场时返回旧名路径。
    pub fn resource(&self, name: &str) -> PathBuf {
        join_current_or_legacy(&self.resources, name)
    }

    pub fn user(&self, name: &str) -> PathBuf {
        join(&self.user_data, name)
    }

    /// 与 `resource` 一样，现名缺席而旧名文件在场时返回旧名。
    pub fn dictionary(&self, name: &str) -> PathBuf {
        join_current_or_legacy(&self.dictionaries, name)
    }

    /// Every root must be absolute; a relative root would resolve against whatever the host's working directory happens to be.
    pub fn validate(&self) -> Result<()> {
        for root in [
            &self.resources,
            &self.user_data,
            &self.cache,
            &self.dictionaries,
        ] {
            // wasm32-unknown-unknown 上 std 的 `is_absolute` 恒为 false（它还要求路径前缀），所以那里以有根路径为绝对路径。
            #[cfg(all(target_family = "wasm", target_os = "unknown"))]
            let absolute = root.has_root();
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            let absolute = root.is_absolute();
            if !absolute {
                return Err(EngineError::invalid(
                    diagnostics::RUNTIME_DIRECTORIES_MUST_BE_ABSOLUTE,
                ));
            }
        }
        Ok(())
    }
}

/// An empty root, an empty name or an absolute name yields an empty path, which every reader treats as a missing file, as the C++ did.
fn join(root: &Path, name: &str) -> PathBuf {
    if root.as_os_str().is_empty() || name.is_empty() || Path::new(name).is_absolute() {
        return PathBuf::new();
    }
    root.join(name)
}

/// `join`，但现名不存在而 `assets::LEGACY_NAMES` 中对应的旧名存在时返回旧名路径。两者都不存在时返回现名，读者报告的仍是缺少现名文件。
fn join_current_or_legacy(root: &Path, name: &str) -> PathBuf {
    let current = join(root, name);
    if current.as_os_str().is_empty() || std::fs::symlink_metadata(&current).is_ok() {
        return current;
    }
    assets::LEGACY_NAMES
        .iter()
        .find(|(present, _)| *present == name)
        .map(|(_, legacy)| root.join(legacy))
        .filter(|legacy| std::fs::symlink_metadata(legacy).is_ok())
        .unwrap_or(current)
}

/// 存储路径可以经过的系统链接只在 `msime-path-trust` 里列一次。
pub(crate) use msime_path_trust::is_trusted_system_alias;

/// Open an Engine asset without following a leaf symlink. Callers still
/// validate the file format and size through their own loaders; this closes
/// the check-then-open race between those checks and the read or mapping.
pub(crate) fn open_file_no_follow(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A replaced asset may be a FIFO; opening it on the engine thread must not block.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || !has_single_link(&file)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "engine asset is not a single-link regular file",
        ));
    }
    Ok(file)
}

/// 已打开的文件是否只有一个硬链接。engine 里拒绝硬链接的地方都用这一份：Windows 上标准库的 `MetadataExt::number_of_links` 还是不稳定特性（`windows_by_handle`），各处自己写就会在 Windows 上编译失败；这里经 `winapi-util` 读硬链接数，与 `msime_client_core::file_lock::has_single_link` 相同（engine 不依赖 client-core，用不了那一份）。
#[cfg(unix)]
pub(crate) fn has_single_link(file: &File) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(file.metadata()?.nlink() == 1)
}

/// 已打开的文件是否只有一个硬链接；见 Unix 版的说明。
#[cfg(windows)]
pub(crate) fn has_single_link(file: &File) -> io::Result<bool> {
    Ok(winapi_util::file::information(file)?.number_of_links() == 1)
}

/// 没有硬链接计数可读的平台一律按单链接处理。
#[cfg(not(any(unix, windows)))]
pub(crate) fn has_single_link(_: &File) -> io::Result<bool> {
    Ok(true)
}

/// 路径上的数据库文件是否只有一个硬链接。Unix 直接读 `metadata` 的 `nlink`；Windows 的硬链接数只能经句柄读，所以先不跟随重解析点地打开它。
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
fn path_has_single_link(path: &Path, metadata: &std::fs::Metadata) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let _ = path;
        Ok(metadata.nlink() == 1)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        let _ = metadata;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        has_single_link(&file)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (path, metadata);
        Ok(true)
    }
}

/// SQLite's `SQLITE_OPEN_NOFOLLOW` rejects trusted system aliases such as
/// macOS `/var` when they remain in the path. Reject untrusted components first,
/// then canonicalize only the parent so the SQLite flag protects the leaf while
/// normal platform storage roots continue to work.
pub(crate) fn sqlite_path_no_follow(path: &Path) -> io::Result<PathBuf> {
    sqlite_path_no_follow_with_parent_policy(path, true)
}

/// Resolve a database's parent directory while protecting only the final path
/// component with SQLite's `SQLITE_OPEN_NOFOLLOW` flag. This is used by
/// packaged language dictionaries, whose established contract permits an
/// application supplied directory alias.
pub(crate) fn sqlite_path_no_follow_allow_parent_symlinks(path: &Path) -> io::Result<PathBuf> {
    sqlite_path_no_follow_with_parent_policy(path, false)
}

fn sqlite_path_no_follow_with_parent_policy(
    path: &Path,
    reject_parent_symlinks: bool,
) -> io::Result<PathBuf> {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        let _ = reject_parent_symlinks;
        Ok(path.to_owned())
    }
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        if reject_parent_symlinks {
            msime_path_trust::reject_symlinked_components(path)?;
        }
        let parent = path.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "database path has no parent")
        })?;
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "database path has no filename")
        })?;
        let resolved = std::fs::canonicalize(parent)?.join(name);
        if let Ok(metadata) = std::fs::symlink_metadata(&resolved) {
            if metadata.file_type().is_symlink() || !path_has_single_link(&resolved, &metadata)? {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "database is not a single-link regular file",
                ));
            }
        }
        Ok(resolved)
    }
}

/// `join` for a name that came from outside the crate: a `..` component is refused rather than allowed to escape the root (`runtime_paths.cpp:14-23`).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no caller joins a user-derived name: the custom helpcode schema is guarded by custom_schema_stem, which refuses separators and a leading dot, matching helpcode_utils.cpp:37-40"
    )
)]
pub fn join_checked(root: &Path, name: &str) -> Result<PathBuf> {
    if Path::new(name)
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(EngineError::invalid(
            diagnostics::RUNTIME_ASSET_PATH_ESCAPES,
        ));
    }
    Ok(join(root, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn opening_an_asset_does_not_follow_a_leaf_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("asset.bin");
        std::fs::write(&target, b"synthetic engine asset").unwrap();
        let linked = root.path().join("asset.bin");
        symlink(&target, &linked).unwrap();

        assert!(open_file_no_follow(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic engine asset");
    }

    #[cfg(unix)]
    #[test]
    fn opening_an_asset_rejects_a_directory() {
        let root = tempfile::tempdir().unwrap();

        assert!(open_file_no_follow(root.path()).is_err());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn opening_an_asset_rejects_a_hard_link() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("asset.bin");
        std::fs::write(&target, b"synthetic engine asset").unwrap();
        let linked = root.path().join("asset.bin");
        std::fs::hard_link(&target, &linked).unwrap();

        assert!(open_file_no_follow(&linked).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic engine asset");
    }

    #[test]
    fn joins_like_the_reference() {
        let paths = RuntimePaths {
            resources: PathBuf::from("/r"),
            user_data: PathBuf::from("/u"),
            cache: PathBuf::from("/c"),
            dictionaries: PathBuf::new(),
        };
        assert_eq!(
            paths.resource("msime-pinyin.db"),
            PathBuf::from("/r/msime-pinyin.db")
        );
        assert_eq!(paths.dictionary("msime-pinyin.db"), PathBuf::new());
        assert_eq!(paths.user("/abs"), PathBuf::new());
        assert!(join_checked(Path::new("/r"), "helpcodes/../x").is_err());
        assert!(paths.validate().is_err());
    }

    /// 升级后还没换新的目录里只有旧名文件：现名缺席时读旧名，两者都在时只读现名。
    #[test]
    fn a_missing_current_name_falls_back_to_the_legacy_file() {
        let root = tempfile::tempdir().unwrap();
        let paths = RuntimePaths {
            resources: root.path().join("resources"),
            user_data: root.path().join("user"),
            cache: root.path().join("cache"),
            dictionaries: root.path().join("generation"),
        };
        std::fs::create_dir_all(&paths.resources).unwrap();
        std::fs::create_dir_all(&paths.dictionaries).unwrap();
        assert_eq!(
            paths.dictionary(assets::MAIN_DICTIONARY),
            paths.dictionaries.join("msime-pinyin.db")
        );
        std::fs::write(paths.dictionaries.join("msime.db"), b"old").unwrap();
        std::fs::write(paths.resources.join("others.db"), b"old").unwrap();
        assert_eq!(
            paths.dictionary(assets::MAIN_DICTIONARY),
            paths.dictionaries.join("msime.db")
        );
        assert_eq!(
            paths.resource(assets::OTHER_DICTIONARY),
            paths.resources.join("others.db")
        );
        // 没有旧名的文件照常拼接。
        assert_eq!(
            paths.resource(assets::WUBI_DICTIONARY),
            paths.resources.join("msime-wubi.db")
        );
        std::fs::write(paths.dictionaries.join("msime-pinyin.db"), b"new").unwrap();
        assert_eq!(
            paths.dictionary(assets::MAIN_DICTIONARY),
            paths.dictionaries.join("msime-pinyin.db")
        );
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn sqlite_paths_reject_hard_linked_databases() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = root.path().join("database.db");
        let external = outside.path().join("database.db");
        std::fs::write(&external, b"synthetic database").unwrap();
        std::fs::hard_link(&external, &target).unwrap();

        assert!(sqlite_path_no_follow(&target).is_err());
        assert_eq!(std::fs::read(&external).unwrap(), b"synthetic database");
    }
}
