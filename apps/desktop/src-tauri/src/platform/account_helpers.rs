use msime_client_services::account::AccountError;
#[cfg(any(target_os = "ios", target_os = "android", test))]
use std::path::Path;
use std::sync::Arc;

/// 预览表的发布与清理搬到了 host-api，与 Windows 设置应用和 `msime_client_cloud_dictionary` 共用。
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) use msime_host_api::cloud_dictionary::snapshot_files::{
    publish_snapshot_preview, take_invalid_snapshot_previews,
};

/// Create a snapshot scratch directory only when every path component is a real directory.
/// Snapshot writers pass paths in this directory to native bridges, so following a replaced
/// temporary-directory symlink would redirect cloud data outside the app's scratch area.
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn prepare_snapshot_directory(directory: &Path) -> std::io::Result<()> {
    crate::shared::atomic_file::create_directory_and_check(directory).map(|_| ())
}

/// Write snapshot text through a private temporary sibling before replacing the requested path.
/// Restore paths are generated internally, but a concurrent filesystem change must not turn one
/// into a write through a leaf symlink.
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn write_snapshot_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    crate::shared::atomic_file::write(path, contents)
}

/// Read a staged snapshot through a no-follow handle and the native 512 MiB limit.
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn read_snapshot_file(path: &Path) -> std::io::Result<String> {
    let file = crate::shared::atomic_file::open_private(path)?;
    let bytes =
        crate::shared::bounded_body::read_bounded(file, 512 * 1024 * 1024).map_err(|error| {
            match error {
                crate::shared::bounded_body::BoundedReadError::TooLarge => std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "snapshot exceeds size limit",
                ),
                crate::shared::bounded_body::BoundedReadError::Read(error) => error,
            }
        })?;
    String::from_utf8(bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "snapshot is not UTF-8"))
}

#[cfg(any(target_os = "ios", target_os = "android", test))]
#[allow(dead_code)]
pub(crate) fn remove_snapshot_file(path: &Path) -> std::io::Result<()> {
    crate::shared::atomic_file::remove_private(path)
}

#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn cleanup_stale_snapshot_previews(directory: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let opened = match crate::shared::atomic_file::open_private_directory(directory) {
            Ok(opened) => opened,
            Err(_error) if msime_path_trust::reject_symlinked_components(directory).is_err() => {
                return Ok(())
            }
            Err(error) => return Err(error),
        };
        cleanup_stale_snapshot_previews_in_directory(directory, &opened)
    }
    #[cfg(not(unix))]
    {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if entry.file_type()?.is_file()
                && file_name.starts_with("download-")
                && file_name.ends_with(".ndjson")
            {
                let _ = remove_snapshot_file(&entry.path());
            }
        }
        Ok(())
    }
}

#[cfg(all(unix, any(target_os = "ios", target_os = "android", test)))]
fn cleanup_stale_snapshot_previews_in_directory(
    directory: &Path,
    opened: &std::os::fd::OwnedFd,
) -> std::io::Result<()> {
    let entries: Vec<_> = std::fs::read_dir(directory)?.collect::<Result<_, _>>()?;
    if !crate::shared::atomic_file::directory_matches(directory, opened)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "snapshot directory changed while listing previews",
        ));
    }
    for entry in entries {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if entry.file_type()?.is_file()
            && file_name.starts_with("download-")
            && file_name.ends_with(".ndjson")
        {
            let _ = crate::shared::atomic_file::remove_private_at(
                opened,
                entry.file_name().as_os_str(),
            );
        }
    }
    Ok(())
}

/// Keep restore requests from copying data that the native snapshot inspectors will reject.
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn snapshot_text_within_limit(bytes: usize) -> bool {
    bytes <= 512 * 1024 * 1024
}

pub(crate) fn account_command_error(error: AccountError) -> crate::CommandError {
    crate::CommandError { code: error.code() }
}

/// Maps an account error from a community service to the `community_*` codes the community pages translate. It is the mapping `mobile_community` uses, so a page reads the same code on every host.
pub(crate) fn community_error(error: AccountError) -> crate::CommandError {
    crate::CommandError {
        code: error.community_code(),
    }
}

/// Parses a community item id the page sent.
pub(crate) fn community_id(
    value: &str,
) -> Result<msime_client_core::uuid::Uuid, crate::CommandError> {
    msime_client_core::uuid::Uuid::parse_str(value).map_err(|_| crate::CommandError {
        code: "community_invalid",
    })
}

/// Runs one blocking community service call off the async runtime, mapping its error with [`community_error`].
pub(crate) async fn community_service_call<T, S, F>(
    service: Arc<S>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    S: Send + Sync + 'static,
    F: FnOnce(&S) -> Result<T, AccountError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&service))
        .await
        .map_err(|_| crate::CommandError {
            code: "community_unavailable",
        })?
        .map_err(community_error)
}

pub(crate) async fn call_session<S, T, F>(
    session: &Arc<S>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    S: Send + Sync + 'static,
    T: Send + 'static,
    F: FnOnce(&S) -> Result<T, AccountError> + Send + 'static,
{
    let session = Arc::clone(session);
    tauri::async_runtime::spawn_blocking(move || operation(&session))
        .await
        .map_err(|_| crate::CommandError {
            code: "account_unavailable",
        })?
        .map_err(account_command_error)
}

/// [`call_session`] 给返回 [`AccountFailure`] 的新接口用：错误码取 `AccountFailure::code`，页面能分出「不存在」「被限流」和接口自己的码（例如 `account_qq_link_code_not_found`）。
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) async fn call_session_detailed<S, T, F>(
    session: &Arc<S>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    S: Send + Sync + 'static,
    T: Send + 'static,
    F: FnOnce(&S) -> Result<T, msime_client_services::account::AccountFailure> + Send + 'static,
{
    let session = Arc::clone(session);
    tauri::async_runtime::spawn_blocking(move || operation(&session))
        .await
        .map_err(|_| crate::CommandError {
            code: "account_unavailable",
        })?
        .map_err(|failure| crate::CommandError {
            code: failure.code(),
        })
}

#[cfg(test)]
mod tests {
    use super::{
        cleanup_stale_snapshot_previews, cleanup_stale_snapshot_previews_in_directory,
        prepare_snapshot_directory, read_snapshot_file, write_snapshot_file,
    };

    #[test]
    fn stale_snapshot_cleanup_removes_only_download_ndjson_files() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("download-old.ndjson"), b"stale").unwrap();
        std::fs::write(directory.path().join("download-in-progress"), b"keep").unwrap();
        std::fs::write(directory.path().join("export-old.ndjson"), b"keep").unwrap();

        cleanup_stale_snapshot_previews(directory.path()).unwrap();

        assert!(!directory.path().join("download-old.ndjson").exists());
        assert!(directory.path().join("download-in-progress").exists());
        assert!(directory.path().join("export-old.ndjson").exists());
    }

    #[cfg(unix)]
    #[test]
    fn stale_snapshot_cleanup_never_deletes_through_a_symlinked_directory() {
        // 以 root 运行时（Linux 测试容器）普通 `symlink` 建出的链接会被 `msime_path_trust::is_root_only_link` 当成受信任的系统链接，测不到拒绝行为；`untrusted_symlink` 那时把属主交给 `nobody`。
        use msime_path_trust::untrusted_symlink as symlink;

        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("download-outside.ndjson");
        std::fs::write(&outside_file, b"synthetic-outside").unwrap();
        let root = tempfile::tempdir().unwrap();
        let linked = root.path().join("snapshots");
        symlink(outside.path(), &linked).unwrap();

        cleanup_stale_snapshot_previews(&linked).unwrap();

        assert_eq!(std::fs::read(&outside_file).unwrap(), b"synthetic-outside");
    }

    #[cfg(unix)]
    #[test]
    fn stale_snapshot_cleanup_rejects_a_directory_replacement_after_opening() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("snapshots");
        let moved = root.path().join("snapshots-moved");
        let replacement = root.path().join("replacement");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&replacement).unwrap();
        let name = "download-stale.ndjson";
        std::fs::write(original.join(name), b"original").unwrap();
        std::fs::write(replacement.join(name), b"replacement").unwrap();
        let opened = crate::shared::atomic_file::open_private_directory(&original).unwrap();

        std::fs::rename(&original, &moved).unwrap();
        std::fs::rename(&replacement, &original).unwrap();

        assert!(cleanup_stale_snapshot_previews_in_directory(&original, &opened).is_err());
        assert!(moved.join(name).exists());
        assert!(original.join(name).exists());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_directory_rejects_symlinked_ancestors() {
        use msime_path_trust::untrusted_symlink as symlink;

        let outside = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let linked = parent.path().join("snapshots");
        symlink(outside.path(), &linked).unwrap();

        let nested = linked.join("missing");
        assert!(prepare_snapshot_directory(&nested).is_err());
        assert!(!outside.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_file_write_replaces_a_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.ndjson");
        std::fs::write(&target, b"synthetic-outside").unwrap();
        let path = root.path().join("restore.ndjson");
        symlink(&target, &path).unwrap();

        write_snapshot_file(&path, b"synthetic-snapshot").unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-outside");
        assert_eq!(std::fs::read(&path).unwrap(), b"synthetic-snapshot");
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_file_read_rejects_a_symlinked_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("outside.ndjson");
        std::fs::write(&target, b"synthetic-outside").unwrap();
        let path = root.path().join("export.ndjson");
        symlink(&target, &path).unwrap();

        assert!(read_snapshot_file(&path).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"synthetic-outside");
    }
}
