//! 显式支持 learning 的宿主密钥项，与剪贴板身份分开，并沿用账号槽的刷新锁。

use super::*;

impl HostSessionStorage {
    fn learning_item(&self) -> Result<SessionStoreItem, AccountError> {
        if self.slot != HostSessionSlot::Account || !self.backend.supports_learning_keystore() {
            return Err(AccountError::Storage);
        }
        Ok(SessionStoreItem::LearningKeystore)
    }

    pub(super) fn read_learning_key(&self) -> Result<Option<String>, AccountError> {
        let item = self.learning_item()?;
        self.backend
            .read(item, item.capacity())?
            .map(|bytes| {
                if bytes.len() > item.capacity() {
                    return Err(AccountError::Storage);
                }
                String::from_utf8(bytes)
                    .map(|value| value.trim_end_matches(['\n', '\r']).to_owned())
                    .map_err(|_| AccountError::Storage)
            })
            .transpose()
    }

    pub(super) fn write_learning_key(&self, value: &str) -> Result<(), AccountError> {
        let item = self.learning_item()?;
        if value.is_empty() || value.len() > item.capacity() {
            return Err(AccountError::Storage);
        }
        match self.backend.write(item, value.as_bytes())? {
            WriteOutcome::Written => Ok(()),
            WriteOutcome::AlreadyExists => Err(AccountError::Storage),
        }
    }

    pub(super) fn clear_learning_key(&self) -> Result<(), AccountError> {
        if self.backend.supports_learning_keystore() {
            self.backend.remove(SessionStoreItem::LearningKeystore)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
