//! 完整云词库快照的预览表和临时文件工具。
//!
//! 预览表的两个函数只经会话，与 Tauri iOS、Android 共用；文件工具给 [`super::full_backup`] 用，放在本用户私有的临时目录里，打开时不跟随符号链接。Tauri iOS 和 Android 仍用它们自己那一套同名工具，平台迁移时再换过来。

use msime_client_core::file_lock;
use msime_client_services::account::{
    AccountApi, AccountError, AccountSessionStorage, BackendAccountSession,
};
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// 原生快照检查接受的最大字节数，与 `inspect_snapshot_json` 相同。
pub const MAX_SNAPSHOT_BYTES: usize = 512 * 1024 * 1024;
/// [`copy_snapshot_file`] 写在保存位置旁边的临时文件的前缀。
const EXPORT_COPY_PREFIX: &str = ".msime-export-";
/// 保存位置旁边的临时文件超过这么久没有改动，就是上次复制到一半时进程被杀留下的。正在复制的那份一直在写，修改时间总是新的。
const STALE_EXPORT_COPY_AGE: Duration = Duration::from_secs(60 * 60);

/// 在账户代次锁内发布预览，避免退出登录后旧下载重新写入预览表。返回被这次发布替换掉的预览，调用方删掉它们的文件。
pub fn publish_snapshot_preview<A, S, V>(
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
pub fn take_invalid_snapshot_previews<A, S, V, F>(
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

/// 建快照临时目录，路径上每一级都必须是真实目录（受信任的系统别名除外）。宿主会把这个目录里的路径交给原生桥接，跟随一个被换成符号链接的临时目录会把云端数据写到别处。
pub fn prepare_snapshot_directory(directory: &Path) -> io::Result<()> {
    msime_path_trust::reject_symlinked_components(directory)?;
    std::fs::create_dir_all(directory)?;
    // 建目录期间路径可能被换掉，建完再看一次。
    let metadata = std::fs::symlink_metadata(directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "snapshot directory is not a real directory",
        ));
    }
    msime_path_trust::reject_symlinked_components(directory)
}

/// 先完整写入并同步一个私有的临时兄弟文件，再替换目标路径；目标是符号链接时替换链接本身，不跟随它。
pub fn write_snapshot_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    prepare_snapshot_directory(parent)?;
    file_lock::replace_private_file(path, contents)
}

/// 经不跟随符号链接的句柄读暂存的快照，最多 [`MAX_SNAPSHOT_BYTES`]，必须是 UTF-8。
pub fn read_snapshot_file(path: &Path) -> io::Result<String> {
    let file = file_lock::open_private_file_in(path)?;
    let bytes = crate::bounded_file::read(file, MAX_SNAPSHOT_BYTES as u64)?;
    String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "snapshot is not UTF-8"))
}

/// 把临时目录里校验过的快照复制到宿主选的保存位置 `destination`：先在目标旁边写完并同步一个私有的临时文件，再换入目标路径，中途任何一步失败，目标上原有的文件都原样保留。进程在复制中途被杀时，目标旁边会留下一份 `.msime-export-*`，里面是整份云词库的明文；它在宿主选的目录里，启动时清理的 `snapshot_directory` 碰不到它，所以每次往一个目录复制之前，先删掉这个目录里超过一小时没有改动的那些。源文件经不跟随符号链接的句柄读，最多 [`MAX_SNAPSHOT_BYTES`]；目标的各级上级目录不能是符号链接（受信任的系统别名除外），目标本身是符号链接时替换链接本身，不跟随它。临时目录和保存位置可能在不同的卷上，所以是复制而不是改名。
pub fn copy_snapshot_file(source: &Path, destination: &Path) -> io::Result<()> {
    let parent = destination
        .parent()
        .filter(|parent| parent.is_absolute())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent"))?;
    msime_path_trust::reject_symlinked_components(parent)?;
    // 清理失败不影响这次复制：留下的文件下次再删。
    let now = SystemTime::now();
    let _ = remove_entries(parent, |name, metadata| {
        name.starts_with(EXPORT_COPY_PREFIX)
            && metadata.modified().is_ok_and(|modified| {
                now.duration_since(modified)
                    .is_ok_and(|age| age >= STALE_EXPORT_COPY_AGE)
            })
    });
    let input = file_lock::open_private_file_in(source)?;
    let mut temporary = tempfile::Builder::new()
        .prefix(EXPORT_COPY_PREFIX)
        .tempfile_in(parent)?;
    let copied = io::copy(
        &mut io::Read::take(input, MAX_SNAPSHOT_BYTES as u64 + 1),
        temporary.as_file_mut(),
    )?;
    if copied > MAX_SNAPSHOT_BYTES as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot is too large",
        ));
    }
    temporary.as_file().sync_all()?;
    temporary
        .persist(destination)
        .map(|_| ())
        .map_err(|error| error.error)
}

/// 相对于已打开的父目录删除文件，父目录的路径同时被换掉也不会删到别处。
pub fn remove_snapshot_file(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "snapshot file has no name"))?;
    let directory = file_lock::open_private_directory(parent)?;
    file_lock::remove_private_file_at(&directory, name)
}

/// 删掉上次运行留下的预览下载（`download-*.ndjson`）。目录本身是符号链接时什么也不删。列目录和删除都经同一个已打开的目录句柄。
pub fn cleanup_stale_snapshot_previews(directory: &Path) -> io::Result<()> {
    remove_entries(directory, |name, _| {
        name.starts_with("download-") && name.ends_with(".ndjson")
    })
}

/// 删掉上次运行被打断时留下的导出、恢复中转文件（`export-*.ndjson`、`restore-*.ndjson`）和原子写入没来得及换入的临时文件（Windows 上的 `.tmp*`，Unix 上的 `.msime-private-*`）。导出的中转文件是用户整份云词库的明文，不能一直留着。
pub fn cleanup_stale_scratch_files(directory: &Path) -> io::Result<()> {
    remove_entries(directory, |name, _| {
        ((name.starts_with("export-") || name.starts_with("restore-")) && name.ends_with(".ndjson"))
            || name.starts_with(".tmp")
            || name.starts_with(".msime-private-")
    })
}

/// 删掉 `directory` 里名字和元数据满足 `matches` 的普通文件。
fn remove_entries(
    directory: &Path,
    matches: impl Fn(&str, &std::fs::Metadata) -> bool,
) -> io::Result<()> {
    let opened = match file_lock::open_private_directory(directory) {
        Ok(opened) => opened,
        Err(_) if msime_path_trust::reject_symlinked_components(directory).is_err() => {
            return Ok(())
        }
        Err(error) => return Err(error),
    };
    for name in file_lock::read_private_directory(&opened)? {
        let Some(text) = name.to_str() else {
            continue;
        };
        // 只删普通文件，同名的目录或符号链接留着；删除本身经已打开的目录句柄，不会删到别处。
        let remove = std::fs::symlink_metadata(directory.join(&name))
            .is_ok_and(|metadata| metadata.file_type().is_file() && matches(text, &metadata));
        if remove {
            let _ = file_lock::remove_private_file_at(&opened, &name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_preview_published_after_logout_is_rejected() {
        use msime_client_services::account::{
            AccountTokens, AccountUser, BackendAccountClient, SavedAccountSession,
        };
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
        std::fs::create_dir(directory.path().join("download-directory.ndjson")).unwrap();

        cleanup_stale_snapshot_previews(directory.path()).unwrap();

        assert!(!directory.path().join("download-old.ndjson").exists());
        assert!(directory.path().join("download-in-progress").exists());
        assert!(directory.path().join("export-old.ndjson").exists());
        assert!(directory.path().join("download-directory.ndjson").is_dir());
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
    fn stale_snapshot_cleanup_never_deletes_through_a_symlinked_leaf() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("target.ndjson");
        std::fs::write(&outside_file, b"synthetic-outside").unwrap();
        let root = tempfile::tempdir().unwrap();
        symlink(&outside_file, root.path().join("download-link.ndjson")).unwrap();

        cleanup_stale_snapshot_previews(root.path()).unwrap();

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

    #[test]
    fn snapshot_files_round_trip_and_reject_non_utf8() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("nested").join("restore-a.ndjson");
        write_snapshot_file(&path, "合成快照\n".as_bytes()).unwrap();
        assert_eq!(read_snapshot_file(&path).unwrap(), "合成快照\n");
        write_snapshot_file(&path, &[0xff, 0xfe]).unwrap();
        assert!(read_snapshot_file(&path).is_err());
        remove_snapshot_file(&path).unwrap();
        assert!(!path.exists());
    }
}
