//! 单写方发布、接收沙箱只读的持续授权；共享读锁只包围本地操作，不包围网络。

use super::{PreferencesStore, UsageReportingConsent};
use crate::file_lock::{self, PrivateDirectory, ScopedFileLock};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const LOCK: &str = "authority.lock";
const DOCUMENT: &str = "authority.json";
const MAX_BYTES: u64 = 2048;

/// 暂停不等于撤回：读取失败后仍可保留旧队列，同源恢复时复用原代次。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportingAuthorization {
    Enabled(UsageReportingConsent),
    Disabled,
    Suspended,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    format: u8,
    source: Option<Uuid>,
    policy: String,
    consent: Option<UsageReportingConsent>,
    suspended: bool,
    source_seen: bool,
}

impl Document {
    fn status(&self) -> ReportingAuthorization {
        if self.suspended {
            ReportingAuthorization::Suspended
        } else {
            self.consent.map_or(
                ReportingAuthorization::Disabled,
                ReportingAuthorization::Enabled,
            )
        }
    }

    fn validate(&self) -> io::Result<()> {
        if self.format != 1
            || self.policy.len() != 64
            || !self.policy.bytes().all(|value| value.is_ascii_hexdigit())
            || self.source.is_some_and(|id| id.is_nil())
            || self.consent.is_some_and(|value| !value.valid())
            || self.consent.is_some() && (self.source.is_none() || !self.source_seen)
        {
            return Err(io::Error::other("invalid reporting authority"));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ReportingAuthority {
    directory: PathBuf,
}

impl ReportingAuthority {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// 接收方不建目录和锁；锁忙、缺失、损坏都返回错误，不能自行初始化授权。
    pub fn with_current<T>(
        &self,
        operation: impl FnOnce(ReportingAuthorization) -> T,
    ) -> io::Result<T> {
        self.with_document(|document| operation(document.status()))
    }

    fn with_document<T>(&self, operation: impl FnOnce(Document) -> T) -> io::Result<T> {
        let directory = file_lock::open_private_directory(&self.directory)?;
        let file = file_lock::open_private_file_at(&directory, OsStr::new(LOCK))?;
        if !file_lock::try_shared(&file)? {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        let _lock = ScopedFileLock::from_locked(file);
        let document = Self::read(&directory)?;
        Ok(operation(document))
    }

    /// 仅由权威设置进程调用。政策身份由持久化的政策决定生成，不含用户数据。
    /// 源偏好锁在共享授权锁之前取得；源暂时不可读时发布暂停，禁止沿用旧许可发送。
    pub fn publish_from(
        &self,
        source: &PreferencesStore,
        policy_allowed: bool,
        policy_identity: &str,
    ) -> io::Result<ReportingAuthorization> {
        if policy_identity.len() > 1024 {
            return Err(io::Error::other("reporting policy identity is too large"));
        }
        let policy = hex::encode(Sha256::digest(policy_identity.as_bytes()));
        // 已观察的源文件消失时不能被默认值重新授权；政策尚未同意的首次发布不算已观察源。
        let missing = file_lock::open_private_file_in(source.directory().join("preferences.json"))
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound);
        if !policy_allowed {
            return self.publish(None, &policy, false, !missing);
        }
        let seen = match self.with_document(|document| document.source_seen) {
            Ok(seen) => seen,
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(_) => true,
        };
        if missing && seen {
            return self.publish(None, &policy, true, false);
        }
        if source.usage_reporting_consent().is_err() {
            return self.publish(None, &policy, true, !missing);
        }
        match source.with_usage_reporting_consent(|enabled, consent| {
            self.publish(
                if enabled { consent } else { None },
                &policy,
                enabled && consent.is_none(),
                true,
            )
        }) {
            Ok(result) => result,
            Err(_) => self.publish(None, &policy, true, !missing),
        }
    }

    fn publish(
        &self,
        source: Option<UsageReportingConsent>,
        policy: &str,
        suspended: bool,
        source_seen: bool,
    ) -> io::Result<ReportingAuthorization> {
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(io::Error::other(
                "reporting authority directory is not a directory",
            ));
        }
        let directory = file_lock::open_private_directory(&self.directory)?;
        let _lock = ScopedFileLock::exclusive(file_lock::open_private_lock_file_at(
            &directory,
            OsStr::new(LOCK),
        )?)?;
        let previous = match Self::read(&directory) {
            Ok(document) => Some(document),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            // 损坏不能变成默认开启；下一份有效输入可以明确重建授权，但不继承旧身份。
            Err(error) if error.kind() == io::ErrorKind::InvalidData => None,
            Err(error) => return Err(error),
        };
        let mut document = if suspended {
            match previous.clone() {
                Some(mut document) => {
                    document.suspended = true;
                    if document.policy != policy {
                        document.policy = policy.to_owned();
                        document.source = None;
                        document.consent = None;
                    }
                    document
                }
                None => Document {
                    format: 1,
                    source: None,
                    policy: policy.to_owned(),
                    consent: None,
                    suspended: true,
                    source_seen,
                },
            }
        } else {
            let consent = match source {
                Some(source) => match previous.as_ref().filter(|previous| {
                    previous.source == Some(source.epoch) && previous.policy == policy
                }) {
                    Some(previous) if previous.consent.is_some() => previous.consent,
                    _ => {
                        let mut consent = UsageReportingConsent::new().map_err(io::Error::other)?;
                        consent.since_unix_ms = consent.since_unix_ms.max(source.since_unix_ms);
                        Some(consent)
                    }
                },
                None => None,
            };
            Document {
                format: 1,
                source: source.map(|value| value.epoch),
                policy: policy.to_owned(),
                consent,
                suspended: false,
                source_seen,
            }
        };
        document.source_seen |= previous
            .as_ref()
            .is_some_and(|document| document.source_seen);
        document.validate()?;
        let status = document.status();
        if previous.as_ref() == Some(&document) {
            return Ok(status);
        }
        let bytes = serde_json::to_vec(&document).map_err(io::Error::other)?;
        file_lock::write_private_file_at(&directory, OsStr::new(DOCUMENT), &bytes)?;
        directory.sync_all()?;
        Ok(status)
    }

    fn read(directory: &PrivateDirectory) -> io::Result<Document> {
        let file = file_lock::open_private_file_at(directory, OsStr::new(DOCUMENT))?;
        if file.metadata()?.len() > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "reporting authority is too large",
            ));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "reporting authority is too large",
            ));
        }
        let document: Document = serde_json::from_slice(&bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid reporting authority")
        })?;
        document.validate().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid reporting authority")
        })?;
        Ok(document)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;

    fn enabled(status: ReportingAuthorization) -> UsageReportingConsent {
        match status {
            ReportingAuthorization::Enabled(consent) => consent,
            other => panic!("expected enabled, got {other:?}"),
        }
    }

    fn save_switch(source: &PreferencesStore, value: bool) {
        let current = source.load().unwrap();
        let mut preferences = current.preferences;
        preferences.usage_reporting = value;
        source.save(current.revision, preferences).unwrap();
    }

    #[test]
    fn reporting_authority_first_policy_denial_does_not_prevent_later_initialization() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        assert_eq!(
            authority.publish_from(&source, false, "policy").unwrap(),
            ReportingAuthorization::Disabled
        );
        assert!(!source.directory().join("preferences.json").exists());
        let first = enabled(authority.publish_from(&source, true, "policy").unwrap());
        authority.publish_from(&source, false, "policy").unwrap();
        let path = source.directory().join("preferences.json");
        let saved = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(
            authority.publish_from(&source, true, "policy").unwrap(),
            ReportingAuthorization::Suspended
        );
        assert!(!path.exists());
        fs::write(&path, saved).unwrap();
        assert_ne!(
            enabled(authority.publish_from(&source, true, "policy").unwrap()).epoch,
            first.epoch
        );
    }

    #[test]
    fn reporting_authority_preserves_restart_and_detects_unobserved_source_cycle() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        let first = enabled(authority.publish_from(&source, true, "policy-one").unwrap());
        let restarted = ReportingAuthority::new(authority.directory());
        assert_eq!(
            enabled(restarted.with_current(|value| value).unwrap()),
            first
        );
        assert_eq!(
            enabled(authority.publish_from(&source, true, "policy-one").unwrap()),
            first
        );
        save_switch(&source, false);
        save_switch(&source, true);
        let second = enabled(authority.publish_from(&source, true, "policy-one").unwrap());
        assert_ne!(first.epoch, second.epoch);
        assert!(second.since_unix_ms >= first.since_unix_ms);
    }

    #[test]
    fn reporting_authority_policy_revoke_and_regrant_changes_epoch_without_changing_switch() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        let first = enabled(
            authority
                .publish_from(&source, true, "same-policy")
                .unwrap(),
        );
        assert_eq!(
            authority
                .publish_from(&source, false, "same-policy")
                .unwrap(),
            ReportingAuthorization::Disabled
        );
        let second = enabled(
            authority
                .publish_from(&source, true, "same-policy")
                .unwrap(),
        );
        assert_ne!(first.epoch, second.epoch);
        assert!(source.load().unwrap().preferences.usage_reporting);
        let third = enabled(authority.publish_from(&source, true, "new-policy").unwrap());
        assert_ne!(second.epoch, third.epoch);
    }

    #[test]
    fn reporting_authority_unreadable_source_suspends_without_granting_or_losing_identity() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        let first = enabled(authority.publish_from(&source, true, "policy").unwrap());
        let path = source.directory().join("preferences.json");
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"broken").unwrap();
        assert_eq!(
            authority.publish_from(&source, true, "policy").unwrap(),
            ReportingAuthorization::Suspended
        );
        assert_eq!(
            authority.with_current(|value| value).unwrap(),
            ReportingAuthorization::Suspended
        );
        fs::remove_file(&path).unwrap();
        assert_eq!(
            authority.publish_from(&source, true, "policy").unwrap(),
            ReportingAuthorization::Suspended
        );
        assert!(!path.exists());
        fs::write(&path, original).unwrap();
        assert_eq!(
            enabled(authority.publish_from(&source, true, "policy").unwrap()),
            first
        );
        fs::write(&path, b"broken").unwrap();
        assert_eq!(
            authority.publish_from(&source, false, "policy").unwrap(),
            ReportingAuthorization::Disabled
        );
    }

    #[test]
    fn reporting_authority_policy_change_during_pause_cannot_restore_an_old_grant() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        let first = enabled(
            authority
                .publish_from(&source, true, "first-policy")
                .unwrap(),
        );
        let path = source.directory().join("preferences.json");
        let saved = fs::read(&path).unwrap();
        fs::write(&path, b"broken").unwrap();
        authority
            .publish_from(&source, true, "second-policy")
            .unwrap();
        fs::write(&path, saved).unwrap();
        assert_ne!(
            enabled(
                authority
                    .publish_from(&source, true, "first-policy")
                    .unwrap()
            )
            .epoch,
            first.epoch
        );
    }

    #[cfg(unix)]
    #[test]
    fn reporting_authority_refuses_linked_lock_document_and_directory() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        authority.publish_from(&source, true, "policy").unwrap();
        for name in [LOCK, DOCUMENT] {
            let path = authority.directory().join(name);
            let saved = fs::read(&path).unwrap();
            let outside = root.path().join(format!("outside-{name}"));
            fs::write(&outside, &saved).unwrap();
            fs::remove_file(&path).unwrap();
            symlink(&outside, &path).unwrap();
            assert!(authority.with_current(|value| value).is_err());
            assert!(authority.publish_from(&source, true, "policy").is_err());
            assert_eq!(fs::read(&outside).unwrap(), saved);
            fs::remove_file(&path).unwrap();
            fs::write(&path, saved).unwrap();
        }
        let linked = root.path().join("linked-authority");
        symlink(authority.directory(), &linked).unwrap();
        assert!(ReportingAuthority::new(linked)
            .with_current(|value| value)
            .is_err());
    }

    #[test]
    fn reporting_authority_busy_missing_and_corrupt_remain_errors() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        assert!(authority.with_current(|value| value).is_err());
        assert!(!authority.directory().exists());
        authority.publish_from(&source, true, "policy").unwrap();
        let lock = ScopedFileLock::exclusive(
            file_lock::open_private_lock_file(authority.directory().join(LOCK)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            authority.with_current(|value| value).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        drop(lock);
        fs::write(authority.directory().join(DOCUMENT), b"{}").unwrap();
        assert_eq!(
            authority.with_current(|value| value).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        // 明确发布有效源才允许重建；接收方永远不能修复或自授权。
        authority.publish_from(&source, true, "policy").unwrap();
        assert!(matches!(
            authority.with_current(|value| value).unwrap(),
            ReportingAuthorization::Enabled(_)
        ));
    }

    #[test]
    fn reporting_authority_read_operation_serializes_publication() {
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        authority.publish_from(&source, true, "policy").unwrap();
        let (entered, observed) = mpsc::channel();
        let (completed, finished) = mpsc::channel();
        let worker_authority = authority.clone();
        let writer = authority
            .with_current(|_| {
                let writer = std::thread::spawn(move || {
                    entered.send(()).unwrap();
                    worker_authority
                        .publish_from(&source, false, "policy")
                        .unwrap();
                    completed.send(()).unwrap();
                });
                observed.recv().unwrap();
                assert!(finished.recv_timeout(Duration::from_millis(100)).is_err());
                writer
            })
            .unwrap();
        finished.recv_timeout(Duration::from_secs(3)).unwrap();
        writer.join().unwrap();
        assert_eq!(
            authority.with_current(|value| value).unwrap(),
            ReportingAuthorization::Disabled
        );
    }

    #[cfg(unix)]
    #[test]
    fn reporting_authority_reads_real_readonly_files_without_creating_anything() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let source = PreferencesStore::new(root.path().join("source"));
        let authority = ReportingAuthority::new(root.path().join("authority"));
        let first = enabled(authority.publish_from(&source, true, "policy").unwrap());
        for name in [LOCK, DOCUMENT] {
            fs::set_permissions(
                authority.directory().join(name),
                fs::Permissions::from_mode(0o400),
            )
            .unwrap();
        }
        fs::set_permissions(authority.directory(), fs::Permissions::from_mode(0o500)).unwrap();
        let result = authority.with_current(|value| value);
        fs::set_permissions(authority.directory(), fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(enabled(result.unwrap()), first);
        let mut entries = fs::read_dir(authority.directory())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        entries.sort();
        assert_eq!(entries, [OsStr::new(DOCUMENT), OsStr::new(LOCK)]);
    }
}
