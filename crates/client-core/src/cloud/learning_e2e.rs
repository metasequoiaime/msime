//! 输入习惯同步（learning 域）的端到端加密。
//!
//! 协议、服务端接口和错误码见 `docs/learning-sync.md`。它是云剪贴板端到端加密（[`super::clipboard_e2e`]）的又一个实例：设备编号、核对码的显示形式、HTTP 请求与响应的形状、随机数和 HKDF 都与剪贴板共用同一份实现，域分隔标签、服务端接口、本机密钥串都是自己的一套。与剪贴板 v1 不同的两步是带认证的封装（批准者的静态私钥也参与 DH）和反向核对码（新设备解封后，用户核对一致才启用密钥）。
//!
//! 这里分两层：密码学原语（密钥串、封装、反向核对码、MSL1 信封和 Padmé 补齐），以及一个 sans-IO 驱动，只管设备登记、建钥、批准、拒绝、移除、重置、反向核对和关闭。上传和下载密文的那一轮（导出、比较摘要、防回滚、合并）不在这里，由调用方用 [`Keystore::seal_blob`] 和 [`Keystore::open_blob`] 拼出来。驱动和剪贴板的一样不打开任何连接。
//!
//! 驱动不和剪贴板的驱动合并成一个按域参数化的状态机：两个服务端接口的路径、请求体和状态流转都不同（剪贴板 `POST /keys` 建钥和换钥共用一个接口、按条目上传；learning 域分成 `POST /key` 和 `POST /rekey`、批准要带批准者、每台设备一份密文），硬合并只会在已经上线六个平台的剪贴板状态机里加分支。共用的部分是上面那几个与域无关的原语。

use std::io::{Read as _, Write as _};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit as _, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use flate2::{bufread::GzDecoder, Compression, GzBuilder};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use super::clipboard_e2e::{decode_exact, grouped_digits, hkdf32, random, sanitize_device_name};
pub use super::clipboard_e2e::{
    device_id, DriverError, HttpRequest, HttpResponse, Persisted, RandomUnavailable,
};

const VERSION_PREFIX: &str = "v1.";
const KEY_ID_INFO: &[u8] = b"msime-learning-v1-key-id";
const BLOB_INFO: &[u8] = b"msime-learning-v1-blob";
const WRAP_INFO: &[u8] = b"msime-learning-v1-wrap";
const CODE_LABEL: &[u8] = b"msime-learning-v1-code";
const SAS_INFO: &[u8] = b"msime-learning-v1-sas";
const API: &str = "/v1/users/me/learning";

const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const WRAP_BYTES: usize = 32 + NONCE_BYTES + 32 + TAG_BYTES;
const MAGIC: &[u8; 4] = b"MSL1";
/// 信封的明文头：`MSL1`、16 字节 `key_id`、16 字节上传设备的 `device_id`、8 字节代次、24 字节 nonce。整个头是 AEAD 的附加数据。
pub const HEADER_BYTES: usize = 4 + 16 + 16 + 8 + NONCE_BYTES;
/// 一台设备的整个信封（明文头加密文）的上限，与服务端的 `MaxBytesReader` 相同。
pub const MAX_ENVELOPE_BYTES: usize = 8 * 1024 * 1024;
/// 代次（即上传时的 `?revision=N`）的上限 2⁵³−1，JSON 数字和服务端的 `bigint` 都能精确表示。
pub const MAX_GENERATION: u64 = (1 << 53) - 1;
/// 解压后的明文流的上限。8 MiB 的信封按调研时约 1/9 的压缩比能装约 72 MiB 明文，这里留出一倍余量；更大的只可能是压缩炸弹，解密方按解不开处理，上传方不写出这么大的流。
pub const MAX_STREAM_BYTES: usize = 128 * 1024 * 1024;
const MAX_DEVICES: usize = 20;
/// 一次操作里最多取几次服务端状态。并发建钥、换钥或批准时会 409 后重取，正常两到三次就够。
const MAX_STATE_ROUNDS: u8 = 4;

// ---- 密码学原语 ----

#[derive(Clone)]
struct LearningKey {
    id: String,
    raw_id: [u8; 16],
    bytes: [u8; 32],
}

impl LearningKey {
    fn new(bytes: [u8; 32]) -> Self {
        let mut raw_id = [0u8; 16];
        Hkdf::<Sha256>::new(None, &bytes)
            .expand(KEY_ID_INFO, &mut raw_id)
            .expect("16 bytes is a valid HKDF-SHA256 output length");
        Self {
            id: hex::encode(raw_id),
            raw_id,
            bytes,
        }
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new(&Key::from(hkdf32(&self.bytes, None, BLOB_INFO)))
    }
}

/// 设备核对码：`SHA-256("msime-learning-v1-code" ‖ 公钥)` 的前 8 字节，16 位数字，每 4 位一组。
pub fn verification_code(public_key: &[u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CODE_LABEL);
    hasher.update(public_key);
    let digest = hasher.finalize();
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    grouped_digits(prefix)
}

/// 反向核对码：`HKDF(ikm=K_learn, salt=P_A ‖ P_D, info="msime-learning-v1-sas", 8)`。批准者 A 批准后显示，新设备 D 解封后显示，号码相同才说明 D 拿到的就是 A 手里的那把密钥。
fn reverse_code(key: &LearningKey, approver: &[u8; 32], recipient: &[u8; 32]) -> String {
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(approver);
    salt[32..].copy_from_slice(recipient);
    let mut prefix = [0u8; 8];
    Hkdf::<Sha256>::new(Some(&salt), &key.bytes)
        .expand(SAS_INFO, &mut prefix)
        .expect("8 bytes is a valid HKDF-SHA256 output length");
    grouped_digits(prefix)
}

fn wrap_aad(key_id: &str, approver_id: &str, recipient_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(WRAP_INFO.len() + 3 + key_id.len() + 64);
    aad.extend_from_slice(WRAP_INFO);
    aad.push(0);
    aad.extend_from_slice(key_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(approver_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(recipient_id.as_bytes());
    aad
}

/// `HKDF(ikm=dh1 ‖ dh2, salt=E ‖ P_A ‖ P_D, info="msime-learning-v1-wrap", 32)`。
fn wrap_cipher(
    dh1: &[u8; 32],
    dh2: &[u8; 32],
    ephemeral: &[u8; 32],
    approver: &[u8; 32],
    recipient: &[u8; 32],
) -> XChaCha20Poly1305 {
    let mut ikm = [0u8; 64];
    ikm[..32].copy_from_slice(dh1);
    ikm[32..].copy_from_slice(dh2);
    let mut salt = [0u8; 96];
    salt[..32].copy_from_slice(ephemeral);
    salt[32..64].copy_from_slice(approver);
    salt[64..].copy_from_slice(recipient);
    XChaCha20Poly1305::new(&Key::from(hkdf32(&ikm, Some(&salt), WRAP_INFO)))
}

/// 批准者用自己的静态私钥 `approver` 把密钥封装给 `recipient`。两次 DH 任一不是贡献性的（对方给了低阶点）就拒绝。第一台设备封装给自己时 `recipient` 就是自己的公钥。
fn wrap_with(
    key: &LearningKey,
    approver: &StaticSecret,
    recipient: &[u8; 32],
    ephemeral: [u8; 32],
    nonce: [u8; NONCE_BYTES],
) -> Option<String> {
    let approver_public = PublicKey::from(approver).to_bytes();
    let ephemeral = StaticSecret::from(ephemeral);
    let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
    let recipient_key = PublicKey::from(*recipient);
    let dh1 = ephemeral.diffie_hellman(&recipient_key);
    let dh2 = approver.diffie_hellman(&recipient_key);
    if !dh1.was_contributory() || !dh2.was_contributory() {
        return None;
    }
    let cipher = wrap_cipher(
        dh1.as_bytes(),
        dh2.as_bytes(),
        &ephemeral_public,
        &approver_public,
        recipient,
    );
    let aad = wrap_aad(&key.id, &device_id(&approver_public), &device_id(recipient));
    let sealed = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &key.bytes,
                aad: &aad,
            },
        )
        .expect("XChaCha20-Poly1305 accepts a 32-byte message");
    let mut envelope = Vec::with_capacity(WRAP_BYTES);
    envelope.extend_from_slice(&ephemeral_public);
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&sealed);
    Some(format!("{VERSION_PREFIX}{}", B64.encode(envelope)))
}

fn wrap_key(
    key: &LearningKey,
    approver: &StaticSecret,
    recipient: &[u8; 32],
) -> Result<Option<String>, RandomUnavailable> {
    Ok(wrap_with(key, approver, recipient, random()?, random()?))
}

/// 用本机私钥和批准者公钥解开封装。批准者公钥不对、封装不是给本机的、或者解出的密钥与服务端声称的 `key_id` 不符时为 `None`。
fn unwrap_key(
    secret: &StaticSecret,
    key_id: &str,
    wrap: &str,
    approver: &[u8; 32],
) -> Option<LearningKey> {
    let envelope: [u8; WRAP_BYTES] = B64
        .decode(wrap.strip_prefix(VERSION_PREFIX)?)
        .ok()?
        .try_into()
        .ok()?;
    let mut ephemeral_public = [0u8; 32];
    ephemeral_public.copy_from_slice(&envelope[..32]);
    let own_public = PublicKey::from(secret).to_bytes();
    let dh1 = secret.diffie_hellman(&PublicKey::from(ephemeral_public));
    let dh2 = secret.diffie_hellman(&PublicKey::from(*approver));
    if !dh1.was_contributory() || !dh2.was_contributory() {
        return None;
    }
    let cipher = wrap_cipher(
        dh1.as_bytes(),
        dh2.as_bytes(),
        &ephemeral_public,
        approver,
        &own_public,
    );
    let aad = wrap_aad(key_id, &device_id(approver), &device_id(&own_public));
    let plain = cipher
        .decrypt(
            &XNonce::try_from(&envelope[32..32 + NONCE_BYTES]).ok()?,
            Payload {
                msg: &envelope[32 + NONCE_BYTES..],
                aad: &aad,
            },
        )
        .ok()?;
    let key = LearningKey::new(plain.try_into().ok()?);
    (key.id == key_id).then_some(key)
}

/// Padmé 补齐后的长度：`L < 2` 时为 `L`；否则 `E = ⌊log₂ L⌋`，`S = ⌊log₂ E⌋ + 1`，`m = 2^(E−S) − 1`，结果是 `(L + m) & ~m`。服务端用它的不动点检查信封长度。
pub fn padme(length: u64) -> u64 {
    if length < 2 {
        return length;
    }
    let exponent = u64::from(length.ilog2());
    let bits = u64::from(exponent.ilog2()) + 1;
    let mask = (1u64 << (exponent - bits)) - 1;
    (length + mask) & !mask
}

/// 按 RFC 1952 压成单个 gzip 成员，`MTIME` 为 0、不带文件名和注释。
fn gzip(stream: &[u8]) -> Vec<u8> {
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    encoder
        .write_all(stream)
        .expect("writing into a Vec cannot fail");
    encoder.finish().expect("writing into a Vec cannot fail")
}

/// 解开单个 gzip 成员，后面不能再有任何字节；解压结果超过 [`MAX_STREAM_BYTES`] 时为 `None`。
fn gunzip(compressed: &[u8]) -> Option<Vec<u8>> {
    gunzip_at_most(compressed, MAX_STREAM_BYTES)
}

fn gunzip_at_most(compressed: &[u8], limit: usize) -> Option<Vec<u8>> {
    let mut decoder = GzDecoder::new(compressed);
    let mut stream = Vec::new();
    decoder
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut stream)
        .ok()?;
    (stream.len() <= limit && decoder.into_inner().is_empty()).then_some(stream)
}

/// 把补齐好的明文加密成信封：明文头整个作为附加数据。
fn seal_padded_with(
    key: &LearningKey,
    device: [u8; 16],
    generation: u64,
    padded: &[u8],
    nonce: [u8; NONCE_BYTES],
) -> Vec<u8> {
    let mut envelope = Vec::with_capacity(HEADER_BYTES + padded.len() + TAG_BYTES);
    envelope.extend_from_slice(MAGIC);
    envelope.extend_from_slice(&key.raw_id);
    envelope.extend_from_slice(&device);
    envelope.extend_from_slice(&generation.to_be_bytes());
    envelope.extend_from_slice(&nonce);
    let sealed = key
        .cipher()
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: padded,
                aad: &envelope,
            },
        )
        .expect("XChaCha20-Poly1305 accepts any message below the envelope limit");
    envelope.extend_from_slice(&sealed);
    envelope
}

/// `padded = u32 大端(len(gz)) ‖ gz ‖ 零字节`，总长 `padme(4 + len(gz))`；整个信封超过 [`MAX_ENVELOPE_BYTES`] 时为 `None`。
fn seal_compressed_with(
    key: &LearningKey,
    device: [u8; 16],
    generation: u64,
    compressed: &[u8],
    nonce: [u8; NONCE_BYTES],
) -> Option<Vec<u8>> {
    let length = u32::try_from(compressed.len()).ok()?;
    let padded_length = usize::try_from(padme(4 + u64::from(length))).ok()?;
    if HEADER_BYTES + padded_length + TAG_BYTES > MAX_ENVELOPE_BYTES {
        return None;
    }
    let mut padded = Vec::with_capacity(padded_length);
    padded.extend_from_slice(&length.to_be_bytes());
    padded.extend_from_slice(compressed);
    padded.resize(padded_length, 0);
    Some(seal_padded_with(key, device, generation, &padded, nonce))
}

/// 解开一份信封。按协议逐项核对：长度和 Padmé 不动点、`MSL1`、头里的 `key_id` 是本机这把、头里的设备就是请求的 `device_id`、代次在 1 到 2⁵³−1 之间、AEAD 通过、长度字段不越界、补齐全为零、gzip 是单个成员且解压后不超过上限。任何一项不符都是 `None`，调用方按 `learning_undecryptable` 处理。
fn open_with(key: &LearningKey, device: &str, envelope: &[u8]) -> Option<OpenedBlob> {
    if envelope.len() < HEADER_BYTES + TAG_BYTES || envelope.len() > MAX_ENVELOPE_BYTES {
        return None;
    }
    let padded_length = (envelope.len() - HEADER_BYTES - TAG_BYTES) as u64;
    if padme(padded_length) != padded_length {
        return None;
    }
    let (header, sealed) = envelope.split_at(HEADER_BYTES);
    if &header[..4] != MAGIC
        || header[4..20] != key.raw_id
        || hex::encode(&header[20..36]) != device
    {
        return None;
    }
    let generation = u64::from_be_bytes(header[36..44].try_into().ok()?);
    if !(1..=MAX_GENERATION).contains(&generation) {
        return None;
    }
    let padded = key
        .cipher()
        .decrypt(
            &XNonce::try_from(&header[44..HEADER_BYTES]).ok()?,
            Payload {
                msg: sealed,
                aad: header,
            },
        )
        .ok()?;
    let (length, rest) = padded.split_first_chunk::<4>()?;
    let length = usize::try_from(u32::from_be_bytes(*length)).ok()?;
    if length > rest.len() || rest[length..].iter().any(|byte| *byte != 0) {
        return None;
    }
    let stream = gunzip(&rest[..length])?;
    Some(OpenedBlob { generation, stream })
}

/// 解开的一份密文：验证过的代次和明文流（`msime-learning-sync` 的 NDJSON，内容由调用方校验）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedBlob {
    pub generation: u64,
    pub stream: Vec<u8>,
}

/// [`Keystore::seal_blob`] 没有写出信封的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealError {
    /// 本机还没有经过反向核对的密钥。
    NotReady,
    /// 代次不在 1 到 [`MAX_GENERATION`] 之间。
    InvalidGeneration,
    /// 明文流超过 [`MAX_STREAM_BYTES`]，或者压缩补齐后的信封超过 [`MAX_ENVELOPE_BYTES`]。调用方按协议截断明文流后再试。
    TooLarge,
    /// 系统随机数源不可用。
    RandomUnavailable,
}

// ---- 本机的密钥串 ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyState {
    /// 本机生成的，或者用户已经核对过反向核对码。只有这种状态的密钥能加解密。
    Confirmed,
    /// 已经解开封装，等用户核对反向核对码。
    Unconfirmed,
    /// 用户核对时点了「不一致」。密钥本身已丢弃，只记着编号和批准者，同一个批准者给同一把密钥的封装不再解开。
    Rejected,
}

impl KeyState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Unconfirmed => "unconfirmed",
            Self::Rejected => "rejected",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "confirmed" => Some(Self::Confirmed),
            "unconfirmed" => Some(Self::Unconfirmed),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct HeldKey {
    id: String,
    /// `Rejected` 时为 `None`，其余两种状态一定有。
    key: Option<LearningKey>,
    /// 封装这把密钥的批准者的公钥；本机生成的就是本机公钥。
    approver: [u8; 32],
    state: KeyState,
}

impl HeldKey {
    fn own(key: LearningKey, public_key: [u8; 32]) -> Self {
        Self {
            id: key.id.clone(),
            key: Some(key),
            approver: public_key,
            state: KeyState::Confirmed,
        }
    }
}

/// learning 域的本机密钥串：设备私钥，以及当前账号密钥和它的核对状态。宿主把 [`Keystore::encode`] 的结果当作不透明字符串存进 `learning-keystore`，与云剪贴板的密钥串分开存放。
pub struct Keystore {
    secret: StaticSecret,
    key: Option<HeldKey>,
}

impl Keystore {
    /// 生成新的设备私钥，还没有账号密钥。
    pub fn generate() -> Result<Self, RandomUnavailable> {
        Ok(Self {
            secret: StaticSecret::from(random::<32>()?),
            key: None,
        })
    }

    /// 解析 [`Keystore::encode`] 的结果；格式不对时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        let rest = value.strip_prefix(VERSION_PREFIX)?;
        let parts = rest.split('.').collect::<Vec<_>>();
        let secret = StaticSecret::from(decode_exact::<32>(parts.first()?)?);
        let key = match parts[1..] {
            [] => None,
            [id, key, approver, state] => {
                let state = KeyState::parse(state)?;
                if !crate::text::is_lower_hex(id, 32) {
                    return None;
                }
                let key = match (state, key) {
                    (KeyState::Rejected, "-") => None,
                    (KeyState::Rejected, _) | (_, "-") => return None,
                    (_, key) => {
                        let key = LearningKey::new(decode_exact::<32>(key)?);
                        if key.id != id {
                            return None;
                        }
                        Some(key)
                    }
                };
                Some(HeldKey {
                    id: id.to_owned(),
                    key,
                    approver: decode_exact::<32>(approver)?,
                    state,
                })
            }
            _ => return None,
        };
        Some(Self { secret, key })
    }

    /// `v1.<私钥>`；有账号密钥时再接 `.<key_id>.<密钥或 ->.<批准者公钥>.<confirmed|unconfirmed|rejected>`，被拒绝的密钥只留编号，密钥一栏写 `-`。
    pub fn encode(&self) -> String {
        let mut value = format!("{VERSION_PREFIX}{}", B64.encode(self.secret.as_bytes()));
        if let Some(held) = &self.key {
            let key = held
                .key
                .as_ref()
                .map_or_else(|| "-".to_owned(), |key| B64.encode(key.bytes));
            value.push_str(&format!(
                ".{}.{key}.{}.{}",
                held.id,
                B64.encode(held.approver),
                held.state.as_str()
            ));
        }
        value
    }

    pub fn public_key(&self) -> [u8; 32] {
        PublicKey::from(&self.secret).to_bytes()
    }

    pub fn device_id(&self) -> String {
        device_id(&self.public_key())
    }

    pub fn verification_code(&self) -> String {
        verification_code(&self.public_key())
    }

    /// 两份编码过的密钥串是不是同一个设备身份（同一把设备私钥）。任何一份解析不了都答 `false`。
    pub fn same_identity(left: &str, right: &str) -> bool {
        match (Self::parse(left), Self::parse(right)) {
            (Some(left), Some(right)) => left.secret.as_bytes() == right.secret.as_bytes(),
            _ => false,
        }
    }

    /// 宿主保存 `value` 之前的检查，规则与云剪贴板的 [`super::clipboard_e2e::Keystore::other_identity`] 相同。
    pub fn other_identity(stored: Option<String>, value: &str) -> Option<String> {
        stored.filter(|stored| Self::parse(stored).is_some() && !Self::same_identity(stored, value))
    }

    /// 能用来加解密的密钥编号：只有经过反向核对（或本机生成）的密钥才算。
    pub fn ready_key_id(&self) -> Option<&str> {
        self.ready_key().map(|key| key.id.as_str())
    }

    fn ready_key(&self) -> Option<&LearningKey> {
        self.key
            .as_ref()
            .filter(|held| held.state == KeyState::Confirmed)
            .and_then(|held| held.key.as_ref())
    }

    /// 把本机导出的明文流压缩、补齐、加密成这台设备的信封，代次就是这次上传的 `?revision=N`。
    pub fn seal_blob(&self, generation: u64, stream: &[u8]) -> Result<Vec<u8>, SealError> {
        let key = self.ready_key().ok_or(SealError::NotReady)?;
        if !(1..=MAX_GENERATION).contains(&generation) {
            return Err(SealError::InvalidGeneration);
        }
        if stream.len() > MAX_STREAM_BYTES {
            return Err(SealError::TooLarge);
        }
        let nonce = random().map_err(|RandomUnavailable| SealError::RandomUnavailable)?;
        let mut device = [0u8; 16];
        device.copy_from_slice(&Sha256::digest(self.public_key())[..16]);
        seal_compressed_with(key, device, generation, &gzip(stream), nonce)
            .ok_or(SealError::TooLarge)
    }

    /// 解开 `GET /blobs/{device_id}` 下载到的信封。头里的设备必须就是请求的 `device_id`，`key_id` 必须是本机能用的这把；代次与防回滚下限的比较由调用方做。
    pub fn open_blob(&self, device_id: &str, envelope: &[u8]) -> Option<OpenedBlob> {
        open_with(self.ready_key()?, device_id, envelope)
    }
}

// ---- 驱动 ----

/// 宿主可以发起的操作。`confirm` 和 `deny` 是用户核对反向核对码之后的选择，`disable` 是本机关闭输入习惯同步。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    Status,
    Approve { device_id: String },
    Reject { device_id: String },
    Remove { device_id: String },
    Reset,
    Confirm,
    Deny,
    Disable,
}

/// 驱动的一步，含义与云剪贴板驱动的 [`super::clipboard_e2e::Step`] 相同。
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Request {
        request: HttpRequest,
        state: DriverState,
        keystore: String,
    },
    Done {
        result: Value,
        keystore: String,
    },
    Failed {
        error: DriverError,
        keystore: String,
    },
}

/// 驱动在两次请求之间的状态，对宿主不透明。它含有本机私钥，与密钥串同等对待，只在内存里传递。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverState {
    operation: Operation,
    device_name: String,
    keystore: String,
    phase: Phase,
    #[serde(default)]
    state_rounds: u8,
    #[serde(default)]
    registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum Phase {
    /// 等待 `GET /?device_id=`。
    State,
    /// 等待登记本机。
    Register,
    /// 等待第一把密钥写入；成功后本机采用它。`finish` 为真时随后只回报状态，否则继续原来的操作。
    CreateKey { key: String, finish: bool },
    /// 等待换钥；成功后本机采用新密钥并回报状态。
    Rekey { key: String },
    /// 等待批准写入；成功后回报状态和这台设备的反向核对码。
    Approve { device_id: String, code: String },
    /// 等待拒绝或删除一台设备，之后回报状态。
    Changed,
    /// 等待注销本机。
    Disable,
    /// 等待最后一次取状态，用来回报。
    Final { approved: Option<Approved> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Approved {
    device_id: String,
    code: String,
}

#[derive(Debug, Deserialize)]
struct ServerState {
    key_id: Option<String>,
    #[serde(default)]
    wrap: Option<ServerWrap>,
    #[serde(default)]
    devices: Vec<ServerDevice>,
}

#[derive(Debug, Deserialize)]
struct ServerWrap {
    wrap: String,
    approver_device_id: String,
}

#[derive(Debug, Deserialize)]
struct ServerDevice {
    device_id: String,
    public_key: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    last_seen: String,
    #[serde(default)]
    approved: bool,
}

impl ServerDevice {
    /// 公钥与编号一致才算数：核对码和封装都按公钥算，对不上的设备不显示、不封装，也不当作批准者。
    fn verified_public_key(&self) -> Option<[u8; 32]> {
        let public_key = decode_exact::<32>(&self.public_key)?;
        (device_id(&public_key) == self.device_id).then_some(public_key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readiness {
    /// 有经过核对的当前密钥，可以批准设备、上传和下载。
    Ready,
    /// 解开了封装，等用户核对反向核对码。
    Confirm,
    /// 没有可用的密钥，等另一台设备允许；`notice` 是给用户的原因。
    Pending { notice: Option<&'static str> },
    /// 账号还没有密钥。
    NoKey,
}

fn state_path(device_id: &str) -> String {
    format!("{API}?device_id={device_id}")
}

/// 开始一个操作。`keystore` 是宿主保存的 learning 密钥串，第一次使用或解析不了时生成新的设备身份。
pub fn start(
    operation: Operation,
    keystore: Option<&str>,
    device_name: &str,
) -> Result<Step, RandomUnavailable> {
    let mut keystore = match keystore.and_then(Keystore::parse) {
        Some(keystore) => keystore,
        None => Keystore::generate()?,
    };
    let own_id = keystore.device_id();
    let mut state = DriverState {
        operation,
        device_name: sanitize_device_name(device_name),
        keystore: keystore.encode(),
        phase: Phase::State,
        state_rounds: 0,
        registered: false,
    };
    if let Some(code) = invalid_operation(&state.operation, &own_id) {
        return Ok(fail(&state, DriverError::local(code)));
    }
    match state.operation.clone() {
        Operation::Reject { device_id } => {
            state.phase = Phase::Changed;
            Ok(request(
                state,
                "DELETE",
                format!("{API}/devices/{device_id}"),
                None,
            ))
        }
        Operation::Disable => {
            // 先在本机丢掉密钥，注销失败时本机也不会再用它加解密。
            keystore.key = None;
            state.keystore = keystore.encode();
            state.phase = Phase::Disable;
            Ok(request(
                state,
                "DELETE",
                format!("{API}/devices/{own_id}"),
                None,
            ))
        }
        Operation::Confirm | Operation::Deny => {
            let Some(held) = keystore
                .key
                .as_mut()
                .filter(|held| held.state == KeyState::Unconfirmed)
            else {
                return Ok(fail(&state, DriverError::local("invalid_state")));
            };
            if state.operation == Operation::Confirm {
                held.state = KeyState::Confirmed;
            } else {
                held.state = KeyState::Rejected;
                held.key = None;
            }
            state.keystore = keystore.encode();
            state.phase = Phase::Final { approved: None };
            Ok(request(state, "GET", state_path(&own_id), None))
        }
        _ => Ok(fetch_state(state)),
    }
}

fn invalid_operation(operation: &Operation, own_id: &str) -> Option<&'static str> {
    match operation {
        Operation::Approve { device_id }
        | Operation::Reject { device_id }
        | Operation::Remove { device_id }
            if !crate::text::is_lower_hex(device_id, 32) || device_id == own_id =>
        {
            Some("invalid_device_id")
        }
        _ => None,
    }
}

/// 交回上一步请求的响应，得到下一步。
pub fn resume(mut state: DriverState, response: HttpResponse) -> Result<Step, RandomUnavailable> {
    let Some(mut keystore) = Keystore::parse(&state.keystore) else {
        return Ok(fail(&state, DriverError::local("invalid_state")));
    };
    let success = (200..300).contains(&response.status);
    let key_conflict = response.status == 409
        && DriverError::from_response(&response).code == "learning_key_conflict";
    match std::mem::replace(&mut state.phase, Phase::State) {
        Phase::State => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Ok(server) = serde_json::from_str::<ServerState>(&response.body) else {
                return Ok(fail(&state, DriverError::invalid_response()));
            };
            handle_state(state, keystore, server)
        }
        Phase::Register => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            Ok(fetch_state(state))
        }
        Phase::CreateKey { key, finish } => {
            if key_conflict {
                // 另一台设备同时建好了密钥：重新取状态，本机会变成待批准。
                return Ok(fetch_state(state));
            }
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Some(bytes) = decode_exact::<32>(&key) else {
                return Ok(fail(&state, DriverError::local("invalid_state")));
            };
            keystore.key = Some(HeldKey::own(LearningKey::new(bytes), keystore.public_key()));
            state.keystore = keystore.encode();
            Ok(if finish {
                final_state(state, &keystore, None)
            } else {
                fetch_state(state)
            })
        }
        Phase::Rekey { key } => {
            if key_conflict {
                // 换钥期间密钥又换了：重新取当前编号再换一次。
                return Ok(fetch_state(state));
            }
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Some(bytes) = decode_exact::<32>(&key) else {
                return Ok(fail(&state, DriverError::local("invalid_state")));
            };
            keystore.key = Some(HeldKey::own(LearningKey::new(bytes), keystore.public_key()));
            state.keystore = keystore.encode();
            Ok(final_state(state, &keystore, None))
        }
        Phase::Approve { device_id, code } => {
            if key_conflict {
                // 批准期间密钥换了：重新取状态，按新密钥再批准一次（本机没有新密钥时会报等待批准）。
                return Ok(fetch_state(state));
            }
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            Ok(final_state(
                state,
                &keystore,
                Some(Approved { device_id, code }),
            ))
        }
        Phase::Changed => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            Ok(final_state(state, &keystore, None))
        }
        Phase::Disable => {
            // 本机本来就没登记（或者已经被删掉）时也算关好了。
            if !success && response.status != 404 {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            Ok(done(
                &state,
                json!({
                    "status": "disabled",
                    "device_id": keystore.device_id(),
                    "code": keystore.verification_code(),
                }),
            ))
        }
        Phase::Final { approved } => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Ok(server) = serde_json::from_str::<ServerState>(&response.body) else {
                return Ok(fail(&state, DriverError::invalid_response()));
            };
            let readiness = evaluate(&mut keystore, &server);
            state.keystore = keystore.encode();
            let view = status_view(&keystore, &server, readiness, approved.as_ref());
            Ok(done(&state, view))
        }
    }
}

fn handle_state(
    mut state: DriverState,
    mut keystore: Keystore,
    server: ServerState,
) -> Result<Step, RandomUnavailable> {
    let own_id = keystore.device_id();
    if !server
        .devices
        .iter()
        .any(|device| device.device_id == own_id)
    {
        if state.registered {
            return Ok(fail(&state, DriverError::invalid_response()));
        }
        state.registered = true;
        state.phase = Phase::Register;
        let body = json!({
            "public_key": B64.encode(keystore.public_key()),
            "name": state.device_name,
        });
        let path = format!("{API}/devices/{own_id}");
        return Ok(request(state, "PUT", path, Some(body)));
    }
    let server_key = server
        .key_id
        .clone()
        .filter(|id| crate::text::is_lower_hex(id, 32));
    match (state.operation.clone(), server_key) {
        (Operation::Reset, Some(previous)) => return rekey(state, &keystore, previous, Vec::new()),
        (Operation::Remove { device_id }, Some(previous)) => {
            return rekey(state, &keystore, previous, vec![device_id])
        }
        (Operation::Reset, None) => return create_key(state, &keystore, true),
        (Operation::Remove { device_id }, None) => {
            // 账号还没有密钥，移除只是删掉登记。
            state.phase = Phase::Changed;
            let path = format!("{API}/devices/{device_id}");
            return Ok(request(state, "DELETE", path, None));
        }
        _ => {}
    }
    let readiness = evaluate(&mut keystore, &server);
    state.keystore = keystore.encode();
    match (state.operation.clone(), readiness) {
        (_, Readiness::NoKey) => create_key(state, &keystore, false),
        (Operation::Status, readiness) => {
            let view = status_view(&keystore, &server, readiness, None);
            Ok(done(&state, view))
        }
        (Operation::Approve { device_id }, Readiness::Ready) => {
            approve(state, &keystore, &server, device_id)
        }
        (Operation::Approve { .. }, Readiness::Pending { notice }) => Ok(fail(
            &state,
            DriverError::local(notice.unwrap_or("learning_waiting_approval")),
        )),
        (Operation::Approve { .. }, Readiness::Confirm) => Ok(fail(
            &state,
            DriverError::local("learning_waiting_approval"),
        )),
        _ => Ok(fail(&state, DriverError::local("invalid_state"))),
    }
}

fn approve(
    mut state: DriverState,
    keystore: &Keystore,
    server: &ServerState,
    device_id: String,
) -> Result<Step, RandomUnavailable> {
    let key = keystore.ready_key().expect("ready implies a confirmed key");
    let Some(device) = server
        .devices
        .iter()
        .find(|device| device.device_id == device_id)
    else {
        return Ok(fail(
            &state,
            DriverError {
                status: 404,
                code: "not_found".to_owned(),
            },
        ));
    };
    let Some(public_key) = device.verified_public_key() else {
        return Ok(fail(&state, DriverError::invalid_response()));
    };
    let Some(wrap) = wrap_key(key, &keystore.secret, &public_key)? else {
        return Ok(fail(&state, DriverError::invalid_response()));
    };
    let body = json!({
        "key_id": key.id,
        "wrap": wrap,
        "approver_device_id": keystore.device_id(),
    });
    state.phase = Phase::Approve {
        code: reverse_code(key, &keystore.public_key(), &public_key),
        device_id: device_id.clone(),
    };
    Ok(request(
        state,
        "PUT",
        format!("{API}/wraps/{device_id}"),
        Some(body),
    ))
}

/// 账号还没有密钥：生成第一把，只封装给本机。
fn create_key(
    mut state: DriverState,
    keystore: &Keystore,
    finish: bool,
) -> Result<Step, RandomUnavailable> {
    let key = LearningKey::new(random()?);
    let Some(wrap) = wrap_key(&key, &keystore.secret, &keystore.public_key())? else {
        return Ok(fail(&state, DriverError::local("invalid_state")));
    };
    let body = json!({ "key_id": key.id, "wrap": wrap });
    state.phase = Phase::CreateKey {
        key: B64.encode(key.bytes),
        finish,
    };
    Ok(request(state, "POST", format!("{API}/key"), Some(body)))
}

/// 换钥：生成新密钥，只封装给本机。服务端在同一事务里删掉 `revoke` 里的设备、全部密文和全部旧封装；其他设备都要重新批准。
fn rekey(
    mut state: DriverState,
    keystore: &Keystore,
    previous: String,
    revoke: Vec<String>,
) -> Result<Step, RandomUnavailable> {
    let key = LearningKey::new(random()?);
    let Some(wrap) = wrap_key(&key, &keystore.secret, &keystore.public_key())? else {
        return Ok(fail(&state, DriverError::local("invalid_state")));
    };
    let body = json!({
        "previous_key_id": previous,
        "key_id": key.id,
        "wrap": wrap,
        "revoke": revoke,
    });
    state.phase = Phase::Rekey {
        key: B64.encode(key.bytes),
    };
    Ok(request(state, "POST", format!("{API}/rekey"), Some(body)))
}

/// 根据服务端状态判断本机能不能用，必要时解开封装并更新本机密钥。
///
/// 封装只在找得到批准者公钥时才解：批准者是本机（第一台设备、本机换的钥）时直接采用，是别的设备时要等用户核对反向核对码。用户拒绝过的「同一把密钥、同一个批准者」不再解开；换了批准者或换了密钥就重新解、重新核对。
fn evaluate(keystore: &mut Keystore, server: &ServerState) -> Readiness {
    let Some(server_key) = server
        .key_id
        .as_deref()
        .filter(|id| crate::text::is_lower_hex(id, 32))
    else {
        keystore.key = None;
        return Readiness::NoKey;
    };
    let own_public = keystore.public_key();
    let own_id = device_id(&own_public);
    let mut rotated = false;
    if let Some(held) = &keystore.key {
        if held.id == server_key {
            match held.state {
                KeyState::Confirmed => return Readiness::Ready,
                KeyState::Unconfirmed => return Readiness::Confirm,
                KeyState::Rejected => {
                    let same_approver = server
                        .wrap
                        .as_ref()
                        .is_none_or(|wrap| wrap.approver_device_id == device_id(&held.approver));
                    if same_approver {
                        return Readiness::Pending {
                            notice: Some("learning_sas_mismatch"),
                        };
                    }
                }
            }
        } else {
            rotated = held.state == KeyState::Confirmed;
        }
    }
    let unwrapped = server.wrap.as_ref().and_then(|wrap| {
        let approver = if wrap.approver_device_id == own_id {
            own_public
        } else {
            server
                .devices
                .iter()
                .find(|device| device.device_id == wrap.approver_device_id)?
                .verified_public_key()?
        };
        let key = unwrap_key(&keystore.secret, server_key, &wrap.wrap, &approver)?;
        Some((key, approver))
    });
    match unwrapped {
        Some((key, approver)) => {
            let state = if approver == own_public {
                KeyState::Confirmed
            } else {
                KeyState::Unconfirmed
            };
            keystore.key = Some(HeldKey {
                id: key.id.clone(),
                key: Some(key),
                approver,
                state,
            });
            if state == KeyState::Confirmed {
                Readiness::Ready
            } else {
                Readiness::Confirm
            }
        }
        None => {
            keystore.key = None;
            Readiness::Pending {
                notice: rotated.then_some("learning_key_rotated"),
            }
        }
    }
}

fn status_view(
    keystore: &Keystore,
    server: &ServerState,
    readiness: Readiness,
    approved: Option<&Approved>,
) -> Value {
    let own_public = keystore.public_key();
    let own_id = device_id(&own_public);
    let (status, notice) = match readiness {
        Readiness::Ready => ("ready", None),
        Readiness::Confirm => ("confirm", None),
        Readiness::Pending { notice } => ("pending", notice),
        Readiness::NoKey => ("pending", None),
    };
    let name_of = |id: &str| {
        server
            .devices
            .iter()
            .find(|device| device.device_id == id)
            .map(|device| sanitize_device_name(&device.name))
            .unwrap_or_default()
    };
    let confirm = keystore
        .key
        .as_ref()
        .filter(|held| readiness == Readiness::Confirm && held.state == KeyState::Unconfirmed)
        .and_then(|held| {
            let key = held.key.as_ref()?;
            let approver_id = device_id(&held.approver);
            Some(json!({
                "code": reverse_code(key, &held.approver, &own_public),
                "approver_device_id": approver_id,
                "approver_name": name_of(&approver_id),
            }))
        });
    let bounded = |value: &str| {
        if crate::is_bounded_text(value, 128) {
            value.to_owned()
        } else {
            String::new()
        }
    };
    let devices = server
        .devices
        .iter()
        .take(MAX_DEVICES)
        .filter_map(|device| {
            let public_key = device.verified_public_key()?;
            Some(json!({
                "device_id": device.device_id,
                "name": sanitize_device_name(&device.name),
                "created_at": bounded(&device.created_at),
                "last_seen": bounded(&device.last_seen),
                "approved": device.approved,
                "current": device.device_id == own_id,
                "code": verification_code(&public_key),
            }))
        })
        .collect::<Vec<_>>();
    json!({
        "status": status,
        "notice": notice,
        "device_id": own_id,
        "code": keystore.verification_code(),
        "confirm": confirm,
        "approved": approved.map(|approved| json!({
            "device_id": approved.device_id,
            "code": approved.code,
            "name": name_of(&approved.device_id),
        })),
        "devices": devices,
    })
}

fn fetch_state(mut state: DriverState) -> Step {
    state.state_rounds += 1;
    if state.state_rounds > MAX_STATE_ROUNDS {
        return fail(&state, DriverError::local("learning_key_conflict"));
    }
    let path = match Keystore::parse(&state.keystore) {
        Some(keystore) => state_path(&keystore.device_id()),
        None => return fail(&state, DriverError::local("invalid_state")),
    };
    state.phase = Phase::State;
    request(state, "GET", path, None)
}

fn final_state(mut state: DriverState, keystore: &Keystore, approved: Option<Approved>) -> Step {
    state.phase = Phase::Final { approved };
    request(state, "GET", state_path(&keystore.device_id()), None)
}

fn request(state: DriverState, method: &str, path: String, body: Option<Value>) -> Step {
    let keystore = state.keystore.clone();
    Step::Request {
        request: HttpRequest {
            method: method.to_owned(),
            path,
            body: body.map(|body| body.to_string()),
        },
        state,
        keystore,
    }
}

fn done(state: &DriverState, result: Value) -> Step {
    Step::Done {
        result,
        keystore: state.keystore.clone(),
    }
}

fn fail(state: &DriverState, error: DriverError) -> Step {
    Step::Failed {
        error,
        keystore: state.keystore.clone(),
    }
}

// ---- 宿主接口 ----

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartInput {
    start: Operation,
    #[serde(default)]
    keystore: Option<String>,
    #[serde(default)]
    device_name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResumeInput {
    resume: DriverState,
    response: HttpResponse,
}

/// C ABI、JNI 和 NAPI 共用的 JSON 入口（经 [`crate::cloud::e2e_drive_json`] 按 `"domain": "learning"` 分派到这里），格式与云剪贴板驱动相同：`{start, keystore, device_name}` 或 `{resume, response}`。
pub(crate) fn drive_value(value: Value) -> Result<Value, String> {
    let step = if value.get("start").is_some() {
        let input: StartInput =
            serde_json::from_value(value).map_err(|_| "invalid learning e2e request")?;
        start(input.start, input.keystore.as_deref(), &input.device_name)
    } else {
        let input: ResumeInput =
            serde_json::from_value(value).map_err(|_| "invalid learning e2e request")?;
        resume(input.resume, input.response)
    }
    .map_err(|_| "secure random source unavailable")?;
    Ok(step_json(step))
}

/// 把一步转成宿主看到的 JSON。
pub fn step_json(step: Step) -> Value {
    match step {
        Step::Request {
            request,
            state,
            keystore,
        } => json!({ "request": request, "state": state, "keystore": keystore }),
        Step::Done { result, keystore } => json!({ "done": result, "keystore": keystore }),
        Step::Failed { error, keystore } => json!({ "error": error, "keystore": keystore }),
    }
}

/// 在 Rust 里直接驱动一个操作（桌面端和 client-services 的同步轮次）：`send` 发请求，`persist` 保存每一步交回的密钥串。身份冲突的处理与云剪贴板的 [`super::clipboard_e2e::run`] 相同，已经发过请求之后再冲突时答 `learning_keystore_conflict`。
pub fn run<E>(
    operation: Operation,
    keystore: Option<&str>,
    device_name: &str,
    mut send: impl FnMut(&HttpRequest) -> Result<HttpResponse, E>,
    mut persist: impl FnMut(&str) -> Result<Persisted, E>,
    random_unavailable: impl FnOnce() -> E,
) -> Result<Result<Value, DriverError>, E> {
    let mut saved = keystore.map(str::to_owned);
    let mut step = match start(operation.clone(), keystore, device_name) {
        Ok(step) => step,
        Err(RandomUnavailable) => return Err(random_unavailable()),
    };
    let mut sent = false;
    let mut restarted = false;
    loop {
        let current = match &step {
            Step::Request { keystore, .. }
            | Step::Done { keystore, .. }
            | Step::Failed { keystore, .. } => keystore,
        };
        if saved.as_deref() != Some(current.as_str()) {
            match persist(current)? {
                Persisted::Saved => saved = Some(current.clone()),
                Persisted::Conflict(stored) => {
                    if sent || restarted || Keystore::parse(&stored).is_none() {
                        return Ok(Err(DriverError::local("learning_keystore_conflict")));
                    }
                    restarted = true;
                    step = match start(operation.clone(), Some(&stored), device_name) {
                        Ok(step) => step,
                        Err(RandomUnavailable) => return Err(random_unavailable()),
                    };
                    saved = Some(stored);
                    continue;
                }
            }
        }
        match step {
            Step::Request { request, state, .. } => {
                sent = true;
                let response = send(&request)?;
                step = match resume(state, response) {
                    Ok(step) => step,
                    Err(RandomUnavailable) => return Err(random_unavailable()),
                };
            }
            Step::Done { result, .. } => return Ok(Ok(result)),
            Step::Failed { error, .. } => return Ok(Err(error)),
        }
    }
}

#[cfg(test)]
#[path = "learning_e2e_tests.rs"]
mod tests;
