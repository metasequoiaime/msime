//! 设置同步专用的密钥派生和有界事件密文。

use super::event::SettingsSyncEvent;
use super::*;

const CIPHERTEXT_PREFIX: &str = "v1.";
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
pub const MAX_SEALED_EVENT_BYTES: usize =
    (MAX_EVENT_BYTES + NONCE_BYTES + TAG_BYTES).div_ceil(3) * 4 + 4096;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    Key, XChaCha20Poly1305, XNonce,
};

/// 设置同步的独立加密域。密钥材料来自现有设备批准密钥，但派生标签、密文前缀和
/// AAD 都与云剪贴板、词库同步隔离；网盘传输层只能看到 `seal` 的结果。
#[derive(Clone)]
pub struct SettingsSyncCipher {
    key_id: String,
    key: [u8; 32],
}

impl std::fmt::Debug for SettingsSyncCipher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSyncCipher")
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

impl SettingsSyncCipher {
    /// 从设备批准协议的当前密钥派生设置同步密钥；设备尚未获准时返回 `None`。
    pub fn from_keystore(keystore: &crate::cloud::clipboard_e2e::Keystore) -> Option<Self> {
        let (key_id, key) = keystore.derived_key(KEY_INFO)?;
        Some(Self {
            key_id: key_id.to_owned(),
            key,
        })
    }

    #[cfg(test)]
    pub(super) fn new(key_id: impl Into<String>, key: [u8; 32]) -> Option<Self> {
        let key_id = key_id.into();
        is_lower_hex(&key_id, 32).then_some(Self { key_id, key })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// 用显式 nonce 加密事件正文。生产调用方应使用系统随机的 24 字节 nonce；显式
    /// 参数让协议测试可以复现密文并检查 AAD 绑定。
    fn seal(&self, plaintext: &[u8], aad: &[u8], nonce: [u8; NONCE_BYTES]) -> String {
        let cipher = XChaCha20Poly1305::new(&Key::from(self.key));
        let encrypted = cipher
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .expect("XChaCha20-Poly1305 accepts settings documents");
        let mut envelope = Vec::with_capacity(NONCE_BYTES + encrypted.len());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&encrypted);
        format!("{CIPHERTEXT_PREFIX}{}", B64.encode(envelope))
    }

    /// AAD、密钥或密文格式不匹配时返回 `None`，调用方必须丢弃该事件。
    fn open(&self, ciphertext: &str, aad: &[u8]) -> Option<Vec<u8>> {
        if ciphertext.len() > MAX_SEALED_EVENT_BYTES {
            return None;
        }
        let encoded = ciphertext.strip_prefix(CIPHERTEXT_PREFIX)?;
        let envelope = B64.decode(encoded).ok()?;
        if envelope.len() < NONCE_BYTES + TAG_BYTES {
            return None;
        }
        let (nonce, encrypted) = envelope.split_at(NONCE_BYTES);
        XChaCha20Poly1305::new(&Key::from(self.key))
            .decrypt(
                &XNonce::try_from(nonce).ok()?,
                Payload {
                    msg: encrypted,
                    aad,
                },
            )
            .ok()
    }
}

/// 网盘只存储此信封。所有可见头字段均纳入 AAD，正文中的同名字段在解密后再核对。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealedSettingsEvent {
    format: String,
    version: u64,
    key_id: String,
    sequence: u64,
    device_id: String,
    parent: String,
    ciphertext: String,
}

impl SealedSettingsEvent {
    fn aad(&self) -> Result<Vec<u8>, SettingsSyncError> {
        serde_json::to_vec(&(
            &self.format,
            self.version,
            &self.key_id,
            self.sequence,
            &self.device_id,
            &self.parent,
        ))
        .map_err(|_| SettingsSyncError::Serialization)
    }

    pub fn to_json(&self) -> Result<Vec<u8>, SettingsSyncError> {
        let bytes = serde_json::to_vec(self).map_err(|_| SettingsSyncError::Serialization)?;
        if bytes.len() > MAX_SEALED_EVENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        Ok(bytes)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, SettingsSyncError> {
        if bytes.len() > MAX_SEALED_EVENT_BYTES {
            return Err(SettingsSyncError::TooLarge);
        }
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| SettingsSyncError::InvalidEvent)?;
        if value.format != "app.msime.settings-sync-sealed"
            || value.version != 1
            || !is_lower_hex(&value.key_id, 32)
            || !is_lower_hex(&value.device_id, 32)
            || (value.sequence == 0 && !value.parent.is_empty())
            || (value.sequence > 0 && !is_lower_hex(&value.parent, 64))
        {
            return Err(SettingsSyncError::InvalidEvent);
        }
        Ok(value)
    }
}

impl SettingsSyncCipher {
    /// 生产入口由系统生成 nonce，调用方无法意外复用固定 nonce。
    pub fn seal_event(
        &self,
        event: &SettingsSyncEvent,
    ) -> Result<SealedSettingsEvent, SettingsSyncError> {
        let plaintext = event.to_json()?;
        let mut nonce = [0; NONCE_BYTES];
        getrandom::fill(&mut nonce).map_err(|_| SettingsSyncError::RandomUnavailable)?;
        let mut sealed = SealedSettingsEvent {
            format: "app.msime.settings-sync-sealed".into(),
            version: 1,
            key_id: self.key_id.clone(),
            sequence: event.sequence,
            device_id: event.device_id.clone(),
            parent: event.parent.clone(),
            ciphertext: String::new(),
        };
        sealed.ciphertext = self.seal(&plaintext, &sealed.aad()?, nonce);
        Ok(sealed)
    }

    pub fn open_event(
        &self,
        sealed: &SealedSettingsEvent,
    ) -> Result<SettingsSyncEvent, SettingsSyncError> {
        if sealed.key_id != self.key_id {
            return Err(SettingsSyncError::Crypto);
        }
        let plaintext = self
            .open(&sealed.ciphertext, &sealed.aad()?)
            .ok_or(SettingsSyncError::Crypto)?;
        let event = SettingsSyncEvent::from_json(&plaintext)?;
        if event.sequence != sealed.sequence
            || event.device_id != sealed.device_id
            || event.parent != sealed.parent
        {
            return Err(SettingsSyncError::InvalidEvent);
        }
        Ok(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encrypted_events_bind_header_and_use_fresh_nonces() {
        let event = SettingsSyncEvent::new(
            0,
            "a".repeat(32),
            "",
            SettingsSyncDocument::from_preferences(&Preferences::default()).unwrap(),
        )
        .unwrap();
        let key = SettingsSyncCipher::new("b".repeat(32), [7; 32]).unwrap();
        let sealed = key.seal_event(&event).unwrap();
        let bytes = sealed.to_json().unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("candidate_page_size"));
        assert_eq!(
            key.open_event(&SealedSettingsEvent::from_json(&bytes).unwrap())
                .unwrap(),
            event
        );
        assert_ne!(sealed, key.seal_event(&event).unwrap());
        let mut tampered = sealed.clone();
        tampered.device_id = "c".repeat(32);
        assert_eq!(key.open_event(&tampered), Err(SettingsSyncError::Crypto));
        let other = SettingsSyncCipher::new("b".repeat(32), [8; 32]).unwrap();
        assert_eq!(other.open_event(&sealed), Err(SettingsSyncError::Crypto));
    }

    #[test]
    fn key_domain_is_independent_and_pending_devices_have_no_key() {
        use crate::cloud::clipboard_e2e::Keystore;
        use hkdf::Hkdf;
        let material = [5; 32];
        let mut id = [0; 16];
        Hkdf::<sha2::Sha256>::new(None, &material)
            .expand(b"msime-clipboard-v1-key-id", &mut id)
            .unwrap();
        let saved = format!(
            "v1.{}.{}.{}",
            B64.encode([3; 32]),
            hex::encode(id),
            B64.encode(material)
        );
        let store = Keystore::parse(&saved).unwrap();
        let key = SettingsSyncCipher::from_keystore(&store).unwrap();
        let (_, dictionary) = store.derived_key(b"msime-dictionary-v1-key").unwrap();
        assert_ne!(key.key, dictionary);
        let pending = Keystore::parse(&format!("v1.{}", B64.encode([3; 32]))).unwrap();
        assert!(SettingsSyncCipher::from_keystore(&pending).is_none());
        assert!(!format!("{key:?}").contains(&hex::encode(key.key)));
    }
}
