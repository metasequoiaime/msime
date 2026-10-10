use msime_client_core::account::AccountError;
#[cfg(any(target_os = "ios", target_os = "android", test))]
use msime_client_core::account::{AccountApi, AccountSessionStorage, BackendAccountSession};
#[cfg(any(target_os = "ios", target_os = "android", test))]
use std::path::Path;
use std::sync::Arc;
#[cfg(any(target_os = "ios", target_os = "android", test))]
use std::{collections::HashMap, sync::Mutex};

/// 在账户代次锁内发布预览，避免退出登录后旧下载重新写入预览表。
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn publish_snapshot_preview<A, S, V>(
    session: &BackendAccountSession<A, S>,
    generation: u64,
    account_id: &str,
    previews: &Mutex<HashMap<String, V>>,
    token: String,
    preview: V,
) -> Result<Vec<V>, AccountError>
where
    A: AccountApi,
    S: AccountSessionStorage,
{
    session.with_generation(generation, Some(account_id), || {
        let mut pending = previews.lock().map_err(|_| AccountError::Unavailable)?;
        let old = pending.drain().map(|(_, item)| item).collect();
        pending.insert(token, preview);
        Ok(old)
    })
}

/// 在账户锁内取走不属于当前代次的预览，避免退出后的清理误删新会话预览。
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) fn take_invalid_snapshot_previews<A, S, V, F>(
    session: &BackendAccountSession<A, S>,
    previews: &Mutex<HashMap<String, V>>,
    identity: F,
) -> Result<Vec<V>, AccountError>
where
    A: AccountApi,
    S: AccountSessionStorage,
    F: for<'a> Fn(&'a V) -> (&'a str, u64),
{
    session.with_current_identity(|current| {
        let mut pending = previews.lock().map_err(|_| AccountError::Unavailable)?;
        let (valid, invalid): (HashMap<_, _>, HashMap<_, _>) = std::mem::take(&mut *pending)
            .into_iter()
            .partition(|(_, preview)| {
                let (account_id, generation) = identity(preview);
                current == Some((account_id, generation))
            });
        *pending = valid;
        Ok(invalid.into_values().collect())
    })
}

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

/// Serializes an account response for the page. A value that cannot be serialized is reported as the service being unavailable, the code the page already handles for a failed call.
pub(crate) fn account_value<T: serde::Serialize>(
    value: T,
) -> Result<serde_json::Value, AccountError> {
    serde_json::to_value(value).map_err(|_| AccountError::Unavailable)
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
            AccountError::BlockedContent => "community_blocked_content",
            AccountError::ScreeningUnavailable => "community_screening_unavailable",
            AccountError::Banned => "community_account_banned",
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

#[cfg(test)]
mod tests {
    use super::{
        cleanup_stale_snapshot_previews, cleanup_stale_snapshot_previews_in_directory,
        prepare_snapshot_directory, publish_snapshot_preview, read_snapshot_file,
        take_invalid_snapshot_previews, write_snapshot_file,
    };

    #[test]
    fn snapshot_preview_published_after_logout_is_rejected() {
        use msime_client_core::account::{
            AccountError, AccountSessionStorage, AccountTokens, AccountUser, BackendAccountClient,
            BackendAccountSession, SavedAccountSession,
        };
        use std::collections::HashMap;
        use std::sync::Mutex;
        use std::time::{SystemTime, UNIX_EPOCH};

        struct MemoryStorage(Mutex<Option<SavedAccountSession>>);
        impl AccountSessionStorage for MemoryStorage {
            fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
                Ok(self.0.lock().map_err(|_| AccountError::Storage)?.clone())
            }
            fn save(&self, saved: &SavedAccountSession) -> Result<(), AccountError> {
                *self.0.lock().map_err(|_| AccountError::Storage)? = Some(saved.clone());
                Ok(())
            }
            fn clear(&self) -> Result<(), AccountError> {
                *self.0.lock().map_err(|_| AccountError::Storage)? = None;
                Ok(())
            }
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let saved = SavedAccountSession {
            tokens: AccountTokens {
                access_token: "a".repeat(64),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "synthetic-user".into(),
                    display_name: "合成用户".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: now + 3_600_000,
            session_id: None,
        };
        let session = BackendAccountSession::new(
            BackendAccountClient::new().unwrap(),
            MemoryStorage(Mutex::new(Some(saved))),
        );
        let (_, _, generation) = session
            .credentials_with_generation(None, Some("synthetic-user"))
            .unwrap();
        let previews = Mutex::new(HashMap::new());
        assert_eq!(
            publish_snapshot_preview(
                &session,
                generation,
                "synthetic-user",
                &previews,
                "first-preview".into(),
                3_u8,
            ),
            Ok(Vec::new())
        );
        assert_eq!(
            publish_snapshot_preview(
                &session,
                generation,
                "synthetic-user",
                &previews,
                "second-preview".into(),
                5_u8,
            ),
            Ok(vec![3_u8])
        );
        assert_eq!(
            previews.lock().unwrap().remove("second-preview"),
            Some(5_u8)
        );
        let mixed_previews = Mutex::new(HashMap::from([
            (
                "current".to_owned(),
                ("synthetic-user".to_owned(), generation),
            ),
            (
                "invalid".to_owned(),
                ("synthetic-user".to_owned(), generation + 1),
            ),
        ]));
        assert_eq!(
            take_invalid_snapshot_previews(&session, &mixed_previews, |preview| {
                (&preview.0, preview.1)
            }),
            Ok(vec![("synthetic-user".to_owned(), generation + 1)])
        );
        assert!(mixed_previews.lock().unwrap().contains_key("current"));
        assert!(!mixed_previews.lock().unwrap().contains_key("invalid"));
        session.forget().unwrap();

        assert_eq!(
            publish_snapshot_preview(
                &session,
                generation,
                "synthetic-user",
                &previews,
                "preview-token".into(),
                7_u8,
            ),
            Err(AccountError::Cancelled)
        );
        assert!(previews.lock().unwrap().is_empty());

        let stale_previews = Mutex::new(HashMap::from([(
            "stale-token".to_owned(),
            ("synthetic-user".to_owned(), generation),
        )]));
        assert_eq!(
            take_invalid_snapshot_previews(&session, &stale_previews, |preview| {
                (&preview.0, preview.1)
            }),
            Ok(vec![("synthetic-user".to_owned(), generation)])
        );
        assert!(stale_previews.lock().unwrap().is_empty());
    }

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
        use std::os::unix::fs::symlink;

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
