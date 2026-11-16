//! 按登录、密钥与来源分隔的密文待合并文件；所有操作绑定私有目录并持跨进程锁。

use crate::{file_lock, storage};
use msime_client_core::cloud::learning_e2e::MAX_ENVELOPE_BYTES;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;

const MAGIC: &[u8] = b"MSLQ1\0";
const HEADER_BYTES: usize = 15;
const MAX_SOURCES: usize = 20;
pub(super) const MAX_ATTEMPTS: u8 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PendingError {
    #[error("learning pending storage unavailable")]
    Unavailable,
    #[error("invalid learning pending data")]
    Invalid,
    #[error("learning pending queue full")]
    Full,
}

impl From<std::io::Error> for PendingError {
    fn from(_: std::io::Error) -> Self {
        Self::Unavailable
    }
}

// 不实现 Debug，密文不进入结果或日志。
pub(super) struct Entry {
    pub generation: u64,
    pub attempts: u8,
    pub envelope: Vec<u8>,
}

pub(super) struct Pending {
    directory: file_lock::PrivateDirectory,
    _lock: file_lock::ScopedFileLock,
    binding: String,
}

impl Pending {
    /// 调用方已经核对当前登录与完整密钥串；旧绑定只清理缓存，永不清理进度屏障。
    pub fn open(state: &Path, binding: String) -> Result<Self, PendingError> {
        if !crate::text::is_lower_hex(&binding, 64) {
            return Err(PendingError::Invalid);
        }
        let cloud = state.join("cloud-sync");
        if !storage::create_directory_and_check(&cloud)? {
            return Err(PendingError::Unavailable);
        }
        let parent = file_lock::open_private_directory(&cloud)?;
        match file_lock::create_private_directory_at(&parent, OsStr::new("learning-pending")) {
            Ok(()) => parent.sync_all()?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let directory =
            file_lock::open_private_directory_at(&parent, OsStr::new("learning-pending"))?;
        let lock = file_lock::open_private_lock_file_at(&directory, OsStr::new("queue.lock"))?;
        let held = file_lock::ScopedFileLock::exclusive(lock)?;
        let queue = Self {
            directory,
            _lock: held,
            binding,
        };
        for name in file_lock::read_private_directory(&queue.directory)? {
            let text = name.to_str().ok_or(PendingError::Invalid)?;
            if text == "queue.lock" {
                continue;
            }
            // 没有写入者能越过队列锁；崩溃留下的原子写临时文件可以安全丢弃。
            if text.starts_with(".msime-private-") {
                file_lock::remove_private_file_at(&queue.directory, &name)?;
                continue;
            }
            let (binding, _) = parse_name(text).ok_or(PendingError::Invalid)?;
            if binding != queue.binding {
                file_lock::remove_private_file_at(&queue.directory, &name)?;
            }
        }
        queue.directory.sync_all()?;
        queue.sources()?;
        Ok(queue)
    }

    fn name(&self, source: &str, claimed: bool) -> String {
        format!(
            "{}.{source}.{}",
            self.binding,
            if claimed { "claimed" } else { "pending" }
        )
    }

    pub fn sources(&self) -> Result<BTreeSet<String>, PendingError> {
        let mut sources = BTreeSet::new();
        for name in file_lock::read_private_directory(&self.directory)? {
            let text = name.to_str().ok_or(PendingError::Invalid)?;
            if text == "queue.lock" {
                continue;
            }
            let (binding, source) = parse_name(text).ok_or(PendingError::Invalid)?;
            if binding != self.binding {
                return Err(PendingError::Invalid);
            }
            sources.insert(source.to_owned());
            if sources.len() > MAX_SOURCES {
                return Err(PendingError::Full);
            }
        }
        Ok(sources)
    }

    /// 观察时间和 verified 必须先持久化。相同代次不会覆盖已经认领的重试次数。
    pub fn publish(
        &self,
        source: &str,
        generation: u64,
        envelope: &[u8],
    ) -> Result<(), PendingError> {
        if !crate::text::is_lower_hex(source, 32)
            || !(1..=msime_client_core::cloud::learning_e2e::MAX_GENERATION).contains(&generation)
            || !(84..=MAX_ENVELOPE_BYTES).contains(&envelope.len())
        {
            return Err(PendingError::Invalid);
        }
        let sources = self.sources()?;
        if !sources.contains(source) && sources.len() == MAX_SOURCES {
            return Err(PendingError::Full);
        }
        for claimed in [true, false] {
            if let Some(old) = self.read(source, claimed)? {
                if old.generation > generation {
                    return Err(PendingError::Invalid);
                }
                if old.generation == generation {
                    return Ok(());
                }
            }
        }
        self.write(
            source,
            false,
            &Entry {
                generation,
                attempts: 0,
                envelope: envelope.to_vec(),
            },
        )
    }

    pub fn claim(&self, source: &str) -> Result<Option<Entry>, PendingError> {
        if let Some(entry) = self.read(source, true)? {
            return Ok(Some(entry));
        }
        match file_lock::open_private_file_at(
            &self.directory,
            OsStr::new(&self.name(source, false)),
        ) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
        file_lock::rename_private_entry(
            &self.directory,
            OsStr::new(&self.name(source, false)),
            &self.directory,
            OsStr::new(&self.name(source, true)),
        )?;
        self.directory.sync_all()?;
        self.read(source, true)
    }

    pub fn started(&self, source: &str, entry: &mut Entry) -> Result<(), PendingError> {
        entry.attempts = entry.attempts.checked_add(1).ok_or(PendingError::Invalid)?;
        self.write(source, true, entry)
    }

    pub fn discard(&self, source: &str) -> Result<(), PendingError> {
        file_lock::remove_private_file_at(&self.directory, OsStr::new(&self.name(source, true)))?;
        self.directory.sync_all()?;
        Ok(())
    }

    fn read(&self, source: &str, claimed: bool) -> Result<Option<Entry>, PendingError> {
        let file = match file_lock::open_private_file_at(
            &self.directory,
            OsStr::new(&self.name(source, claimed)),
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take((MAX_ENVELOPE_BYTES + HEADER_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() < HEADER_BYTES + 84
            || bytes.len() > MAX_ENVELOPE_BYTES + HEADER_BYTES
            || !bytes.starts_with(MAGIC)
        {
            return Err(PendingError::Invalid);
        }
        let attempts = bytes[6];
        let generation =
            u64::from_le_bytes(bytes[7..15].try_into().map_err(|_| PendingError::Invalid)?);
        if attempts > MAX_ATTEMPTS
            || !(1..=msime_client_core::cloud::learning_e2e::MAX_GENERATION).contains(&generation)
        {
            return Err(PendingError::Invalid);
        }
        Ok(Some(Entry {
            generation,
            attempts,
            envelope: bytes.split_off(HEADER_BYTES),
        }))
    }

    fn write(&self, source: &str, claimed: bool, entry: &Entry) -> Result<(), PendingError> {
        if !(1..=msime_client_core::cloud::learning_e2e::MAX_GENERATION).contains(&entry.generation)
            || entry.attempts > MAX_ATTEMPTS
            || !(84..=MAX_ENVELOPE_BYTES).contains(&entry.envelope.len())
        {
            return Err(PendingError::Invalid);
        }
        let mut bytes = Vec::with_capacity(HEADER_BYTES + entry.envelope.len());
        bytes.extend_from_slice(MAGIC);
        bytes.push(entry.attempts);
        bytes.extend_from_slice(&entry.generation.to_le_bytes());
        bytes.extend_from_slice(&entry.envelope);
        file_lock::write_private_file_at(
            &self.directory,
            OsStr::new(&self.name(source, claimed)),
            &bytes,
        )?;
        self.directory.sync_all()?;
        Ok(())
    }
}

fn parse_name(name: &str) -> Option<(&str, &str)> {
    let mut parts = name.split('.');
    let binding = parts.next()?;
    let source = parts.next()?;
    let suffix = parts.next()?;
    (parts.next().is_none()
        && crate::text::is_lower_hex(binding, 64)
        && crate::text::is_lower_hex(source, 32)
        && matches!(suffix, "pending" | "claimed"))
    .then_some((binding, source))
}

#[cfg(test)]
mod tests;
