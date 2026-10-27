//! 事件和检查点的加密。词库的密钥 `K` 来自词库自己的密钥串（与云剪贴板同一套设备批准协议，见 `docs/dictionary-webdav-sync.md`）；密钥串只交出 `M = HKDF(K, info="msime-dictionary-v1-key", 32)`（[`DataKey::from_keystore`]），这里再从 `M` 派生两把子密钥，网盘上只有密文。
//!
//! - 事件：`HKDF(M, info="msime-dictionary-v1-event", 32)`
//! - 检查点：`HKDF(M, info="msime-dictionary-v1-checkpoint", 32)`
//!
//! 密文是 `"v1." ‖ b64(nonce ‖ XChaCha20-Poly1305(子密钥, nonce, 明文, aad))`，aad 是文件里明文头部的规范写法，改动头部的任何字段都会让解密失败。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit as _, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fmt;

const PREFIX: &str = "v1.";
/// 从密钥串的当前密钥派生词库密钥用的标签。
pub const KEYSTORE_INFO: &[u8] = b"msime-dictionary-v1-key";
const EVENT_INFO: &[u8] = b"msime-dictionary-v1-event";
const CHECKPOINT_INFO: &[u8] = b"msime-dictionary-v1-checkpoint";
pub const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;

/// 子密钥用在哪里。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Event,
    Checkpoint,
}

/// 一把词库密钥派生出的两把子密钥，连同密钥串给它的编号。本机同步状态里只存子密钥，不存 `K` 本身。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataKey {
    key_id: String,
    #[serde(with = "hex_key")]
    event: [u8; 32],
    #[serde(with = "hex_key")]
    checkpoint: [u8; 32],
}

impl fmt::Debug for DataKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DataKey")
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

fn derive(material: &[u8; 32], info: &[u8]) -> [u8; 32] {
    let mut output = [0u8; 32];
    Hkdf::<Sha256>::new(None, material)
        .expand(info, &mut output)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    output
}

impl DataKey {
    /// `key_id` 是密钥串给这把密钥的编号（32 位小写十六进制），`material` 是密钥本身。
    pub fn new(key_id: &str, material: &[u8; 32]) -> Option<Self> {
        crate::is_lower_hex(key_id, 32).then(|| Self {
            key_id: key_id.to_owned(),
            event: derive(material, EVENT_INFO),
            checkpoint: derive(material, CHECKPOINT_INFO),
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// 词库密钥串现在的密钥；本机还没获准（没有密钥）时为 `None`。
    pub fn from_keystore(keystore: &crate::cloud::clipboard_e2e::Keystore) -> Option<Self> {
        let (key_id, material) = keystore.derived_key(KEYSTORE_INFO)?;
        Self::new(key_id, &material)
    }

    fn cipher(&self, purpose: Purpose) -> XChaCha20Poly1305 {
        let key = match purpose {
            Purpose::Event => &self.event,
            Purpose::Checkpoint => &self.checkpoint,
        };
        XChaCha20Poly1305::new(&Key::from(*key))
    }

    pub fn seal(
        &self,
        purpose: Purpose,
        plaintext: &[u8],
        aad: &[u8],
        nonce: [u8; NONCE_BYTES],
    ) -> String {
        let sealed = self
            .cipher(purpose)
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .expect("XChaCha20-Poly1305 accepts any message far below its length limit");
        let mut envelope = Vec::with_capacity(NONCE_BYTES + sealed.len());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&sealed);
        format!("{PREFIX}{}", B64.encode(envelope))
    }

    /// 解不开（密钥不对、头部或密文被改过、格式不对）时为 `None`。
    pub fn open(&self, purpose: Purpose, boxed: &str, aad: &[u8]) -> Option<Vec<u8>> {
        let envelope = B64.decode(boxed.strip_prefix(PREFIX)?).ok()?;
        if envelope.len() < NONCE_BYTES + TAG_BYTES {
            return None;
        }
        let (nonce, sealed) = envelope.split_at(NONCE_BYTES);
        self.cipher(purpose)
            .decrypt(&XNonce::try_from(nonce).ok()?, Payload { msg: sealed, aad })
            .ok()
    }
}

mod hex_key {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(key: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(key))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes = hex::decode(text).map_err(serde::de::Error::custom)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_ID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn sealed_payloads_open_only_with_the_same_key_purpose_and_header() {
        let key = DataKey::new(KEY_ID, &[7; 32]).unwrap();
        let boxed = key.seal(Purpose::Event, b"payload", b"header", [1; NONCE_BYTES]);
        assert_eq!(
            key.open(Purpose::Event, &boxed, b"header").as_deref(),
            Some(b"payload".as_slice())
        );
        assert!(key.open(Purpose::Event, &boxed, b"header2").is_none());
        assert!(key.open(Purpose::Checkpoint, &boxed, b"header").is_none());
        let other = DataKey::new(KEY_ID, &[8; 32]).unwrap();
        assert!(other.open(Purpose::Event, &boxed, b"header").is_none());
        assert!(key.open(Purpose::Event, "v1.", b"header").is_none());
        assert!(key.open(Purpose::Event, "v2.AAAA", b"header").is_none());
        // 密文里看不出明文。
        assert!(!boxed.contains("payload"));
    }

    #[test]
    fn the_keystore_hands_out_a_derived_key_but_never_its_own_labels() {
        use crate::cloud::clipboard_e2e::Keystore;
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
        let mut key_id = [0u8; 16];
        Hkdf::<Sha256>::new(None, &[5; 32])
            .expand(b"msime-clipboard-v1-key-id", &mut key_id)
            .unwrap();
        let encoded = format!(
            "v1.{}.{}.{}",
            B64.encode([3u8; 32]),
            hex::encode(key_id),
            B64.encode([5u8; 32])
        );
        let keystore = Keystore::parse(&encoded).unwrap();
        let key = DataKey::from_keystore(&keystore).unwrap();
        assert_eq!(key.key_id(), hex::encode(key_id));
        assert_eq!(
            key,
            DataKey::new(&hex::encode(key_id), &derive(&[5; 32], KEYSTORE_INFO)).unwrap()
        );
        for label in [
            b"msime-clipboard-v1-item".as_slice(),
            b"msime-clipboard-v1-dedup",
            b"msime-clipboard-v1-wrap",
            b"msime-clipboard-v1-key-id",
            b"msime-clipboard-v1-code",
        ] {
            assert!(keystore.derived_key(label).is_none());
        }
        let pending = Keystore::parse(&format!("v1.{}", B64.encode([3u8; 32]))).unwrap();
        assert!(DataKey::from_keystore(&pending).is_none());
    }

    #[test]
    fn subkeys_survive_serialisation_and_never_show_in_debug_output() {
        let key = DataKey::new(KEY_ID, &[9; 32]).unwrap();
        let saved = serde_json::to_string(&key).unwrap();
        let restored: DataKey = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored, key);
        let boxed = key.seal(Purpose::Checkpoint, b"x", b"", [0; NONCE_BYTES]);
        assert_eq!(
            restored.open(Purpose::Checkpoint, &boxed, b"").unwrap(),
            b"x"
        );
        let debug = format!("{key:?}");
        assert!(!debug.contains(&hex::encode(key.event)), "{debug}");
        // 存下来的是派生出的子密钥，不是密钥本身。
        assert!(!saved.contains(&hex::encode([9u8; 32])));
        assert!(DataKey::new("not-a-key-id", &[9; 32]).is_none());
    }
}
