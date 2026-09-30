use msime_client_core::account::AccountError;
use std::path::Path;
use std::sync::Arc;

pub(crate) fn cleanup_stale_snapshot_previews(directory: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if entry.file_type()?.is_file()
            && file_name.starts_with("download-")
            && file_name.ends_with(".ndjson")
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::cleanup_stale_snapshot_previews;

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
}

pub(crate) fn account_command_error(error: AccountError) -> crate::CommandError {
    crate::CommandError { code: error.code() }
}

/// Maps an account error from a community service to the `community_*` codes the community pages translate. It is the mapping `mobile_community` uses, so a page reads the same code on every host.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) fn community_error(error: AccountError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            AccountError::Invalid => "community_invalid",
            AccountError::Unauthorized => "community_unauthorized",
            AccountError::Forbidden => "community_forbidden",
            AccountError::NotFound => "community_not_found",
            AccountError::RateLimited => "community_rate_limited",
            AccountError::Cancelled => "community_cancelled",
            AccountError::Storage => "community_storage",
            AccountError::Conflict => "community_conflict",
            AccountError::Unavailable => "community_unavailable",
        },
    }
}

/// Parses a community item id the page sent.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) fn community_id(
    value: &str,
) -> Result<msime_client_core::uuid::Uuid, crate::CommandError> {
    msime_client_core::uuid::Uuid::parse_str(value).map_err(|_| crate::CommandError {
        code: "community_invalid",
    })
}

/// Runs one blocking community service call off the async runtime, mapping its error with [`community_error`].
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
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
