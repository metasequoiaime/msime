//! 存储路径要做符号链接检查，防止被人放进去的链接把客户端的写入重定向到别处。但通往每个应用存储的路径上，有些链接本来就属于操作系统，连它们一起拒绝，等于拒绝了该平台上的全部存储。这个 crate 是唯一列出这些系统链接的地方，所有逐层检查存储路径的 crate 都来这里查询。

use std::path::{Component, Path, PathBuf};

/// 存储路径可以经过的系统链接，以及每条链接唯一受信任的目标。
///
/// - macOS 和 iOS 上 `/tmp`、`/var` 是指向 `/private` 的链接，临时目录和每个用户的目录都在它们下面；iOS 真机上的应用容器和 App Group 容器位于 `/var/mobile` 下（Foundation 给出的路径可能带 `/private` 前缀，也可能不带）。
/// - Android 11 起隔离应用数据：在应用自己的 mount namespace 里，`/data/user/0` 是指向 `/data/data` 的链接，主用户的 `Context.getFilesDir()` 会经过它。从 `adb shell` 看不到这条链接。
const SYSTEM_ALIASES: &[(&str, &str)] = if cfg!(any(target_os = "macos", target_os = "ios")) {
    &[("/tmp", "/private/tmp"), ("/var", "/private/var")]
} else if cfg!(target_os = "android") {
    &[("/data/user/0", "/data/data")]
} else {
    &[]
};

/// 判断 `path` 是否是本平台的系统链接之一，并且从它读出的 `target`（可能是相对路径）解析后正好是该链接唯一受信任的指向。光凭名字不能证明链接归系统所有，所以目标也要核对。
pub fn trusted_system_alias_target(path: &Path, target: &Path) -> bool {
    let Some((_, expected)) = SYSTEM_ALIASES
        .iter()
        .find(|(alias, _)| path == Path::new(alias))
    else {
        return false;
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("/"));
    normalize_lexical(&parent.join(target)) == Path::new(expected)
}

/// 判断 `path` 是否是本平台系统自己放置的符号链接，见 [`trusted_system_alias_target`]。
///
/// Linux（以及目标为 `target_os = "linux"` 的 HarmonyOS）没有固定清单：Fedora Silverblue 等 ostree 系统把 `/home` 链接到 `var/home`，迁到另一块磁盘上的 `/home` 也常用链接接回来。因此在这些平台上，只有 root 才能创建或修改的链接才受信任，见 [`is_root_only_link`]。
pub fn is_trusted_system_alias(path: &Path) -> bool {
    if cfg!(target_os = "linux") {
        return is_root_only_link(path);
    }
    !SYSTEM_ALIASES.is_empty()
        && std::fs::read_link(path).is_ok_and(|target| trusted_system_alias_target(path, &target))
}

/// 判断 `path` 是否是属于 root 的符号链接，且所在目录也属于 root、组和其他用户都不可写。这样的链接只有 root 能创建、替换或删除，不可能是别人为了重定向用户存储而放进去的。位于 `/tmp` 这类设置了 sticky 位、所有人可写的目录里的链接不受信任。
pub fn is_root_only_link(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Some(parent) = path.parent() else {
            return false;
        };
        let link_is_root_owned = std::fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.file_type().is_symlink() && metadata.uid() == 0);
        let parent_is_root_only = std::fs::metadata(parent).is_ok_and(|metadata| {
            metadata.is_dir() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0
        });
        link_is_root_owned && parent_is_root_only
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// 拒绝 `path` 任何一级上的符号链接（包括最后一级），唯一的例外是最后一级之上至多一个受信任的系统链接。
///
/// 不存在的层级可以接受，调用方正要创建它们；其它 I/O 错误原样返回。
pub fn reject_symlinked_components(path: &Path) -> std::io::Result<()> {
    let mut current = PathBuf::new();
    let mut saw_system_alias = false;
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::CurDir => continue,
            Component::Normal(_) => current.push(component),
            _ => {
                current.push(component);
                continue;
            }
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let last = index + 1 == components.len();
                if last
                    || saw_system_alias
                    || !path.is_absolute()
                    || !is_trusted_system_alias(&current)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "path contains a symbolic link",
                    ));
                }
                saw_system_alias = true;
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_require_the_exact_system_target() {
        let macos = cfg!(any(target_os = "macos", target_os = "ios"));
        assert_eq!(
            trusted_system_alias_target(Path::new("/tmp"), Path::new("private/tmp")),
            macos
        );
        assert_eq!(
            trusted_system_alias_target(Path::new("/var"), Path::new("/private/var")),
            macos
        );
        assert!(!trusted_system_alias_target(
            Path::new("/tmp"),
            Path::new("/Users/synthetic/outside")
        ));
        assert!(!trusted_system_alias_target(
            Path::new("/tmp/work"),
            Path::new("/private/tmp/work")
        ));
        let android = cfg!(target_os = "android");
        assert_eq!(
            trusted_system_alias_target(Path::new("/data/user/0"), Path::new("/data/data")),
            android
        );
        assert!(!trusted_system_alias_target(
            Path::new("/data/user/0"),
            Path::new("/data/local/tmp")
        ));
        assert!(!trusted_system_alias_target(
            Path::new("/data/user/10"),
            Path::new("/data/data")
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_links_the_system_did_not_make() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(root.path()).unwrap();
        let linked = root.join("linked");
        symlink(outside.path(), &linked).unwrap();
        assert!(reject_symlinked_components(&linked.join("missing/below")).is_err());
        assert!(reject_symlinked_components(&linked).is_err());
        assert!(reject_symlinked_components(&root.join("missing/below")).is_ok());
        std::fs::remove_file(linked).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn root_only_links_need_root_and_a_closed_directory() {
        use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let link = directory.path().join("home");
        symlink(directory.path(), &link).unwrap();
        // 临时目录属于运行测试的用户且不对外开放（0700），所以只有测试以 root 身份运行时（Linux 容器里就是这样）这条链接才算受信任。
        let as_root = std::fs::symlink_metadata(&link).unwrap().uid() == 0;
        assert_eq!(is_root_only_link(&link), as_root);
        assert!(!is_root_only_link(directory.path()));
        // 目录一旦允许其他人写入，链接就可能已被他们替换。
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(!is_root_only_link(&link));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_trusts_only_its_listed_aliases() {
        // `/etc` 和 `/var` 一样是 root 在 root 自己不对外开放的 `/` 下的链接，但 macOS 只信任清单里列出的别名，而且目标必须完全一致。
        assert!(is_root_only_link(Path::new("/etc")));
        assert!(!is_trusted_system_alias(Path::new("/etc")));
        assert!(is_trusted_system_alias(Path::new("/var")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn passes_through_a_macos_system_alias_once() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        assert!(Path::new("/tmp").is_symlink());
        assert!(reject_symlinked_components(&directory.path().join("missing")).is_ok());
        assert!(reject_symlinked_components(Path::new("/tmp")).is_err());
    }
}
