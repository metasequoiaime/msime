//! 云剪贴板中继接口的参考语义，以及 WebDAV 后端用的只追加事件日志。都不碰网络。
//!
//! 两层：
//!
//! - 请求的解析和校验（[`parse_request`]）：`docs/cloud-clipboard-e2e.md` 里客户端用到的那 12 个接口，按路径、方法和正文得出一个 [`RelayRequest`]，不合规时直接得出协议规定的错误响应。参考中继（`msime-clipboard-relay` 的 `store`）和这里的日志共用这一份，两边的错误码因此不会漂移。
//! - 日志（[`Ledger`]）：WebDAV 网盘上没有能在服务端执行的逻辑，客户端把每个改变状态的请求写成一个不可变的事件文件，序号是唯一的文件名，谁抢到下一个序号谁的写入就算数（独占创建由传输层保证，见 `docs/cloud-clipboard-webdav.md`）。每台设备按序号重放事件，得出与参考中继同样的状态：空间、设备登记（20 台上限，满了淘汰最早登记、尚未批准的一台）、按当前密钥核对的封装、换钥时比较 `previous_key_id`、去重时保留原来的条目编号并移到最前、50 条上限（全都置顶时答 409 `clipboard_full`）、置顶、删除、清空、关闭开关和保留期。
//!
//! 事件之间用哈希串起来（`prev` 是上一个事件文件字节的 SHA-256），重放时核对：本机记得的最后一个事件之后接上的不是它，说明日志被改写或删过（例如有人删掉一次换钥，想让旧密钥复活），或者网盘的独占创建并不可靠，两种情况都不能悄悄接受。
//!
//! 事件攒多了以后压缩：一个 `seal` 事件抢下一个序号，关闭当前的 epoch；下一个 epoch 的 `snapshot.json` 是封口那一刻的完整状态，内容由日志唯一决定，几台设备同时写出的是同一份字节。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const API: &str = "/v1/users/me/clipboard";
/// 每个空间最多登记的设备数，也是一次换钥最多带的封装数。
pub const MAX_DEVICES: usize = 20;
/// 每个空间最多保留的条目数。
pub const MAX_ITEMS: usize = 50;
/// 条目密文的最大字符数（含 `v1.` 前缀）。
pub use super::clipboard_e2e::MAX_CIPHERTEXT_CHARS;
/// 上传一条的请求体上限，与官方服务端相同。
pub const MAX_UPLOAD_BYTES: usize = 32768;
/// 密文解码后的最小字节数：24 字节 nonce、16 字节 tag 和至少 1 字节明文。
const MIN_CIPHERTEXT_BYTES: usize = 24 + 16 + 1;
/// 封装解码后的字节数：32 字节临时公钥、24 字节 nonce、32 字节密钥和 16 字节 tag。
const WRAP_BYTES: usize = 32 + 24 + 32 + 16;
const MAX_DEVICE_NAME_CHARS: usize = 64;
/// 保留期允许的天数，0 表示一直保留。
pub const RETENTION_DAYS: [i64; 4] = [0, 1, 7, 30];
/// 一个 epoch 里攒到这么多事件就压缩。
pub const COMPACT_AFTER_EVENTS: u32 = 64;
/// 日志格式的名字和版本，写在 `format.json` 和每份快照里。
pub const FORMAT: &str = "msime-clipboard";
pub const FORMAT_VERSION: u32 = 1;
/// 第一个 epoch 的快照之前没有事件，链头是 64 个零。
pub const GENESIS_HEAD: &str = "0000000000000000000000000000000000000000000000000000000000000000";

// ---- 响应 ----

/// 状态码和 JSON 正文（204 没有正文）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Option<String>,
}

impl Response {
    pub fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            body: Some(value.to_string()),
        }
    }

    pub fn no_content() -> Self {
        Self {
            status: 204,
            body: None,
        }
    }

    /// 协议规定的错误正文：`{"error":{"code":…,"message":…}}`。
    pub fn error(status: u16, code: &str) -> Self {
        Self::json(
            status,
            json!({ "error": { "code": code, "message": code.replace('_', " ") } }),
        )
    }
}

fn reply(status: u16, code: &str) -> Response {
    Response::error(status, code)
}

// ---- 输入校验 ----

pub fn is_lower_hex(value: &str, length: usize) -> bool {
    crate::text::is_lower_hex(value, length)
}

/// 不带填充的 base64url，按规范写法严格解码：字符集之外的字符、填充和多余的尾部位都不接受，每个值只有一种写法。
fn decode_b64(value: &str) -> Option<Vec<u8>> {
    B64.decode(value).ok()
}

/// 设备编号是公钥 SHA-256 的前 16 字节，公钥是 32 字节的 base64url。
pub fn valid_device(device_id: &str, public_key: &str) -> bool {
    is_lower_hex(device_id, 32)
        && decode_b64(public_key).is_some_and(|key| {
            key.len() == 32 && hex::encode(&Sha256::digest(&key)[..16]) == device_id
        })
}

pub fn valid_device_name(name: &str) -> bool {
    name.chars().count() <= MAX_DEVICE_NAME_CHARS && !name.chars().any(char::is_control)
}

pub fn valid_wrap(wrap: &str) -> bool {
    wrap.strip_prefix("v1.")
        .and_then(decode_b64)
        .is_some_and(|bytes| bytes.len() == WRAP_BYTES)
}

pub fn valid_ciphertext(ciphertext: &str) -> bool {
    ciphertext.len() <= MAX_CIPHERTEXT_CHARS
        && ciphertext
            .strip_prefix("v1.")
            .and_then(decode_b64)
            .is_some_and(|bytes| bytes.len() >= MIN_CIPHERTEXT_BYTES)
}

// ---- 请求 ----

/// 换钥请求里给一台设备的封装。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrapEntry {
    pub device_id: String,
    pub wrap: String,
}

/// 一个解析过、通过了不依赖状态的校验的请求。依赖状态的检查（开关、当前密钥、设备是否登记）由各自的实现做。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayRequest {
    /// `GET /clipboard`
    List,
    /// `POST /clipboard`
    Add {
        ciphertext: String,
        key_id: String,
        dedup: String,
    },
    /// `DELETE /clipboard`
    Clear,
    /// `GET /keys?device_id=`
    Keys { device_id: String },
    /// `POST /keys`
    Rotate {
        previous_key_id: Option<String>,
        key_id: String,
        wraps: Vec<WrapEntry>,
        revoke: Vec<String>,
    },
    /// `PUT /keys/{key_id}/wraps/{device_id}`
    Approve {
        key_id: String,
        device_id: String,
        wrap: String,
    },
    /// `PUT /devices/{device_id}`
    Register {
        device_id: String,
        public_key: String,
        name: String,
    },
    /// `DELETE /devices/{device_id}`
    Reject { device_id: String },
    /// `PUT /settings`
    SetEnabled { enabled: bool },
    /// `PUT /retention`
    SetRetention { days: i64 },
    /// `DELETE /clipboard/{id}`
    Delete { id: String },
    /// `PUT /clipboard/{id}/pin`
    Pin { id: String, pinned: bool },
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, Response> {
    serde_json::from_slice(body).map_err(|_| reply(400, "invalid_json"))
}

/// 按方法、路径（去掉基地址之后，可以带查询串）和正文得出请求；路由不认识、正文不合规时得出协议规定的错误响应。
pub fn parse_request(method: &str, path: &str, body: &[u8]) -> Result<RelayRequest, Response> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let Some(rest) = path.strip_prefix(API) else {
        return Err(reply(404, "not_found"));
    };
    let segments: Vec<&str> = rest.split('/').skip(1).collect();
    if !rest.is_empty() && !rest.starts_with('/') {
        return Err(reply(404, "not_found"));
    }
    let request = match (method, segments.as_slice()) {
        ("GET", []) => RelayRequest::List,
        ("POST", []) => parse_add(body)?,
        ("DELETE", []) => RelayRequest::Clear,
        ("GET", ["keys"]) => RelayRequest::Keys {
            device_id: query
                .split('&')
                .find_map(|pair| pair.strip_prefix("device_id="))
                .unwrap_or_default()
                .to_owned(),
        },
        ("POST", ["keys"]) => parse_rotate(body)?,
        ("PUT", ["keys", key_id, "wraps", device_id]) => {
            #[derive(Deserialize)]
            struct Body {
                #[serde(default)]
                wrap: String,
            }
            let body: Body = parse(body)?;
            RelayRequest::Approve {
                key_id: (*key_id).to_owned(),
                device_id: (*device_id).to_owned(),
                wrap: body.wrap,
            }
        }
        ("PUT", ["devices", device_id]) => {
            #[derive(Deserialize)]
            struct Body {
                #[serde(default)]
                public_key: String,
                #[serde(default)]
                name: String,
            }
            let body: Body = parse(body)?;
            RelayRequest::Register {
                device_id: (*device_id).to_owned(),
                public_key: body.public_key,
                name: body.name,
            }
        }
        ("DELETE", ["devices", device_id]) => RelayRequest::Reject {
            device_id: (*device_id).to_owned(),
        },
        ("PUT", ["settings"]) => {
            #[derive(Deserialize)]
            struct Body {
                enabled: Option<bool>,
            }
            let body: Body = parse(body)?;
            let Some(enabled) = body.enabled else {
                return Err(reply(400, "enabled_required"));
            };
            RelayRequest::SetEnabled { enabled }
        }
        ("PUT", ["retention"]) => {
            #[derive(Deserialize)]
            struct Body {
                days: Option<i64>,
            }
            let body: Body = parse(body)?;
            RelayRequest::SetRetention {
                days: body.days.unwrap_or(-1),
            }
        }
        ("DELETE", [id]) => RelayRequest::Delete {
            id: (*id).to_owned(),
        },
        ("PUT", [id, "pin"]) => {
            #[derive(Deserialize)]
            struct Body {
                pinned: Option<bool>,
            }
            let body: Body = parse(body)?;
            let Some(pinned) = body.pinned else {
                return Err(reply(400, "pinned_required"));
            };
            RelayRequest::Pin {
                id: (*id).to_owned(),
                pinned,
            }
        }
        _ => return Err(reply(404, "not_found")),
    };
    request.check()?;
    Ok(request)
}

fn parse_add(body: &[u8]) -> Result<RelayRequest, Response> {
    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        text: String,
        #[serde(default)]
        ciphertext: String,
        #[serde(default)]
        key_id: String,
        #[serde(default)]
        dedup: String,
    }
    if body.len() > MAX_UPLOAD_BYTES {
        return Err(reply(413, "request_too_large"));
    }
    let body: Body = parse(body)?;
    if !body.text.is_empty() {
        return Err(reply(400, "clipboard_e2e_required"));
    }
    Ok(RelayRequest::Add {
        ciphertext: body.ciphertext,
        key_id: body.key_id,
        dedup: body.dedup,
    })
}

fn parse_rotate(body: &[u8]) -> Result<RelayRequest, Response> {
    #[derive(Deserialize)]
    struct Body {
        #[serde(default)]
        previous_key_id: Option<String>,
        key_id: String,
        wraps: Vec<WrapEntry>,
        #[serde(default)]
        revoke: Vec<String>,
    }
    let body: Body =
        serde_json::from_slice(body).map_err(|_| reply(400, "invalid_clipboard_key"))?;
    Ok(RelayRequest::Rotate {
        previous_key_id: body.previous_key_id,
        key_id: body.key_id,
        wraps: body.wraps,
        revoke: body.revoke,
    })
}

impl RelayRequest {
    /// 不依赖状态的校验：形状、长度、公钥与编号一致、封装和密文能解码。日志重放时每个事件也过一遍。
    pub fn check(&self) -> Result<(), Response> {
        match self {
            Self::Add {
                ciphertext, dedup, ..
            } if !valid_ciphertext(ciphertext) || !is_lower_hex(dedup, 64) => {
                Err(reply(400, "invalid_clipboard_ciphertext"))
            }
            Self::Keys { device_id } if !is_lower_hex(device_id, 32) => {
                Err(reply(400, "invalid_device_id"))
            }
            Self::Rotate {
                key_id,
                wraps,
                revoke,
                ..
            } => {
                let mut seen = std::collections::HashSet::new();
                let well_formed = is_lower_hex(key_id, 32)
                    && (1..=MAX_DEVICES).contains(&wraps.len())
                    && revoke.len() <= MAX_DEVICES
                    && revoke.iter().all(|id| is_lower_hex(id, 32))
                    && wraps.iter().all(|wrap| {
                        is_lower_hex(&wrap.device_id, 32)
                            && valid_wrap(&wrap.wrap)
                            && !revoke.contains(&wrap.device_id)
                            && seen.insert(wrap.device_id.as_str())
                    });
                if well_formed {
                    Ok(())
                } else {
                    Err(reply(400, "invalid_clipboard_key"))
                }
            }
            Self::Approve { wrap, .. } if !valid_wrap(wrap) => {
                Err(reply(400, "invalid_clipboard_key"))
            }
            Self::Register {
                device_id,
                public_key,
                name,
            } if !valid_device(device_id, public_key) || !valid_device_name(name) => {
                Err(reply(400, "invalid_clipboard_device"))
            }
            Self::SetRetention { days } if !RETENTION_DAYS.contains(days) => {
                Err(reply(400, "invalid_retention"))
            }
            Self::Pin { id, .. } if !is_lower_hex(id, 64) => Err(reply(404, "not_found")),
            _ => Ok(()),
        }
    }

    /// 是否会改变状态。读取（`List`、`Keys`）不写日志。
    pub fn is_write(&self) -> bool {
        !matches!(self, Self::List | Self::Keys { .. })
    }
}

// ---- 状态 ----

/// 一个空间的全部状态，与参考中继的数据库表一一对应。字段顺序和集合的顺序都是确定的，序列化出的快照因此由日志唯一决定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogState {
    pub enabled: bool,
    pub retention_days: i64,
    pub key_id: Option<String>,
    /// 密钥换过几次（换钥和关闭开关各算一次）。本机记住见过的最大值，变小就是日志被回退了。
    pub generation: u64,
    /// 按登记先后排列。
    pub devices: Vec<LogDevice>,
    /// 设备编号到它的封装。封装属于哪把密钥记在里面，只有属于当前密钥的才算「已批准」。
    pub wraps: BTreeMap<String, LogWrap>,
    /// 按写入序号从小到大排列。
    pub items: Vec<LogItem>,
}

impl Default for LogState {
    /// 新空间默认开启，还没有密钥，与参考中继相同。
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 0,
            key_id: None,
            generation: 0,
            devices: Vec::new(),
            wraps: BTreeMap::new(),
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogDevice {
    pub device_id: String,
    pub public_key: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogWrap {
    pub key_id: String,
    pub wrap: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogItem {
    pub id: String,
    pub ciphertext: String,
    pub key_id: String,
    pub dedup: String,
    /// 最后一次写入它的事件序号，排序用。
    pub sequence: u64,
    pub updated_at: String,
    pub updated_unix: i64,
    pub pinned: bool,
}

/// Unix 秒数写成 RFC 3339（UTC，精确到秒），与参考中继的时间格式相同。
pub fn rfc3339(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default()
}

impl LogState {
    fn approved(&self, device_id: &str) -> bool {
        match (&self.key_id, self.wraps.get(device_id)) {
            (Some(key_id), Some(wrap)) => &wrap.key_id == key_id,
            _ => false,
        }
    }

    fn device_exists(&self, device_id: &str) -> bool {
        self.devices
            .iter()
            .any(|device| device.device_id == device_id)
    }

    fn expired(&self, item: &LogItem, now: i64) -> bool {
        self.retention_days > 0
            && !item.pinned
            && item.updated_unix < now - self.retention_days * 86_400
    }

    /// 保留期过了的未置顶条目。参考中继在每次读写条目时删掉它们；这里读取时滤掉，写入时删掉。
    pub fn has_expired_items(&self, now: i64) -> bool {
        self.items.iter().any(|item| self.expired(item, now))
    }

    fn prune(&mut self, now: i64) {
        if self.retention_days > 0 {
            let cutoff = now - self.retention_days * 86_400;
            self.items
                .retain(|item| item.pinned || item.updated_unix >= cutoff);
        }
    }

    fn item_json(item: &LogItem) -> Value {
        json!({
            "id": item.id,
            "ciphertext": item.ciphertext,
            "key_id": item.key_id,
            "updated_at": item.updated_at,
            "pinned": item.pinned,
            // 与参考中继相同：没有登录会话，认不出写入的设备。
            "device": "",
        })
    }

    /// 置顶的在前，同组里新写入的在前。
    fn ordered(&self) -> Vec<&LogItem> {
        let mut items: Vec<&LogItem> = self.items.iter().collect();
        items.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.sequence.cmp(&a.sequence))
        });
        items
    }

    fn keys(&self, device_id: &str) -> Response {
        let wrap = self.key_id.as_ref().and_then(|key_id| {
            self.wraps
                .get(device_id)
                .filter(|wrap| &wrap.key_id == key_id)
                .map(|wrap| wrap.wrap.clone())
        });
        let devices: Vec<Value> = self
            .devices
            .iter()
            .map(|device| {
                json!({
                    "device_id": device.device_id,
                    "public_key": device.public_key,
                    "name": device.name,
                    "created_at": device.created_at,
                    "approved": self.approved(&device.device_id),
                })
            })
            .collect();
        Response::json(
            200,
            json!({
                "enabled": self.enabled,
                "key_id": self.key_id,
                "wrap": wrap,
                "devices": devices,
            }),
        )
    }

    fn list(&self, now: i64) -> Response {
        let items: Vec<Value> = match &self.key_id {
            Some(key_id) => self
                .ordered()
                .into_iter()
                .filter(|item| &item.key_id == key_id && !self.expired(item, now))
                .take(MAX_ITEMS)
                .map(Self::item_json)
                .collect(),
            None => Vec::new(),
        };
        Response::json(
            200,
            json!({
                "enabled": self.enabled,
                "retention_days": self.retention_days,
                "key_id": self.key_id,
                "items": items,
            }),
        )
    }

    /// 只读的请求在当前状态上答；`now` 是本机时间，用来滤掉过了保留期的条目。
    pub fn read(&self, request: &RelayRequest, now: i64) -> Option<Response> {
        match request {
            RelayRequest::List => Some(self.list(now)),
            RelayRequest::Keys { device_id } => Some(self.keys(device_id)),
            _ => None,
        }
    }

    /// 把一个写入用到状态上，`at` 是写入那一刻的 Unix 秒数（来自事件），`sequence` 是事件序号，`item_id` 是新条目的编号（只有上传用）。失败时状态可能已经改了一半，调用方在副本上调用，失败就丢弃副本。
    fn write(
        &mut self,
        request: &RelayRequest,
        at: i64,
        sequence: u64,
        item_id: &str,
    ) -> Result<Response, Response> {
        match request {
            RelayRequest::List | RelayRequest::Keys { .. } => Err(reply(404, "not_found")),
            RelayRequest::Register {
                device_id,
                public_key,
                name,
            } => {
                if let Some(device) = self
                    .devices
                    .iter_mut()
                    .find(|device| &device.device_id == device_id)
                {
                    device.name = name.clone();
                    return Ok(Response::no_content());
                }
                if self.devices.len() >= MAX_DEVICES {
                    // 满了时先淘汰最早登记、还没被批准的一台；都已批准时拒绝。
                    let Some(index) = self
                        .devices
                        .iter()
                        .position(|device| !self.approved(&device.device_id))
                    else {
                        return Err(reply(409, "clipboard_device_limit"));
                    };
                    self.devices.remove(index);
                }
                self.wraps.remove(device_id);
                self.devices.push(LogDevice {
                    device_id: device_id.clone(),
                    public_key: public_key.clone(),
                    name: name.clone(),
                    created_at: rfc3339(at),
                });
                Ok(Response::no_content())
            }
            RelayRequest::Reject { device_id } => {
                let before = self.devices.len();
                self.devices.retain(|device| &device.device_id != device_id);
                if self.devices.len() == before {
                    return Err(reply(404, "not_found"));
                }
                self.wraps.remove(device_id);
                Ok(Response::no_content())
            }
            RelayRequest::Rotate {
                previous_key_id,
                key_id,
                wraps,
                revoke,
            } => {
                if !self.enabled {
                    return Err(reply(403, "clipboard_sync_disabled"));
                }
                if &self.key_id != previous_key_id {
                    return Err(reply(409, "clipboard_key_conflict"));
                }
                self.devices
                    .retain(|device| !revoke.contains(&device.device_id));
                if wraps
                    .iter()
                    .any(|wrap| !self.device_exists(&wrap.device_id))
                {
                    return Err(reply(400, "invalid_clipboard_key"));
                }
                self.items.clear();
                self.wraps.clear();
                self.key_id = Some(key_id.clone());
                self.generation += 1;
                for wrap in wraps {
                    self.wraps.insert(
                        wrap.device_id.clone(),
                        LogWrap {
                            key_id: key_id.clone(),
                            wrap: wrap.wrap.clone(),
                        },
                    );
                }
                Ok(Response::no_content())
            }
            RelayRequest::Approve {
                key_id,
                device_id,
                wrap,
            } => {
                if self.key_id.as_deref() != Some(key_id.as_str()) {
                    return Err(reply(409, "clipboard_key_conflict"));
                }
                if !self.device_exists(device_id) {
                    return Err(reply(404, "not_found"));
                }
                self.wraps.insert(
                    device_id.clone(),
                    LogWrap {
                        key_id: key_id.clone(),
                        wrap: wrap.clone(),
                    },
                );
                Ok(Response::no_content())
            }
            RelayRequest::Add {
                ciphertext,
                key_id,
                dedup,
            } => {
                if !self.enabled {
                    return Err(reply(403, "clipboard_sync_disabled"));
                }
                if self.key_id.as_deref() != Some(key_id.as_str()) {
                    return Err(reply(409, "clipboard_key_conflict"));
                }
                self.prune(at);
                // 不属于当前密钥的条目不会再出现在列表里，趁这次写入删掉，免得占用五十条的名额。
                self.items.retain(|item| &item.key_id == key_id);
                // 五十条都已置顶时，新的一条会在下面裁剪时被立刻删掉：明确答 409，而不是挤掉置顶的条目。去重标签落在一条置顶条目上时只是更新它，照常接受。
                let pinned = self.items.iter().filter(|item| item.pinned).count();
                let pinned_duplicate = self
                    .items
                    .iter()
                    .any(|item| &item.dedup == dedup && item.pinned);
                if pinned >= MAX_ITEMS && !pinned_duplicate {
                    return Err(reply(409, "clipboard_full"));
                }
                let updated_at = rfc3339(at);
                match self.items.iter().position(|item| &item.dedup == dedup) {
                    Some(index) => {
                        // 相同内容只保留一条：保留原来的编号和置顶，移到最前。
                        let mut item = self.items.remove(index);
                        item.ciphertext = ciphertext.clone();
                        item.key_id = key_id.clone();
                        item.sequence = sequence;
                        item.updated_at = updated_at;
                        item.updated_unix = at;
                        self.items.push(item);
                    }
                    None => self.items.push(LogItem {
                        id: item_id.to_owned(),
                        ciphertext: ciphertext.clone(),
                        key_id: key_id.clone(),
                        dedup: dedup.clone(),
                        sequence,
                        updated_at,
                        updated_unix: at,
                        pinned: false,
                    }),
                }
                let keep: std::collections::HashSet<String> = self
                    .ordered()
                    .into_iter()
                    .take(MAX_ITEMS)
                    .map(|item| item.id.clone())
                    .collect();
                self.items.retain(|item| keep.contains(&item.id));
                let item = self
                    .items
                    .iter()
                    .find(|item| &item.dedup == dedup)
                    .ok_or_else(|| reply(500, "internal"))?;
                Ok(Response::json(200, Self::item_json(item)))
            }
            RelayRequest::Clear => {
                self.items.clear();
                Ok(Response::no_content())
            }
            RelayRequest::Delete { id } => {
                let before = self.items.len();
                self.items.retain(|item| &item.id != id);
                if self.items.len() == before {
                    return Err(reply(404, "not_found"));
                }
                Ok(Response::no_content())
            }
            RelayRequest::Pin { id, pinned } => {
                let Some(item) = self.items.iter_mut().find(|item| &item.id == id) else {
                    return Err(reply(404, "not_found"));
                };
                item.pinned = *pinned;
                Ok(Response::no_content())
            }
            RelayRequest::SetRetention { days } => {
                self.retention_days = *days;
                Ok(Response::no_content())
            }
            RelayRequest::SetEnabled { enabled } => {
                self.enabled = *enabled;
                if !enabled {
                    // 关闭时连同条目、密钥、封装和全部设备登记一起删除，再次打开时由第一台设备重新建钥。
                    if self.key_id.take().is_some() {
                        self.generation += 1;
                    }
                    self.items.clear();
                    self.wraps.clear();
                    self.devices.clear();
                }
                Ok(Response::json(200, json!({ "enabled": enabled })))
            }
        }
    }

    /// 快照里的状态是否合规。快照来自网盘，谁都能改，读进来之前每个字段都按请求的规则核对一遍。
    fn well_formed(&self) -> bool {
        let key_ok = self
            .key_id
            .as_deref()
            .is_none_or(|key_id| is_lower_hex(key_id, 32));
        let devices_ok = self.devices.len() <= MAX_DEVICES
            && self.devices.iter().all(|device| {
                valid_device(&device.device_id, &device.public_key)
                    && valid_device_name(&device.name)
                    && crate::is_bounded_text(&device.created_at, 128)
            });
        let wraps_ok = self.wraps.len() <= MAX_DEVICES * 2
            && self.wraps.iter().all(|(id, wrap)| {
                is_lower_hex(id, 32) && is_lower_hex(&wrap.key_id, 32) && valid_wrap(&wrap.wrap)
            });
        let items_ok = self.items.len() <= MAX_ITEMS
            && self.items.iter().all(|item| {
                is_lower_hex(&item.id, 64)
                    && valid_ciphertext(&item.ciphertext)
                    && is_lower_hex(&item.key_id, 32)
                    && is_lower_hex(&item.dedup, 64)
                    && crate::is_bounded_text(&item.updated_at, 128)
            });
        key_ok
            && devices_ok
            && wraps_ok
            && items_ok
            && RETENTION_DAYS.contains(&self.retention_days)
    }
}

// ---- 事件 ----

/// 一个事件改变了什么：参考中继的一个写请求的字段，上传多带新条目的编号；`seal` 关闭当前的 epoch。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Change {
    Register {
        device_id: String,
        public_key: String,
        name: String,
    },
    Reject {
        device_id: String,
    },
    Rotate {
        previous_key_id: Option<String>,
        key_id: String,
        wraps: Vec<WrapEntry>,
        revoke: Vec<String>,
    },
    Approve {
        key_id: String,
        device_id: String,
        wrap: String,
    },
    Add {
        id: String,
        ciphertext: String,
        key_id: String,
        dedup: String,
    },
    Clear,
    Delete {
        id: String,
    },
    Pin {
        id: String,
        pinned: bool,
    },
    Retention {
        days: i64,
    },
    Settings {
        enabled: bool,
    },
    Seal {
        next_epoch: u64,
    },
}

impl Change {
    fn from_request(request: &RelayRequest, item_id: &str) -> Option<Self> {
        Some(match request.clone() {
            RelayRequest::List | RelayRequest::Keys { .. } => return None,
            RelayRequest::Add {
                ciphertext,
                key_id,
                dedup,
            } => Self::Add {
                id: item_id.to_owned(),
                ciphertext,
                key_id,
                dedup,
            },
            RelayRequest::Clear => Self::Clear,
            RelayRequest::Rotate {
                previous_key_id,
                key_id,
                wraps,
                revoke,
            } => Self::Rotate {
                previous_key_id,
                key_id,
                wraps,
                revoke,
            },
            RelayRequest::Approve {
                key_id,
                device_id,
                wrap,
            } => Self::Approve {
                key_id,
                device_id,
                wrap,
            },
            RelayRequest::Register {
                device_id,
                public_key,
                name,
            } => Self::Register {
                device_id,
                public_key,
                name,
            },
            RelayRequest::Reject { device_id } => Self::Reject { device_id },
            RelayRequest::SetEnabled { enabled } => Self::Settings { enabled },
            RelayRequest::SetRetention { days } => Self::Retention { days },
            RelayRequest::Delete { id } => Self::Delete { id },
            RelayRequest::Pin { id, pinned } => Self::Pin { id, pinned },
        })
    }

    /// 对应的写请求和新条目的编号；`seal` 没有。
    fn request(&self) -> Option<(RelayRequest, &str)> {
        let request = match self {
            Self::Seal { .. } => return None,
            Self::Add {
                id,
                ciphertext,
                key_id,
                dedup,
            } => {
                return Some((
                    RelayRequest::Add {
                        ciphertext: ciphertext.clone(),
                        key_id: key_id.clone(),
                        dedup: dedup.clone(),
                    },
                    id.as_str(),
                ))
            }
            Self::Clear => RelayRequest::Clear,
            Self::Rotate {
                previous_key_id,
                key_id,
                wraps,
                revoke,
            } => RelayRequest::Rotate {
                previous_key_id: previous_key_id.clone(),
                key_id: key_id.clone(),
                wraps: wraps.clone(),
                revoke: revoke.clone(),
            },
            Self::Approve {
                key_id,
                device_id,
                wrap,
            } => RelayRequest::Approve {
                key_id: key_id.clone(),
                device_id: device_id.clone(),
                wrap: wrap.clone(),
            },
            Self::Register {
                device_id,
                public_key,
                name,
            } => RelayRequest::Register {
                device_id: device_id.clone(),
                public_key: public_key.clone(),
                name: name.clone(),
            },
            Self::Reject { device_id } => RelayRequest::Reject {
                device_id: device_id.clone(),
            },
            Self::Settings { enabled } => RelayRequest::SetEnabled { enabled: *enabled },
            Self::Retention { days } => RelayRequest::SetRetention { days: *days },
            Self::Delete { id } => RelayRequest::Delete { id: id.clone() },
            Self::Pin { id, pinned } => RelayRequest::Pin {
                id: id.clone(),
                pinned: *pinned,
            },
        };
        Some((request, ""))
    }

    /// 写完以后要不要马上压缩：清空、换钥和关闭开关之后，旧 epoch 里的密文和封装都已经用不上了。
    fn compacts(&self) -> bool {
        matches!(
            self,
            Self::Clear | Self::Rotate { .. } | Self::Settings { enabled: false }
        )
    }
}

/// 一个事件文件的内容：序号、上一个事件的哈希、随机数（同样的请求写出的文件也各不相同）、写入时间和改变。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub prev: String,
    pub nonce: String,
    pub at: i64,
    #[serde(flatten)]
    pub change: Change,
}

/// 写一个事件时由调用方给的随机部分和时间，核心本身不取随机数也不读时钟，测试因此是确定的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// 本机现在的 Unix 秒数。
    pub now: i64,
    /// 16 字节随机数的十六进制。
    pub nonce: String,
    /// 上传时新条目的编号，32 字节随机数的十六进制。
    pub item_id: String,
}

impl Draft {
    /// 从系统随机数源取随机部分。
    pub fn random(now: i64) -> Result<Self, super::clipboard_e2e::RandomUnavailable> {
        let mut nonce = [0u8; 16];
        let mut id = [0u8; 32];
        getrandom::fill(&mut nonce).map_err(|_| super::clipboard_e2e::RandomUnavailable)?;
        getrandom::fill(&mut id).map_err(|_| super::clipboard_e2e::RandomUnavailable)?;
        Ok(Self {
            now,
            nonce: hex::encode(nonce),
            item_id: hex::encode(id),
        })
    }
}

/// 日志读不下去的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogError {
    /// 事件接不上本机记得的上一个事件：日志被改写、删除过，或者网盘的独占创建不可靠。
    Chain,
    /// 快照格式不对、版本不认识或者内容不合规。
    Malformed,
    /// 这个 epoch 已经封口，后面不再接事件。
    Sealed,
}

/// 一个请求在日志上的处理结果：直接答（读取、出错、不改变状态的写入），或者要先抢到下一个序号写入 `bytes`，写成功了再答 `response`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Reply(Response),
    Append { bytes: Vec<u8>, response: Response },
}

/// 一台设备读到的日志位置和由它得出的状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub epoch: u64,
    /// 最后一个已经读到的事件序号；序号跨 epoch 连续。
    pub seq: u64,
    /// 最后一个事件文件的 SHA-256（十六进制），下一个事件的 `prev` 必须等于它。
    pub head: String,
    /// 这个 epoch 的快照之后有几个事件。
    pub events: u32,
    /// 最后一个事件是 `seal`：这个 epoch 不再接事件，下一个 epoch 从它开始。
    pub sealed: bool,
    /// 上一次写入要求压缩（清空、换钥、关闭开关）。
    pub compact: bool,
    pub state: LogState,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    format: String,
    version: u32,
    epoch: u64,
    seq: u64,
    head: String,
    state: LogState,
}

impl Ledger {
    /// 第一个 epoch 开始时的位置：没有事件，空间是新的。
    pub fn genesis() -> Self {
        Self {
            epoch: 0,
            seq: 0,
            head: GENESIS_HEAD.to_owned(),
            events: 0,
            sealed: false,
            compact: false,
            state: LogState::default(),
        }
    }

    /// 这个 epoch 开始时的快照内容。只在 epoch 刚开始（`events == 0`）时有意义；同一个日志得出的字节完全相同，几台设备同时写也只是写出同一份。
    pub fn snapshot(&self) -> Vec<u8> {
        serde_json::to_vec(&Snapshot {
            format: FORMAT.to_owned(),
            version: FORMAT_VERSION,
            epoch: self.epoch,
            seq: self.seq,
            head: self.head.clone(),
            state: self.state.clone(),
        })
        .unwrap_or_default()
    }

    /// 读一份快照。`epoch` 是它所在的目录编号，与内容不一致时不认。
    pub fn from_snapshot(epoch: u64, bytes: &[u8]) -> Result<Self, LogError> {
        let snapshot: Snapshot = serde_json::from_slice(bytes).map_err(|_| LogError::Malformed)?;
        if snapshot.format != FORMAT
            || snapshot.version != FORMAT_VERSION
            || snapshot.epoch != epoch
            || !is_lower_hex(&snapshot.head, 64)
            || !snapshot.state.well_formed()
        {
            return Err(LogError::Malformed);
        }
        Ok(Self {
            epoch,
            seq: snapshot.seq,
            head: snapshot.head,
            events: 0,
            sealed: false,
            compact: false,
            state: snapshot.state,
        })
    }

    /// 接上下一个事件，`bytes` 是 `e-<seq+1>.json` 的内容。`prev` 接不上本机记得的链头、或者 epoch 已封口时报错，位置不变。内容不合规（解析不了、序号与文件名不符、请求不合规、在当时的状态上会被拒绝）的事件照样占这个序号、接上链，但不改变状态：所有设备都这样跳过它，日志不会因为一个坏文件卡住。
    pub fn apply(&mut self, bytes: &[u8]) -> Result<(), LogError> {
        if self.sealed {
            return Err(LogError::Sealed);
        }
        #[derive(Deserialize)]
        struct Position {
            prev: String,
        }
        if let Ok(position) = serde_json::from_slice::<Position>(bytes) {
            if position.prev != self.head {
                return Err(LogError::Chain);
            }
        }
        let seq = self.seq + 1;
        self.seq = seq;
        self.head = hex::encode(Sha256::digest(bytes));
        self.events = self.events.saturating_add(1);
        let Ok(event) = serde_json::from_slice::<Event>(bytes) else {
            return Ok(());
        };
        if event.seq != seq {
            return Ok(());
        }
        match &event.change {
            Change::Seal { next_epoch } => {
                if *next_epoch == self.epoch + 1 {
                    self.state.prune(event.at);
                    self.sealed = true;
                }
            }
            change => {
                let Some((request, item_id)) = change.request() else {
                    return Ok(());
                };
                if request.check().is_err()
                    || (matches!(change, Change::Add { .. }) && !is_lower_hex(item_id, 64))
                {
                    return Ok(());
                }
                let mut next = self.state.clone();
                if next.write(&request, event.at, seq, item_id).is_ok() {
                    self.state = next;
                    if change.compacts() {
                        self.compact = true;
                    }
                }
            }
        }
        Ok(())
    }

    fn event_bytes(&self, change: Change, draft: &Draft) -> Vec<u8> {
        serde_json::to_vec(&Event {
            seq: self.seq + 1,
            prev: self.head.clone(),
            nonce: draft.nonce.clone(),
            at: draft.now,
            change,
        })
        .unwrap_or_default()
    }

    /// 在现在的位置上处理一个请求。读取在本机状态上直接答；写入在副本上试一遍，被拒绝或者不改变状态时直接答，否则交回要抢写的下一个事件和写成功后的响应。
    pub fn plan(&self, method: &str, path: &str, body: &[u8], draft: &Draft) -> Plan {
        let request = match parse_request(method, path, body) {
            Ok(request) => request,
            Err(response) => return Plan::Reply(response),
        };
        if let Some(response) = self.state.read(&request, draft.now) {
            return Plan::Reply(response);
        }
        let mut next = self.state.clone();
        let response = match next.write(&request, draft.now, self.seq + 1, &draft.item_id) {
            Ok(response) => response,
            Err(response) => return Plan::Reply(response),
        };
        if next == self.state {
            return Plan::Reply(response);
        }
        let Some(change) = Change::from_request(&request, &draft.item_id) else {
            return Plan::Reply(response);
        };
        Plan::Append {
            bytes: self.event_bytes(change, draft),
            response,
        }
    }

    /// 封口事件：抢下一个序号，关闭这个 epoch。
    pub fn seal(&self, draft: &Draft) -> Vec<u8> {
        self.event_bytes(
            Change::Seal {
                next_epoch: self.epoch + 1,
            },
            draft,
        )
    }

    /// 封口之后，下一个 epoch 开始时的位置；它的 [`Ledger::snapshot`] 就是新目录里的 `snapshot.json`。
    pub fn next_epoch(&self) -> Option<Self> {
        self.sealed.then(|| Self {
            epoch: self.epoch + 1,
            seq: self.seq,
            head: self.head.clone(),
            events: 0,
            sealed: false,
            compact: false,
            state: self.state.clone(),
        })
    }

    /// 要不要压缩：事件攒得太多、有条目过了保留期，或者上一次写入是清空、换钥、关闭开关。
    pub fn compaction_due(&self, now: i64) -> bool {
        !self.sealed
            && self.events > 0
            && (self.events > COMPACT_AFTER_EVENTS
                || self.compact
                || self.state.has_expired_items(now))
    }
}

#[cfg(test)]
#[path = "clipboard_log_tests.rs"]
mod tests;
