//! Android 旧版队列状态 `state.bin` 的只读解码。
//!
//! 2026-10 以前 Android 的快照激活队列是 Java 的 `DictionarySnapshotQueue`（`platforms/android/java/app/msime/android/dictionary/DictionarySnapshotQueue.java`），状态由 `writeState` 经 `DataOutputStream` 写成大端序二进制：`int` 魔数 `0x4d535131`（`MSQ1`）、`int` 版本 1、`boolean` 有无本机版本、有时 `writeUTF` 本机版本、`boolean` 有无请求，有请求时依次是 `long` UUID 高 64 位、`long` 低 64 位、`writeUTF` 账号、`writeUTF` 期望的本机版本、`writeUTF` 文件 SHA-256、`long` 云端修订号、`byte` 状态序号（`QUEUED`、`PREPARING`、`APPLIED`、`CONFLICT`、`FAILED`、`CANCELLED` 依次是 0–5），后面不能再有字节，整份最多 65536 字节。`writeUTF` 是两字节长度加 Java 的 modified UTF-8：U+0000 写成两字节，补充平面的字符拆成两个代理项、各写三字节。快照文件与 Rust 同名（`<id>.ndjson`），转换时不动。

use super::{
    SnapshotQueueError, SnapshotQueueState, SnapshotRequest, SnapshotRequestStatus,
    MAXIMUM_STATE_BYTES,
};
use std::ffi::OsStr;
use uuid::Uuid;

pub(super) const STATE_NAME: &str = "state.bin";
const MAGIC: u32 = 0x4d53_5131;
const FORMAT_VERSION: u32 = 1;

/// 读出目录里 Android 旧版的状态，没有 `state.bin` 时是 `None`。不改动任何文件；读不懂（截断、魔数或版本不对、字段不合法、多出字节）是 `Invalid`。
pub(super) fn read(
    directory: &crate::file_lock::PrivateDirectory,
) -> Result<Option<SnapshotQueueState>, SnapshotQueueError> {
    let file = match crate::file_lock::open_private_file_at(directory, OsStr::new(STATE_NAME)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
            return Err(SnapshotQueueError::Invalid);
        }
        Err(_) => return Err(SnapshotQueueError::Unavailable),
    };
    let metadata = file
        .metadata()
        .map_err(|_| SnapshotQueueError::Unavailable)?;
    if metadata.len() == 0 || metadata.len() > MAXIMUM_STATE_BYTES {
        return Err(SnapshotQueueError::Invalid);
    }
    let bytes = crate::bounded_io::read_bounded_file_with(
        file,
        MAXIMUM_STATE_BYTES,
        || SnapshotQueueError::Invalid,
        |_| SnapshotQueueError::Unavailable,
    )?;
    decode(&bytes).map(Some)
}

/// 按 Java `readUnlocked` 的顺序解码，再用 Rust 状态自己的规则校验一遍。
pub(super) fn decode(bytes: &[u8]) -> Result<SnapshotQueueState, SnapshotQueueError> {
    let mut input = Reader { remaining: bytes };
    if input.u32()? != MAGIC || input.u32()? != FORMAT_VERSION {
        return Err(SnapshotQueueError::Invalid);
    }
    let local_version = if input.boolean()? {
        Some(input.utf()?)
    } else {
        None
    };
    let request = if input.boolean()? {
        Some(request(&mut input)?)
    } else {
        None
    };
    if !input.remaining.is_empty() {
        return Err(SnapshotQueueError::Invalid);
    }
    let state = SnapshotQueueState {
        version: 1,
        local_version,
        request,
    };
    state.validate()?;
    Ok(state)
}

fn request(input: &mut Reader<'_>) -> Result<SnapshotRequest, SnapshotQueueError> {
    let most_significant = input.u64()?;
    let least_significant = input.u64()?;
    let account_id = input.utf()?;
    let expected_local_version = input.utf()?;
    let file_sha256 = input.utf()?;
    let cloud_revision = input.i64()?;
    let status = match input.u8()? {
        0 => SnapshotRequestStatus::Queued,
        1 => SnapshotRequestStatus::Preparing,
        2 => SnapshotRequestStatus::Applied,
        3 => SnapshotRequestStatus::Conflict,
        4 => SnapshotRequestStatus::Failed,
        5 => SnapshotRequestStatus::Cancelled,
        _ => return Err(SnapshotQueueError::Invalid),
    };
    Ok(SnapshotRequest {
        id: Uuid::from_u64_pair(most_significant, least_significant),
        account_id,
        cloud_revision,
        expected_local_version,
        file_sha256,
        status,
    })
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], SnapshotQueueError> {
        let (head, tail) = self
            .remaining
            .split_first_chunk::<N>()
            .ok_or(SnapshotQueueError::Invalid)?;
        self.remaining = tail;
        Ok(*head)
    }

    fn u8(&mut self) -> Result<u8, SnapshotQueueError> {
        Ok(self.take::<1>()?[0])
    }

    /// 与 `DataInputStream.readBoolean` 相同：非零即真。
    fn boolean(&mut self) -> Result<bool, SnapshotQueueError> {
        Ok(self.u8()? != 0)
    }

    fn u32(&mut self) -> Result<u32, SnapshotQueueError> {
        Ok(u32::from_be_bytes(self.take()?))
    }

    fn u64(&mut self) -> Result<u64, SnapshotQueueError> {
        Ok(u64::from_be_bytes(self.take()?))
    }

    fn i64(&mut self) -> Result<i64, SnapshotQueueError> {
        Ok(i64::from_be_bytes(self.take()?))
    }

    fn utf(&mut self) -> Result<String, SnapshotQueueError> {
        let length = usize::from(u16::from_be_bytes(self.take()?));
        if self.remaining.len() < length {
            return Err(SnapshotQueueError::Invalid);
        }
        let (text, tail) = self.remaining.split_at(length);
        self.remaining = tail;
        modified_utf8(text)
    }
}

/// `DataInputStream.readUTF` 的解码规则：一字节 `0xxxxxxx`、两字节 `110xxxxx 10xxxxxx`、三字节 `1110xxxx 10xxxxxx 10xxxxxx` 各得一个 UTF-16 码元，其余首字节和不是 `10` 开头的后续字节都不合法。拼出的码元里有落单的代理项时 Rust 的 `String` 装不下，按不合法处理。
fn modified_utf8(bytes: &[u8]) -> Result<String, SnapshotQueueError> {
    let continuation = |index: usize| match bytes.get(index) {
        Some(&byte) if byte & 0xc0 == 0x80 => Ok(u16::from(byte & 0x3f)),
        _ => Err(SnapshotQueueError::Invalid),
    };
    let mut units = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while let Some(&lead) = bytes.get(index) {
        let (unit, width) = match lead >> 4 {
            0..=7 => (u16::from(lead), 1),
            12 | 13 => ((u16::from(lead & 0x1f) << 6) | continuation(index + 1)?, 2),
            14 => (
                (u16::from(lead & 0x0f) << 12)
                    | (continuation(index + 1)? << 6)
                    | continuation(index + 2)?,
                3,
            ),
            _ => return Err(SnapshotQueueError::Invalid),
        };
        units.push(unit);
        index += width;
    }
    String::from_utf16(&units).map_err(|_| SnapshotQueueError::Invalid)
}

#[cfg(test)]
mod tests;
