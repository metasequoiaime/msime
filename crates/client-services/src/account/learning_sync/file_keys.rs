//! 文件账号存储的 learning 密钥槽；与剪贴板、持久进度文件分别保存。

use super::*;

impl FileAccountSessionStorage {
    pub(super) fn read_learning_key(&self) -> Result<Option<String>, AccountError> {
        self.read_private(LEARNING_KEYSTORE_FILE, MAX_KEYSTORE_BYTES)?
            .map(|bytes| {
                String::from_utf8(bytes)
                    .map(|value| value.trim_end_matches(['\n', '\r']).to_owned())
                    .map_err(|_| AccountError::Storage)
            })
            .transpose()
    }

    pub(super) fn write_learning_key(&self, value: &str) -> Result<(), AccountError> {
        if value.is_empty() || value.len() as u64 > MAX_KEYSTORE_BYTES {
            return Err(AccountError::Storage);
        }
        self.create_directory()?;
        save_bytes(self, LEARNING_KEYSTORE_FILE, value.as_bytes())
    }
}

#[cfg(test)]
#[path = "file_keys/tests.rs"]
mod tests;
