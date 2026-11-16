//! 设置事件的序号和父哈希契约。

use super::*;
use sha2::{Digest, Sha256};

/// WebDAV 上追加写入的设置事件。事件正文在交给传输层前还应由独立的设置同步
/// 密钥加密；这里只定义不可回退的顺序和哈希链接，避免把明文事件误当成日志协议。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsSyncEvent {
    pub format: String,
    pub version: u64,
    pub sequence: u64,
    /// 设备公钥摘要（32 个小写十六进制字符）。
    pub device_id: String,
    /// 创世事件为空，后续事件必须是前一事件的 SHA-256 摘要。
    pub parent: String,
    pub document: SettingsSyncDocument,
}

impl SettingsSyncEvent {
    pub fn new(
        sequence: u64,
        device_id: impl Into<String>,
        parent: impl Into<String>,
        document: SettingsSyncDocument,
    ) -> Result<Self, SettingsSyncError> {
        let event = Self {
            format: EVENT_FORMAT.into(),
            version: EVENT_VERSION,
            sequence,
            device_id: device_id.into(),
            parent: parent.into(),
            document,
        };
        event.validate()
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, SettingsSyncError> {
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        let event: Self =
            serde_json::from_slice(bytes).map_err(|_| SettingsSyncError::InvalidEvent)?;
        event.validate()
    }

    pub fn to_json(&self) -> Result<Vec<u8>, SettingsSyncError> {
        let event = self.clone().validate()?;
        let bytes = serde_json::to_vec(&event).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        Ok(bytes)
    }

    /// 返回事件的稳定摘要，供下一个事件写入 `parent`。
    pub fn head(&self) -> Result<String, SettingsSyncError> {
        Ok(hex::encode(Sha256::digest(&self.to_json()?)))
    }

    /// 检查事件是否紧接在 `previous` 后面；失败时拒绝回退、分叉或跳号。
    pub fn validate_successor(&self, previous: &Self) -> Result<(), SettingsSyncError> {
        self.clone().validate()?;
        previous.clone().validate()?;
        if previous.sequence.checked_add(1) != Some(self.sequence)
            || self.parent != previous.head()?
        {
            return Err(SettingsSyncError::ChainMismatch);
        }
        Ok(())
    }

    fn validate(self) -> Result<Self, SettingsSyncError> {
        if self.format != EVENT_FORMAT
            || self.version != EVENT_VERSION
            || !is_lower_hex(&self.device_id, 32)
            || (self.sequence == 0 && !self.parent.is_empty())
            || (self.sequence > 0 && !is_lower_hex(&self.parent, 64))
        {
            return Err(SettingsSyncError::InvalidEvent);
        }
        self.document.clone().validate()?;
        let bytes = serde_json::to_vec(&self).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_EVENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        Ok(self)
    }
}
