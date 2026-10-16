//! 云剪贴板的端到端加密。
//!
//! 协议、服务端接口和宿主的职责见 `docs/cloud-clipboard-e2e.md`。这里分两层：密码学原语（设备密钥对、剪贴板密钥、条目加密、密钥封装），以及一个 sans-IO 驱动——它决定下一步发哪个请求，宿主发出去后把状态码和正文交回来。宿主各自持有账号凭据和网络栈，所以这个模块和 `cloud` 下的其他模块一样不打开任何连接。
//!
//! 换钥（移除设备、重置）时新密钥只封装给本机，不封装给设备列表里标着「已批准」的其他设备：那个标志来自服务端，服务端可以把它自己登记的设备也标成已批准。其他设备因此要在本机上重新点一次「允许」，每一次都显示核对码。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit as _, Payload},
    Key, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

const VERSION_PREFIX: &str = "v1.";
const KEY_ID_INFO: &[u8] = b"msime-clipboard-v1-key-id";
const ITEM_INFO: &[u8] = b"msime-clipboard-v1-item";
const DEDUP_INFO: &[u8] = b"msime-clipboard-v1-dedup";
const WRAP_INFO: &[u8] = b"msime-clipboard-v1-wrap";
const CODE_LABEL: &[u8] = b"msime-clipboard-v1-code";

/// 明文上限，与改用密文之前相同。
pub const MAX_TEXT_UTF16_UNITS: usize = 4000;
/// 密文字段上限：4000 个 UTF-16 单元的明文最多 12000 字节，编码后约 16057 个字符。
pub const MAX_CIPHERTEXT_CHARS: usize = 16384;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const WRAP_BYTES: usize = 32 + NONCE_BYTES + 32 + TAG_BYTES;
const MAX_DEVICE_NAME_CHARS: usize = 64;
const MAX_ITEMS: usize = 50;
const MAX_DEVICES: usize = 20;
/// 一次操作里最多取几次密钥状态。并发建钥或换钥时会 409 后重取，正常情况下两到三次就够；超过说明另一台设备在反复换钥，交给用户稍后再试。
const MAX_KEY_ROUNDS: u8 = 4;
const API: &str = "/v1/users/me/clipboard";

/// 本机的设备私钥和当前剪贴板密钥。宿主把 [`Keystore::encode`] 的结果当作不透明字符串存进与账号会话相同的安全存储。
pub struct Keystore {
    secret: StaticSecret,
    key: Option<ClipboardKey>,
}

#[derive(Clone)]
struct ClipboardKey {
    id: String,
    bytes: [u8; 32],
}

impl ClipboardKey {
    fn new(bytes: [u8; 32]) -> Self {
        let mut id = [0u8; 16];
        Hkdf::<Sha256>::new(None, &bytes)
            .expand(KEY_ID_INFO, &mut id)
            .expect("16 bytes is a valid HKDF-SHA256 output length");
        Self {
            id: hex::encode(id),
            bytes,
        }
    }

    fn subkey(&self, info: &[u8]) -> [u8; 32] {
        hkdf32(&self.bytes, None, info)
    }
}

/// 加密失败只有一种原因：系统随机数源不可用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomUnavailable;

fn random<const N: usize>() -> Result<[u8; N], RandomUnavailable> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|_| RandomUnavailable)?;
    Ok(bytes)
}

fn hkdf32(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> [u8; 32] {
    let mut output = [0u8; 32];
    Hkdf::<Sha256>::new(salt, ikm)
        .expand(info, &mut output)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    output
}

fn decode_exact<const N: usize>(value: &str) -> Option<[u8; N]> {
    B64.decode(value).ok()?.try_into().ok()
}

/// 由公钥得出设备编号：SHA-256 的前 16 字节，小写十六进制。
pub fn device_id(public_key: &[u8; 32]) -> String {
    hex::encode(&Sha256::digest(public_key)[..16])
}

/// 批准设备时两边显示的核对码，16 位数字，每 4 位一组。
pub fn verification_code(public_key: &[u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CODE_LABEL);
    hasher.update(public_key);
    let digest = hasher.finalize();
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&digest[..8]);
    let digits = format!(
        "{:016}",
        u64::from_be_bytes(prefix) % 10_000_000_000_000_000
    );
    [
        &digits[0..4],
        &digits[4..8],
        &digits[8..12],
        &digits[12..16],
    ]
    .join(" ")
}

impl Keystore {
    /// 生成新的设备私钥，还没有剪贴板密钥。
    pub fn generate() -> Result<Self, RandomUnavailable> {
        Ok(Self {
            secret: StaticSecret::from(random::<32>()?),
            key: None,
        })
    }

    /// 解析 [`Keystore::encode`] 的结果；格式不对时返回 `None`。
    pub fn parse(value: &str) -> Option<Self> {
        let rest = value.strip_prefix(VERSION_PREFIX)?;
        let mut parts = rest.split('.');
        let secret = StaticSecret::from(decode_exact::<32>(parts.next()?)?);
        let key = match (parts.next(), parts.next()) {
            (None, None) => None,
            (Some(id), Some(bytes)) => {
                let key = ClipboardKey::new(decode_exact::<32>(bytes)?);
                if key.id != id {
                    return None;
                }
                Some(key)
            }
            _ => return None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self { secret, key })
    }

    /// `v1.<私钥>`，有剪贴板密钥时再接 `.<key_id>.<密钥>`。
    pub fn encode(&self) -> String {
        let mut value = format!("{VERSION_PREFIX}{}", B64.encode(self.secret.as_bytes()));
        if let Some(key) = &self.key {
            value.push('.');
            value.push_str(&key.id);
            value.push('.');
            value.push_str(&B64.encode(key.bytes));
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

    /// 当前持有的剪贴板密钥编号。
    pub fn key_id(&self) -> Option<&str> {
        self.key.as_ref().map(|key| key.id.as_str())
    }
}

/// 上传用的条目请求体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedItem {
    pub ciphertext: String,
    pub key_id: String,
    pub dedup: String,
}

/// 明文是否满足上传要求：非空白、不超过 4000 个 UTF-16 单元、除制表符和换行外不含控制字符。
pub fn is_valid_text(text: &str) -> bool {
    !text.trim().is_empty()
        && crate::is_bounded_utf16(text, MAX_TEXT_UTF16_UNITS)
        && !text.contains('\0')
        && !crate::text::has_disallowed_control(text)
}

fn item_aad(key_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(ITEM_INFO.len() + 1 + key_id.len());
    aad.extend_from_slice(ITEM_INFO);
    aad.push(0);
    aad.extend_from_slice(key_id.as_bytes());
    aad
}

fn seal_with_nonce(key: &ClipboardKey, text: &str, nonce: [u8; NONCE_BYTES]) -> SealedItem {
    let cipher = XChaCha20Poly1305::new(&Key::from(key.subkey(ITEM_INFO)));
    let aad = item_aad(&key.id);
    let sealed = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: text.as_bytes(),
                aad: &aad,
            },
        )
        .expect("XChaCha20-Poly1305 accepts any message far below its length limit");
    let mut envelope = Vec::with_capacity(NONCE_BYTES + sealed.len());
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&sealed);
    let mut mac = <Hmac<Sha256>>::new_from_slice(&key.subkey(DEDUP_INFO))
        .expect("HMAC accepts a key of any length");
    mac.update(text.as_bytes());
    SealedItem {
        ciphertext: format!("{VERSION_PREFIX}{}", B64.encode(envelope)),
        key_id: key.id.clone(),
        dedup: hex::encode(mac.finalize().into_bytes()),
    }
}

fn open_item(key: &ClipboardKey, ciphertext: &str) -> Option<String> {
    if ciphertext.len() > MAX_CIPHERTEXT_CHARS {
        return None;
    }
    let envelope = B64.decode(ciphertext.strip_prefix(VERSION_PREFIX)?).ok()?;
    if envelope.len() <= NONCE_BYTES + TAG_BYTES {
        return None;
    }
    let (nonce, sealed) = envelope.split_at(NONCE_BYTES);
    let cipher = XChaCha20Poly1305::new(&Key::from(key.subkey(ITEM_INFO)));
    let aad = item_aad(&key.id);
    let plain = cipher
        .decrypt(
            &XNonce::try_from(nonce).ok()?,
            Payload {
                msg: sealed,
                aad: &aad,
            },
        )
        .ok()?;
    let text = String::from_utf8(plain).ok()?;
    is_valid_text(&text).then_some(text)
}

fn wrap_aad(key_id: &str, device_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(WRAP_INFO.len() + 2 + key_id.len() + device_id.len());
    aad.extend_from_slice(WRAP_INFO);
    aad.push(0);
    aad.extend_from_slice(key_id.as_bytes());
    aad.push(0);
    aad.extend_from_slice(device_id.as_bytes());
    aad
}

fn wrap_with(
    key: &ClipboardKey,
    recipient: &[u8; 32],
    ephemeral: [u8; 32],
    nonce: [u8; NONCE_BYTES],
) -> Option<String> {
    let ephemeral = StaticSecret::from(ephemeral);
    let ephemeral_public = PublicKey::from(&ephemeral).to_bytes();
    let shared = ephemeral.diffie_hellman(&PublicKey::from(*recipient));
    if !shared.was_contributory() {
        return None;
    }
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(&ephemeral_public);
    salt[32..].copy_from_slice(recipient);
    let cipher = XChaCha20Poly1305::new(&Key::from(hkdf32(
        shared.as_bytes(),
        Some(&salt),
        WRAP_INFO,
    )));
    let aad = wrap_aad(&key.id, &device_id(recipient));
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

fn wrap_key(key: &ClipboardKey, recipient: &[u8; 32]) -> Result<Option<String>, RandomUnavailable> {
    Ok(wrap_with(key, recipient, random()?, random()?))
}

fn unwrap_key(secret: &StaticSecret, key_id: &str, wrap: &str) -> Option<ClipboardKey> {
    let envelope: [u8; WRAP_BYTES] = B64
        .decode(wrap.strip_prefix(VERSION_PREFIX)?)
        .ok()?
        .try_into()
        .ok()?;
    let mut ephemeral_public = [0u8; 32];
    ephemeral_public.copy_from_slice(&envelope[..32]);
    let own_public = PublicKey::from(secret).to_bytes();
    let shared = secret.diffie_hellman(&PublicKey::from(ephemeral_public));
    if !shared.was_contributory() {
        return None;
    }
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(&ephemeral_public);
    salt[32..].copy_from_slice(&own_public);
    let cipher = XChaCha20Poly1305::new(&Key::from(hkdf32(
        shared.as_bytes(),
        Some(&salt),
        WRAP_INFO,
    )));
    let aad = wrap_aad(key_id, &device_id(&own_public));
    let plain = cipher
        .decrypt(
            &XNonce::try_from(&envelope[32..32 + NONCE_BYTES]).ok()?,
            Payload {
                msg: &envelope[32 + NONCE_BYTES..],
                aad: &aad,
            },
        )
        .ok()?;
    let key = ClipboardKey::new(plain.try_into().ok()?);
    // 封装的 AAD 已经绑定了编号，这里再核对一次：解出的密钥必须就是服务端声称的那一把。
    (key.id == key_id).then_some(key)
}

// ---- 驱动 ----

/// 宿主要发出的请求。一律带账号凭据；`body` 是 JSON 文本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub body: Option<String>,
}

/// 宿主交回的响应。传输失败（断网、超时）不交回，由宿主按原来的方式报错。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    #[serde(default)]
    pub body: String,
}

/// 宿主可以发起的操作。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    List {
        #[serde(default)]
        search: String,
    },
    Add {
        text: String,
    },
    Status,
    Approve {
        device_id: String,
    },
    Reject {
        device_id: String,
    },
    Remove {
        device_id: String,
    },
    Reset,
}

/// 操作没有完成的原因。`status` 为 0 表示不是服务端返回的错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverError {
    pub status: u16,
    pub code: String,
}

impl DriverError {
    fn local(code: &str) -> Self {
        Self {
            status: 0,
            code: code.to_owned(),
        }
    }

    fn invalid_response() -> Self {
        Self::local("invalid_response")
    }

    /// 按服务端的错误正文（`{"error":{"code":...}}`）得出错误码，读不出时是 `http_<状态码>`。自建中继的直连接口（`msime_client_services::clipboard_relay`）也这样读。
    #[doc(hidden)]
    pub fn from_response(response: &HttpResponse) -> Self {
        #[derive(Deserialize)]
        struct Body {
            error: Inner,
        }
        #[derive(Deserialize)]
        struct Inner {
            code: String,
        }
        let code = serde_json::from_str::<Body>(&response.body)
            .ok()
            .map(|body| body.error.code)
            .filter(|code| crate::is_bounded_ascii_identifier(code, 64))
            .unwrap_or_else(|| format!("http_{}", response.status));
        Self {
            status: response.status,
            code,
        }
    }
}

/// 驱动的一步。宿主收到 `Request` 就发请求并调用 [`resume`]，收到另外两种就结束。每一步都附带最新的密钥串，宿主看到与已保存的不同就立即保存。
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
    key_rounds: u8,
    #[serde(default)]
    registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
enum Phase {
    /// 等待 `GET …/keys`。
    Keys,
    /// 等待登记本机。
    Register,
    /// 等待新密钥写入；成功后本机采用它。`finish` 为真时随后只回报状态，否则继续原来的操作。
    CreateKey { key: String, finish: bool },
    /// 等待条目列表。
    List { pending_devices: usize },
    /// 等待上传结果。
    Add,
    /// 等待批准或拒绝的结果，之后回报状态。
    Changed,
    /// 等待最后一次取密钥状态，用来回报状态。
    Final,
}

#[derive(Debug, Deserialize)]
struct KeysResponse {
    enabled: bool,
    key_id: Option<String>,
    wrap: Option<String>,
    #[serde(default)]
    devices: Vec<ServerDevice>,
}

#[derive(Debug, Clone, Deserialize)]
struct ServerDevice {
    device_id: String,
    public_key: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    approved: bool,
}

impl ServerDevice {
    /// 公钥与编号一致才算数：核对码按公钥算，封装也按公钥做，两者对不上的设备不显示也不封装。
    fn verified_public_key(&self) -> Option<[u8; 32]> {
        let public_key = decode_exact::<32>(&self.public_key)?;
        (device_id(&public_key) == self.device_id).then_some(public_key)
    }
}

#[derive(Debug, Deserialize)]
struct ServerPage {
    enabled: bool,
    #[serde(default)]
    retention_days: i64,
    #[serde(default)]
    items: Vec<ServerItem>,
}

#[derive(Debug, Deserialize)]
struct ServerItem {
    id: String,
    ciphertext: String,
    key_id: String,
    updated_at: String,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    device: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readiness {
    Ready,
    Pending,
    NoKey,
}

fn sanitize_device_name(name: &str) -> String {
    name.chars()
        .filter(|character| !character.is_control())
        .take(MAX_DEVICE_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    crate::text::is_lower_hex(value, length)
}

fn keys_path(device_id: &str) -> String {
    format!("{API}/keys?device_id={device_id}")
}

/// 开始一个操作。`keystore` 是宿主保存的密钥串，第一次使用或解析不了时生成新的设备身份。
pub fn start(
    operation: Operation,
    keystore: Option<&str>,
    device_name: &str,
) -> Result<Step, RandomUnavailable> {
    let keystore = match keystore.and_then(Keystore::parse) {
        Some(keystore) => keystore,
        None => Keystore::generate()?,
    };
    let encoded = keystore.encode();
    let mut state = DriverState {
        operation,
        device_name: sanitize_device_name(device_name),
        keystore: encoded,
        phase: Phase::Keys,
        key_rounds: 0,
        registered: false,
    };
    if let Some(code) = invalid_operation(&state.operation, &keystore.device_id()) {
        return Ok(fail(&state, DriverError::local(code)));
    }
    if let Operation::Reject { device_id } = &state.operation {
        // 拒绝一台待批准的设备只是删掉它的登记，不需要本机持有密钥。
        let path = format!("{API}/devices/{device_id}");
        state.phase = Phase::Changed;
        return Ok(request(state, "DELETE", path, None));
    }
    Ok(fetch_keys(state))
}

fn invalid_operation(operation: &Operation, own_id: &str) -> Option<&'static str> {
    match operation {
        Operation::Add { text } if !is_valid_text(text) => Some("invalid_clipboard_text"),
        Operation::List { search } if !crate::is_bounded_text(search, 1024) => {
            Some("invalid_search")
        }
        Operation::Approve { device_id }
        | Operation::Reject { device_id }
        | Operation::Remove { device_id }
            if !is_lower_hex(device_id, 32) || device_id == own_id =>
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
    match std::mem::replace(&mut state.phase, Phase::Keys) {
        Phase::Keys => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Ok(keys) = serde_json::from_str::<KeysResponse>(&response.body) else {
                return Ok(fail(&state, DriverError::invalid_response()));
            };
            handle_keys(state, keystore, keys)
        }
        Phase::Register => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            Ok(fetch_keys(state))
        }
        Phase::CreateKey { key, finish } => {
            if response.status == 409 && !finish {
                // 另一台设备同时建好了密钥：重新取状态，本机会变成待批准。
                return Ok(fetch_keys(state));
            }
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Some(bytes) = decode_exact::<32>(&key) else {
                return Ok(fail(&state, DriverError::local("invalid_state")));
            };
            keystore.key = Some(ClipboardKey::new(bytes));
            state.keystore = keystore.encode();
            if finish {
                state.phase = Phase::Final;
                Ok(request(
                    state.clone(),
                    "GET",
                    keys_path(&keystore.device_id()),
                    None,
                ))
            } else {
                Ok(fetch_keys(state))
            }
        }
        Phase::List { pending_devices } => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Ok(page) = serde_json::from_str::<ServerPage>(&response.body) else {
                return Ok(fail(&state, DriverError::invalid_response()));
            };
            let Operation::List { search } = &state.operation else {
                return Ok(fail(&state, DriverError::local("invalid_state")));
            };
            match open_page(&keystore, page, search, pending_devices) {
                Some(result) => Ok(done(&state, result)),
                None => Ok(fail(&state, DriverError::invalid_response())),
            }
        }
        Phase::Add => {
            if response.status == 409 {
                // 上传期间密钥换了：重新取状态，拿到新密钥后再传一次。
                return Ok(fetch_keys(state));
            }
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Ok(item) = serde_json::from_str::<ServerItem>(&response.body) else {
                return Ok(fail(&state, DriverError::invalid_response()));
            };
            match open_server_item(&keystore, &item) {
                Some(Some(item)) => Ok(done(&state, item)),
                _ => Ok(fail(&state, DriverError::invalid_response())),
            }
        }
        Phase::Changed => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            state.phase = Phase::Final;
            Ok(request(
                state,
                "GET",
                keys_path(&keystore.device_id()),
                None,
            ))
        }
        Phase::Final => {
            if !success {
                return Ok(fail(&state, DriverError::from_response(&response)));
            }
            let Ok(keys) = serde_json::from_str::<KeysResponse>(&response.body) else {
                return Ok(fail(&state, DriverError::invalid_response()));
            };
            let readiness = evaluate(&mut keystore, &keys);
            state.keystore = keystore.encode();
            let view = status_view(&keystore, &keys, readiness);
            Ok(done(&state, view))
        }
    }
}

fn fetch_keys(mut state: DriverState) -> Step {
    state.key_rounds += 1;
    if state.key_rounds > MAX_KEY_ROUNDS {
        return fail(&state, DriverError::local("clipboard_key_conflict"));
    }
    let path = match Keystore::parse(&state.keystore) {
        Some(keystore) => keys_path(&keystore.device_id()),
        None => return fail(&state, DriverError::local("invalid_state")),
    };
    state.phase = Phase::Keys;
    request(state, "GET", path, None)
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

/// 根据服务端的密钥状态判断本机能不能读写，必要时解开封装并更新本机密钥。
fn evaluate(keystore: &mut Keystore, keys: &KeysResponse) -> Readiness {
    let Some(key_id) = keys.key_id.as_deref().filter(|id| is_lower_hex(id, 32)) else {
        keystore.key = None;
        return Readiness::NoKey;
    };
    if keystore.key_id() == Some(key_id) {
        return Readiness::Ready;
    }
    keystore.key = keys
        .wrap
        .as_deref()
        .and_then(|wrap| unwrap_key(&keystore.secret, key_id, wrap));
    if keystore.key.is_some() {
        Readiness::Ready
    } else {
        Readiness::Pending
    }
}

fn handle_keys(
    mut state: DriverState,
    mut keystore: Keystore,
    keys: KeysResponse,
) -> Result<Step, RandomUnavailable> {
    if !keys.enabled {
        return Ok(match &state.operation {
            Operation::List { .. } => {
                let view = list_view(&keystore, "disabled", Vec::new(), 0, 0, 0);
                done(&state, view)
            }
            Operation::Status => {
                let view = status_view(&keystore, &keys, Readiness::Pending);
                done(&state, view)
            }
            _ => fail(
                &state,
                DriverError {
                    status: 403,
                    code: "clipboard_sync_disabled".to_owned(),
                },
            ),
        });
    }
    let own_id = keystore.device_id();
    if !keys.devices.iter().any(|device| device.device_id == own_id) {
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
    if let Operation::Reset | Operation::Remove { .. } = &state.operation {
        let revoke = match &state.operation {
            Operation::Remove { device_id } => vec![device_id.clone()],
            _ => Vec::new(),
        };
        return create_key(state, &keystore, keys.key_id.clone(), revoke, true);
    }
    let readiness = evaluate(&mut keystore, &keys);
    state.keystore = keystore.encode();
    match readiness {
        Readiness::NoKey => create_key(state, &keystore, None, Vec::new(), false),
        Readiness::Pending => Ok(match &state.operation {
            Operation::List { .. } => {
                let view = list_view(&keystore, "pending", Vec::new(), 0, 0, 0);
                done(&state, view)
            }
            Operation::Status => {
                let view = status_view(&keystore, &keys, readiness);
                done(&state, view)
            }
            _ => fail(&state, DriverError::local("clipboard_device_pending")),
        }),
        Readiness::Ready => {
            let key = keystore.key.clone().expect("ready implies a key");
            match state.operation.clone() {
                Operation::List { .. } => {
                    let pending_devices = keys
                        .devices
                        .iter()
                        .filter(|device| {
                            device.device_id != own_id
                                && !device.approved
                                && device.verified_public_key().is_some()
                        })
                        .count();
                    state.phase = Phase::List { pending_devices };
                    Ok(request(state, "GET", API.to_owned(), None))
                }
                Operation::Add { text } => {
                    let sealed = seal_with_nonce(&key, &text, random()?);
                    state.phase = Phase::Add;
                    let body = serde_json::to_value(sealed).expect("sealed item serialises");
                    Ok(request(state, "POST", API.to_owned(), Some(body)))
                }
                Operation::Status => {
                    let view = status_view(&keystore, &keys, readiness);
                    Ok(done(&state, view))
                }
                Operation::Approve { device_id } => {
                    let Some(device) = keys
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
                    let Some(wrap) = wrap_key(&key, &public_key)? else {
                        return Ok(fail(&state, DriverError::invalid_response()));
                    };
                    state.phase = Phase::Changed;
                    let path = format!("{API}/keys/{}/wraps/{device_id}", key.id);
                    Ok(request(state, "PUT", path, Some(json!({ "wrap": wrap }))))
                }
                Operation::Reject { .. } | Operation::Remove { .. } | Operation::Reset => {
                    Ok(fail(&state, DriverError::local("invalid_state")))
                }
            }
        }
    }
}

/// 生成一把新密钥，只封装给本机，请求服务端换上它。
fn create_key(
    mut state: DriverState,
    keystore: &Keystore,
    previous_key_id: Option<String>,
    revoke: Vec<String>,
    finish: bool,
) -> Result<Step, RandomUnavailable> {
    let key = ClipboardKey::new(random()?);
    let Some(wrap) = wrap_key(&key, &keystore.public_key())? else {
        return Ok(fail(&state, DriverError::local("invalid_state")));
    };
    let body = json!({
        "previous_key_id": previous_key_id,
        "key_id": key.id,
        "wraps": [{ "device_id": keystore.device_id(), "wrap": wrap }],
        "revoke": revoke,
    });
    state.phase = Phase::CreateKey {
        key: B64.encode(key.bytes),
        finish,
    };
    Ok(request(state, "POST", format!("{API}/keys"), Some(body)))
}

fn list_view(
    keystore: &Keystore,
    status: &str,
    items: Vec<Value>,
    retention_days: i64,
    pending_devices: usize,
    undecryptable: usize,
) -> Value {
    json!({
        "enabled": status != "disabled",
        "retention_days": retention_days,
        "items": items,
        "e2e": {
            "status": status,
            "code": keystore.verification_code(),
            "pending_devices": pending_devices,
            "undecryptable": undecryptable,
        },
    })
}

fn status_view(keystore: &Keystore, keys: &KeysResponse, readiness: Readiness) -> Value {
    let own_id = keystore.device_id();
    let status = if !keys.enabled {
        "disabled"
    } else if readiness == Readiness::Ready {
        "ready"
    } else {
        "pending"
    };
    let devices = keys
        .devices
        .iter()
        .take(MAX_DEVICES)
        .filter_map(|device| {
            let public_key = device.verified_public_key()?;
            Some(json!({
                "device_id": device.device_id,
                "name": sanitize_device_name(&device.name),
                "created_at": if crate::is_bounded_text(&device.created_at, 128) {
                    device.created_at.as_str()
                } else {
                    ""
                },
                "approved": device.approved,
                "current": device.device_id == own_id,
                "code": verification_code(&public_key),
            }))
        })
        .collect::<Vec<_>>();
    json!({
        "enabled": keys.enabled,
        "status": status,
        "device_id": own_id,
        "code": keystore.verification_code(),
        "devices": devices,
    })
}

/// 解开一条服务端条目。结构不合法返回 `None`（整页作废），只是解不开返回 `Some(None)`。
fn open_server_item(keystore: &Keystore, item: &ServerItem) -> Option<Option<Value>> {
    if !is_lower_hex(&item.id, 64)
        || item.updated_at.is_empty()
        || !crate::is_bounded_text(&item.updated_at, 128)
    {
        return None;
    }
    let text = keystore
        .key
        .as_ref()
        .filter(|key| key.id == item.key_id)
        .and_then(|key| open_item(key, &item.ciphertext));
    Some(text.map(|text| {
        json!({
            "id": item.id,
            "text": text,
            "updated_at": item.updated_at,
            "pinned": item.pinned,
            "device": sanitize_device_name(&item.device),
        })
    }))
}

fn open_page(
    keystore: &Keystore,
    page: ServerPage,
    search: &str,
    pending_devices: usize,
) -> Option<Value> {
    if page.items.len() > MAX_ITEMS {
        return None;
    }
    let needle = search.to_lowercase();
    let mut items = Vec::with_capacity(page.items.len());
    let mut undecryptable = 0;
    for item in &page.items {
        match open_server_item(keystore, item)? {
            Some(value) => {
                let matches = needle.is_empty()
                    || value["text"]
                        .as_str()
                        .is_some_and(|text| text.to_lowercase().contains(&needle));
                if matches {
                    items.push(value);
                }
            }
            None => undecryptable += 1,
        }
    }
    let status = if page.enabled { "ready" } else { "disabled" };
    Some(list_view(
        keystore,
        status,
        items,
        page.retention_days,
        pending_devices,
        undecryptable,
    ))
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

/// C ABI、JNI 和 NAPI 共用的 JSON 入口，格式见 `docs/cloud-clipboard-e2e.md` 的「客户端驱动」。
pub fn drive_json(input: &[u8]) -> Result<Value, String> {
    let value: Value =
        serde_json::from_slice(input).map_err(|_| "invalid cloud clipboard e2e request")?;
    let step = if value.get("start").is_some() {
        let input: StartInput =
            serde_json::from_value(value).map_err(|_| "invalid cloud clipboard e2e request")?;
        start(input.start, input.keystore.as_deref(), &input.device_name)
    } else {
        let input: ResumeInput =
            serde_json::from_value(value).map_err(|_| "invalid cloud clipboard e2e request")?;
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

/// 在 Rust 里直接驱动一个操作（桌面端）：`send` 发请求，`persist` 保存每一步交回的密钥串。外层错误是传输错误，内层是操作结果。
pub fn run<E>(
    operation: Operation,
    keystore: Option<&str>,
    device_name: &str,
    mut send: impl FnMut(&HttpRequest) -> Result<HttpResponse, E>,
    mut persist: impl FnMut(&str) -> Result<(), E>,
    random_unavailable: impl FnOnce() -> E,
) -> Result<Result<Value, DriverError>, E> {
    let mut saved = keystore.map(str::to_owned);
    let mut step = match start(operation, keystore, device_name) {
        Ok(step) => step,
        Err(RandomUnavailable) => return Err(random_unavailable()),
    };
    loop {
        let current = match &step {
            Step::Request { keystore, .. }
            | Step::Done { keystore, .. }
            | Step::Failed { keystore, .. } => keystore,
        };
        if saved.as_deref() != Some(current.as_str()) {
            persist(current)?;
            saved = Some(current.clone());
        }
        match step {
            Step::Request { request, state, .. } => {
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
#[path = "clipboard_e2e_tests.rs"]
mod tests;
