use msime_client_services::account::AccountError;
use std::sync::Arc;

/// 预览表的发布与清理、快照临时文件的建目录、读写、删除和清理都在 host-api（#8201 的决定 3），与 Windows 设置应用和 `msime_client_cloud_dictionary` 共用同一套：打开时不跟随符号链接，路径上每一级都必须是真实目录，读最多 512 MiB。
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) use msime_host_api::cloud_dictionary::snapshot_files::{
    cleanup_stale_snapshot_previews, prepare_snapshot_directory, publish_snapshot_preview,
    read_snapshot_file, remove_snapshot_file, take_invalid_snapshot_previews, write_snapshot_file,
};

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
    // 这几个工具现在是 host-api 的（见上面的 use），这里核对 Tauri 移动端拿到的就是不跟随符号链接的那一套。
    use msime_host_api::cloud_dictionary::snapshot_files::{
        cleanup_stale_snapshot_previews, prepare_snapshot_directory, read_snapshot_file,
        write_snapshot_file,
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
