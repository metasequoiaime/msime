//! 存储路径要做符号链接检查，防止被人放进去的链接把客户端的写入重定向到别处。但通往每个应用存储的路径上，有些链接本来就属于操作系统，连它们一起拒绝，等于拒绝了该平台上的全部存储。这个 crate 是唯一列出这些系统链接的地方，所有逐层检查存储路径的 crate 都来这里查询。

use std::fs::File;
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

/// 只读打开的文件有多个硬链接时，是否仍然可以读。
///
/// 拒绝硬链接防的是：别人在我们读取的目录里放一个指向别处 inode 的链接，让我们把不属于这里的文件当作资源或私有数据读进来。要放下这样的链接，必须能写这个目录。所以文件所在的目录如果属于 root 或当前用户、组和其他用户都不可写，里面的多链接文件只可能是 root 或用户自己建的，例如包管理器把相同文件合并成硬链接（Nix 的 store 去重、ostree 部署），或者用户用 `cp -al` 做的备份。这时放行；其他情况都拒绝。
///
/// 只用于只读打开。写入一个多链接文件会改到链接另一端的文件，那条规则与目录是否可信无关，写入路径继续要求单链接。
///
/// `file` 是已经打开的文件，`path` 是打开它用的路径。这里以句柄打开 `path` 的父目录，检查目录的属主和权限，再在这个目录句柄下按名字查到的 inode 必须就是 `file` 的那一个，所以检查的正是手里这个文件所在的目录，而不是检查之后被换上来的另一个。Windows 等没有属主和权限位的平台一律不信任。
pub fn multi_link_is_trusted(file: &File, path: &Path) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Ok(false);
        };
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        let directory = open_directory(parent)?;
        if !directory_is_trusted(&directory.metadata()?) {
            return Ok(false);
        }
        let opened = file.metadata()?;
        Ok(entry_identity(&directory, name)?
            .is_some_and(|(device, inode)| device == opened.dev() && inode == opened.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path);
        Ok(false)
    }
}

/// 与 [`multi_link_is_trusted`] 相同，但文件是相对于 `directory` 这个已打开的目录句柄打开的（`openat`），所以只需检查这个目录本身。
pub fn multi_link_in_directory_is_trusted(directory: &File) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        Ok(directory_is_trusted(&directory.metadata()?))
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Ok(false)
    }
}

/// 与 [`multi_link_is_trusted`] 相同，给只能按路径打开文件的调用方用（SQLite）：`path` 处必须是普通文件，且所在目录可信。按路径检查之后调用方还要按路径再打开一次，所以它只比按句柄的版本多一个与原先单链接检查相同的窗口。
pub fn multi_link_path_is_trusted(path: &Path) -> std::io::Result<bool> {
    #[cfg(unix)]
    {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return Ok(false);
        };
        let parent = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        let directory = open_directory(parent)?;
        Ok(directory_is_trusted(&directory.metadata()?)
            && entry_identity(&directory, name)?.is_some())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(false)
    }
}

/// 目录属于 root 或当前用户，且组和其他用户都不可写：只有 root 或当前用户能在里面放进、替换文件。
#[cfg(unix)]
fn directory_is_trusted(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.is_dir()
        && (metadata.uid() == 0 || metadata.uid() == effective_uid())
        && metadata.mode() & 0o022 == 0
}

#[cfg(unix)]
fn open_directory(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
        .open(path)
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn effective_uid() -> u32 {
    // SAFETY: geteuid takes no arguments, cannot fail and touches no memory of ours.
    unsafe { libc::geteuid() }
}

/// `directory` 下名为 `name` 的目录项（不跟随符号链接）是普通文件时，返回它的设备号和 inode 号；不存在或不是普通文件时返回 `None`。
#[cfg(unix)]
#[allow(unsafe_code)]
// `libc::stat` 的字段宽度随平台而变（`st_mode` 与 `S_IFMT` 在有的平台上类型不同），统一转换后比较，在宽度刚好一致的平台上这些转换就是多余的。
#[allow(clippy::unnecessary_cast, clippy::useless_conversion)]
fn entry_identity(directory: &File, name: &std::ffi::OsStr) -> std::io::Result<Option<(u64, u64)>> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::AsRawFd;
    let Ok(name) = std::ffi::CString::new(name.as_bytes()) else {
        return Ok(None);
    };
    let mut status = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the descriptor is open for the duration of the call, `name` is a NUL-terminated string that outlives it, and `status` points to writable memory of the size fstatat fills.
    let result = unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            status.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result != 0 {
        let error = std::io::Error::last_os_error();
        return match error.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            _ => Err(error),
        };
    }
    // SAFETY: fstatat returned 0, so it initialised every field.
    let status = unsafe { status.assume_init() };
    if (status.st_mode as u32) & (libc::S_IFMT as u32) != libc::S_IFREG as u32 {
        return Ok(None);
    }
    Ok(Some((status.st_dev as u64, status.st_ino as u64)))
}

/// 建一个本 crate 永远不会当作系统链接的符号链接，给测试拒绝行为用。Linux 上属于 root、所在目录只有 root 能写的链接是受信任的（见 [`is_root_only_link`]），以 root 身份运行的测试（测试容器、容器里的 CI 任务）建出的链接正是这样，所以那时把链接的属主交给 `nobody`。
#[cfg(all(unix, any(test, feature = "test-support")))]
pub fn untrusted_symlink(
    original: impl AsRef<Path>,
    link: impl AsRef<Path>,
) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let link = link.as_ref();
    std::os::unix::fs::symlink(original, link)?;
    if std::fs::symlink_metadata(link)?.uid() == 0 {
        std::os::unix::fs::lchown(link, Some(65534), Some(65534))?;
    }
    Ok(())
}

/// 让别的用户也能写 `directory`，给测试拒绝行为用：这样的目录里的多链接文件可能是别人放进来的，不受信任（见 [`multi_link_is_trusted`]）。
#[cfg(all(unix, any(test, feature = "test-support")))]
pub fn open_to_other_users(directory: impl AsRef<Path>) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o777))
}

/// 让只有属主能写 `directory`（0700），给测试放行行为用。临时目录的权限取决于 umask：umask 为 002 时目录是组可写的，里面的多链接文件不受信任。
#[cfg(all(unix, any(test, feature = "test-support")))]
pub fn close_to_other_users(directory: impl AsRef<Path>) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
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
        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(root.path()).unwrap();
        let linked = root.join("linked");
        untrusted_symlink(outside.path(), &linked).unwrap();
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

    #[cfg(unix)]
    #[test]
    fn hard_linked_files_are_trusted_only_in_a_closed_directory() {
        use std::os::unix::fs::PermissionsExt;

        // 属于运行测试的用户（或 root）且只有属主能写，相当于包管理器的 store 或用户自己的目录。
        let directory = tempfile::tempdir().unwrap();
        close_to_other_users(directory.path()).unwrap();
        let original = directory.path().join("model");
        let linked = directory.path().join("linked-model");
        std::fs::write(&original, b"model").unwrap();
        std::fs::hard_link(&original, &linked).unwrap();
        let file = File::open(&linked).unwrap();
        assert!(multi_link_is_trusted(&file, &linked).unwrap());
        assert!(multi_link_path_is_trusted(&linked).unwrap());
        assert!(
            multi_link_in_directory_is_trusted(&File::open(directory.path()).unwrap()).unwrap()
        );

        // 组或其他用户能写这个目录，链接就可能是他们放的。
        for mode in [0o770, 0o707, 0o777] {
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(mode))
                .unwrap();
            assert!(
                !multi_link_is_trusted(&file, &linked).unwrap(),
                "mode {mode:o}"
            );
            assert!(
                !multi_link_path_is_trusted(&linked).unwrap(),
                "mode {mode:o}"
            );
            assert!(
                !multi_link_in_directory_is_trusted(&File::open(directory.path()).unwrap())
                    .unwrap(),
                "mode {mode:o}"
            );
        }
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn multi_link_trust_checks_the_directory_of_the_opened_file() {
        let directory = tempfile::tempdir().unwrap();
        let opened = directory.path().join("opened");
        let elsewhere = directory.path().join("elsewhere");
        std::fs::write(&opened, b"opened").unwrap();
        std::fs::write(&elsewhere, b"elsewhere").unwrap();
        let file = File::open(&opened).unwrap();
        // 路径上现在是另一个 inode：检查之后被换掉的文件不能借用这个目录的信任。
        assert!(!multi_link_is_trusted(&file, &elsewhere).unwrap());
        assert!(!multi_link_is_trusted(&file, &directory.path().join("missing")).unwrap());
        // 符号链接本身不是普通文件。
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&opened, &link).unwrap();
        assert!(!multi_link_is_trusted(&file, &link).unwrap());
        assert!(!multi_link_path_is_trusted(&link).unwrap());
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
