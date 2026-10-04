//! The four runtime roots a session reads and writes. Paths are captured at construction, so changing the process environment cannot redirect a live session. The working dictionaries under `dictionaries` are durable user data, never a disposable cache.
//!
//! `prepare_runtime_paths`, which stages a generation directory, lives in `user_dictionary::generation` because it replays the journal.

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
            if !root.is_absolute() {
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
}
