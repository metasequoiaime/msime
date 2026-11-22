//! 学习密钥的能力边界与存储转调；匿名和旧宿主均不回退到剪贴板。

use super::*;

impl CallbackBackend {
    pub(super) fn check_learning_item(&self, item: SessionStoreItem) -> Result<(), AccountError> {
        if item == SessionStoreItem::LearningKeystore && !self.supports_learning_keystore() {
            return Err(AccountError::Storage);
        }
        Ok(())
    }
}

impl SessionStorage {
    pub(super) fn read_learning_key(&self) -> Result<Option<String>, AccountError> {
        match self {
            Self::File(storage) => storage.load_learning_keystore(),
            Self::Host(storage) => storage.load_learning_keystore(),
            Self::AnonymousFile(_) => Err(AccountError::Storage),
        }
    }

    pub(super) fn write_learning_key(&self, value: &str) -> Result<(), AccountError> {
        match self {
            Self::File(storage) => storage.save_learning_keystore(value),
            Self::Host(storage) => storage.save_learning_keystore(value),
            Self::AnonymousFile(_) => Err(AccountError::Storage),
        }
    }
}

#[cfg(test)]
mod tests;
