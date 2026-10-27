//! 网盘上的文件格式和按序号重放的日志位置（[`Ledger`]）。格式是契约，见 `docs/dictionary-webdav-sync.md` 的「存储布局」。
//!
//! 每个事件是一个不可变文件，序号就是文件名，谁先独占创建出下一个序号谁的写入算数；事件之间用哈希串成链，本机记着链头，网盘上的历史被删改过时接不上。事件攒多了以后用一个封口事件关闭当前 epoch，下一个 epoch 从一份加密的检查点开始。

use super::clock::Hlc;
use super::crypto::{DataKey, Purpose, NONCE_BYTES};
use super::entry::{WordKey, WordKind};
use super::replica::{DeviceInfo, Op, Record, Replica};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// 格式名和版本，写在 `format.json`、每个事件和每份检查点里。
pub const FORMAT: &str = "msime-dictionary";
pub const FORMAT_VERSION: u32 = 1;
/// 第一个 epoch 的检查点之前没有事件，链头是 64 个零。
pub const GENESIS_HEAD: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// 一个事件里最多的操作数。
pub const MAX_OPS_PER_EVENT: usize = 1000;
/// 一个事件的明文（操作列表）上限。最长的词条约 4.6 KB，所以一个事件至少装得下 50 条。
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
/// 一个事件文件的上限：明文加上加密和 base64 的开销。
pub const MAX_EVENT_BYTES: u64 = 512 * 1024;
/// 一份检查点文件的上限。10 万个词条加 10 万个墓碑约 20 MB。
pub const MAX_CHECKPOINT_BYTES: u64 = 32 * 1024 * 1024;
/// 检查点里最多的词条（含墓碑）。本机只在不到 10 万个词条时才收新词（见 `MAX_LIVE_WORDS`），几台设备同时加词会略超一些，这里留出余量。
pub const MAX_CHECKPOINT_ENTRIES: usize = 300_000;
/// 一个 epoch 里攒到这么多事件或这么多字节就封口。
pub const COMPACT_AFTER_EVENTS: u32 = 64;
pub const COMPACT_AFTER_BYTES: u64 = 8 * 1024 * 1024;

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn is_device_id(value: &str) -> bool {
    crate::is_lower_hex(value, 32)
}

/// `format.json` 的内容。
pub fn format_bytes() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({ "format": FORMAT, "version": FORMAT_VERSION }))
        .unwrap_or_default()
}

/// `format.json` 是不是这一版能读写的格式。
pub fn format_supported(bytes: &[u8]) -> bool {
    #[derive(Deserialize)]
    struct Format {
        format: String,
        version: u32,
    }
    serde_json::from_slice::<Format>(bytes)
        .is_ok_and(|format| format.format == FORMAT && format.version == FORMAT_VERSION)
}

// ---- 事件 ----

/// 事件文件的明文部分。`ops` 事件的操作在 `box` 里加密，头部的其余字段是 aad。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EventFile {
    format: String,
    version: u32,
    seq: u64,
    prev: String,
    at: i64,
    #[serde(flatten)]
    body: EventBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum EventBody {
    Ops {
        key_id: String,
        #[serde(rename = "box")]
        sealed: String,
    },
    Seal {
        next_epoch: u64,
        /// 新版本的客户端把日志迁走之前写的封口带着新格式的版本号；这一版读到它就停下，不再读写。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        upgrade: Option<u32>,
    },
}

fn event_aad(seq: u64, prev: &str, at: i64, key_id: &str) -> Vec<u8> {
    format!("{FORMAT}-v{FORMAT_VERSION}-event\0{seq}\0{prev}\0{at}\0{key_id}").into_bytes()
}

/// 一个事件加密的内容：写入的设备、它确认到的位置、操作，以及要移除的设备。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EventPayload {
    device: String,
    ack: u64,
    /// 每条是 `[类别, 编码, 词, 时间戳, 是否存在]`。
    ops: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    retire: Vec<String>,
}

fn op_value(op: &Op) -> Value {
    serde_json::json!([
        op.key.kind().name(),
        op.key.code(),
        op.key.word(),
        op.hlc.0,
        op.live
    ])
}

fn parse_op(value: &Value) -> Option<Op> {
    let [kind, code, word, hlc, live] = value.as_array()?.as_slice() else {
        return None;
    };
    Some(Op {
        key: WordKey::canonical(
            WordKind::from_name(kind.as_str()?)?,
            code.as_str()?,
            word.as_str()?,
        )?,
        hlc: Hlc::parse(hlc.as_u64()?)?,
        live: live.as_bool()?,
    })
}

/// 要写的一个事件里装什么。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Draft {
    pub ops: Vec<Op>,
    pub ack: u64,
    pub retire: Vec<String>,
}

impl Draft {
    /// 明文的字节数，用来按 [`MAX_PAYLOAD_BYTES`] 分批。
    pub fn op_bytes(op: &Op) -> usize {
        op.key.encoded_len() + 24
    }
}

/// 读到一个事件之后，调用方要知道的：谁写的、带了哪些合规的操作（推进本机时钟、核对待发送的操作）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Received {
    pub device: Option<String>,
    pub ops: Vec<Op>,
}

/// 日志读不下去的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogError {
    /// 事件接不上本机记得的链头：历史被改写、删过，或者网盘的独占创建不可靠。
    Chain,
    /// 这个 epoch 已经封口，后面不再接事件。
    Sealed,
    /// 本机没有这个 epoch 的密钥，读不了它的事件（还没获准，或者密钥换过了）。位置不变。
    MissingKey,
    /// 检查点格式不对、版本不认识、与它所在的位置对不上，或者内容不合规。
    Malformed,
}

// ---- 检查点 ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointFile {
    format: String,
    version: u32,
    epoch: u64,
    seq: u64,
    head: String,
    generation: u64,
    lineage: String,
    key_id: String,
    #[serde(rename = "box")]
    sealed: String,
}

impl CheckpointFile {
    fn aad(&self) -> Vec<u8> {
        format!(
            "{FORMAT}-v{FORMAT_VERSION}-checkpoint\0{}\0{}\0{}\0{}\0{}\0{}",
            self.epoch, self.seq, self.head, self.generation, self.lineage, self.key_id
        )
        .into_bytes()
    }
}

/// 检查点的明文头部，不用密钥就能读：读到它的设备据此核对位置、认出密钥换过或者日志被重建过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointHeader {
    pub epoch: u64,
    pub seq: u64,
    pub head: String,
    pub generation: u64,
    pub lineage: String,
    pub key_id: String,
}

/// 读检查点的明文头部；格式不对时为 `None`。
pub fn checkpoint_header(bytes: &[u8]) -> Option<CheckpointHeader> {
    let file: CheckpointFile = serde_json::from_slice(bytes).ok()?;
    (file.format == FORMAT
        && file.version == FORMAT_VERSION
        && crate::is_lower_hex(&file.head, 64)
        && crate::is_lower_hex(&file.lineage, 32)
        && crate::is_lower_hex(&file.key_id, 32))
    .then_some(CheckpointHeader {
        epoch: file.epoch,
        seq: file.seq,
        head: file.head,
        generation: file.generation,
        lineage: file.lineage,
        key_id: file.key_id,
    })
}

/// 共享状态的紧凑写法，检查点的明文和本机缓存都用它。词条里的设备写成 `writers` 里的下标；顺序全部确定，同一个状态写出的字节相同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplicaFile {
    writers: Vec<String>,
    /// 每条是 `[设备编号, 确认位置, 最后出现的时间, 是否已移除]`。
    devices: Vec<(String, u64, i64, bool)>,
    /// 每条是 `[类别, 编码, 词, 时间戳, 写入者下标, 序号, 是否存在]`。
    entries: Vec<(String, String, String, u64, usize, u64, bool)>,
}

impl ReplicaFile {
    pub(crate) fn from_replica(replica: &Replica) -> Self {
        let writers: Vec<String> = replica
            .entries
            .values()
            .map(|record| record.device.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let index: BTreeMap<&str, usize> = writers
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        Self {
            devices: replica
                .devices
                .iter()
                .map(|(id, info)| (id.clone(), info.ack, info.seen, info.retired))
                .collect(),
            entries: replica
                .entries
                .iter()
                .map(|(key, record)| {
                    (
                        key.kind().name().to_owned(),
                        key.code().to_owned(),
                        key.word().to_owned(),
                        record.hlc.0,
                        index[record.device.as_str()],
                        record.seq,
                        record.live,
                    )
                })
                .collect(),
            writers,
        }
    }

    /// 逐项核对：设备编号、规范的词条、合法的时间戳、不超过 `seq` 的序号和确认位置、不重复的键。不合规的整份不认。
    pub(crate) fn into_replica(self, seq: u64) -> Option<Replica> {
        if self.entries.len() > MAX_CHECKPOINT_ENTRIES
            || self.devices.len() > super::replica::MAX_DEVICE_TABLE
            || !self.writers.iter().all(|id| is_device_id(id))
        {
            return None;
        }
        let mut replica = Replica::default();
        for (id, ack, seen, retired) in self.devices {
            if !is_device_id(&id) || ack > seq {
                return None;
            }
            let info = DeviceInfo { ack, seen, retired };
            if replica.devices.insert(id, info).is_some() {
                return None;
            }
        }
        for (kind, code, word, hlc, writer, record_seq, live) in self.entries {
            let key = WordKey::canonical(WordKind::from_name(&kind)?, &code, &word)?;
            let record = Record {
                live,
                hlc: Hlc::parse(hlc)?,
                device: self.writers.get(writer)?.clone(),
                seq: record_seq,
            };
            if record_seq > seq || replica.entries.insert(key, record).is_some() {
                return None;
            }
        }
        Some(replica)
    }
}

// ---- 日志位置 ----

/// 封口事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealInfo {
    /// 新格式的版本号：日志已经迁到这一版不认识的格式，不再读写。
    pub upgrade: Option<u32>,
}

/// 一台设备读到的日志位置和由它得出的共享状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ledger {
    pub epoch: u64,
    /// 最后读到的事件序号；序号跨 epoch 连续。
    pub seq: u64,
    /// 最后一个事件文件（或者 epoch 开始时检查点所接的那个事件）的 SHA-256。
    pub head: String,
    /// 这个 epoch 的检查点之后读到的事件数和字节数，决定什么时候封口。
    pub events: u32,
    pub bytes: u64,
    /// 读到了封口事件：这个 epoch 不再接事件。
    pub sealed: Option<SealInfo>,
    /// 这个 epoch 的事件用哪把密钥加密，来自它的检查点。
    pub key_id: String,
    /// 密钥换过、日志重建过几次。只增不减，变小就是网盘被回退了。
    pub generation: u64,
    /// 这份共享词库的编号：第一台设备建日志时随机生成，换钥和压缩都不变；有设备选择「以本机为准重新开始」时换一个新的。
    pub lineage: String,
    pub replica: Replica,
}

impl Ledger {
    /// 第一个 epoch：没有事件，空的共享状态。
    pub fn genesis(lineage: &str, key_id: &str) -> Self {
        Self {
            epoch: 0,
            seq: 0,
            head: GENESIS_HEAD.to_owned(),
            events: 0,
            bytes: 0,
            sealed: None,
            key_id: key_id.to_owned(),
            generation: 0,
            lineage: lineage.to_owned(),
            replica: Replica::default(),
        }
    }

    pub fn header(&self) -> CheckpointHeader {
        CheckpointHeader {
            epoch: self.epoch,
            seq: self.seq,
            head: self.head.clone(),
            generation: self.generation,
            lineage: self.lineage.clone(),
            key_id: self.key_id.clone(),
        }
    }

    /// 这个 epoch 开始时的检查点，用 `key` 加密（它的编号必须是 [`Ledger::key_id`]）。只在 epoch 刚开始（`events == 0`）时有意义。明文由共享状态唯一决定。
    pub fn checkpoint(&self, key: &DataKey, nonce: [u8; NONCE_BYTES]) -> Vec<u8> {
        let plaintext =
            serde_json::to_vec(&ReplicaFile::from_replica(&self.replica)).unwrap_or_default();
        let mut file = CheckpointFile {
            format: FORMAT.to_owned(),
            version: FORMAT_VERSION,
            epoch: self.epoch,
            seq: self.seq,
            head: self.head.clone(),
            generation: self.generation,
            lineage: self.lineage.clone(),
            key_id: self.key_id.clone(),
            sealed: String::new(),
        };
        file.sealed = key.seal(Purpose::Checkpoint, &plaintext, &file.aad(), nonce);
        serde_json::to_vec(&file).unwrap_or_default()
    }

    /// 读一份检查点。`epoch` 是它所在的目录编号；`key` 必须是头部 `key_id` 那一把。目录编号对不上、解不开或者内容不合规时是 [`LogError::Malformed`]，没有给密钥时是 [`LogError::MissingKey`]。
    pub fn from_checkpoint(
        epoch: u64,
        bytes: &[u8],
        key: Option<&DataKey>,
    ) -> Result<Self, LogError> {
        let file: CheckpointFile =
            serde_json::from_slice(bytes).map_err(|_| LogError::Malformed)?;
        let header = checkpoint_header(bytes).ok_or(LogError::Malformed)?;
        if header.epoch != epoch {
            return Err(LogError::Malformed);
        }
        let key = key
            .filter(|key| key.key_id() == header.key_id)
            .ok_or(LogError::MissingKey)?;
        let plaintext = key
            .open(Purpose::Checkpoint, &file.sealed, &file.aad())
            .ok_or(LogError::Malformed)?;
        let replica = serde_json::from_slice::<ReplicaFile>(&plaintext)
            .ok()
            .and_then(|replica| replica.into_replica(header.seq))
            .ok_or(LogError::Malformed)?;
        Ok(Self {
            epoch,
            seq: header.seq,
            head: header.head,
            events: 0,
            bytes: 0,
            sealed: None,
            key_id: header.key_id,
            generation: header.generation,
            lineage: header.lineage,
            replica,
        })
    }

    /// 接上下一个事件，`bytes` 是 `e-<seq+1>.json` 的内容，`key` 是本机持有的这个 epoch 的密钥。
    ///
    /// `prev` 接不上链头时是 [`LogError::Chain`]，epoch 已封口时是 [`LogError::Sealed`]，要解密却没有密钥时是 [`LogError::MissingKey`]；这三种情况位置都不变。其余不合规的事件——解析不了、序号与文件名不符、格式版本不认识、用了别的密钥、解不开、内容不对——照样占这个序号、接上链，但不改变状态：所有持有密钥的设备都这样跳过它。一个事件里单条不合规的操作只跳过那一条。
    pub fn apply(&mut self, bytes: &[u8], key: Option<&DataKey>) -> Result<Received, LogError> {
        if self.sealed.is_some() {
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
        let event = serde_json::from_slice::<EventFile>(bytes)
            .ok()
            .filter(|event| {
                event.seq == seq && event.format == FORMAT && event.version == FORMAT_VERSION
            });
        let mut received = Received::default();
        let mut change: Option<(String, i64, EventPayload, Vec<Op>)> = None;
        let mut seal = None;
        match event.as_ref() {
            Some(EventFile {
                body:
                    EventBody::Seal {
                        next_epoch,
                        upgrade,
                    },
                ..
            }) if *next_epoch == self.epoch + 1 => {
                seal = Some(SealInfo { upgrade: *upgrade });
            }
            Some(
                event @ EventFile {
                    body: EventBody::Ops { key_id, sealed },
                    ..
                },
            ) if *key_id == self.key_id => {
                let key = key
                    .filter(|key| key.key_id() == self.key_id)
                    .ok_or(LogError::MissingKey)?;
                let aad = event_aad(seq, &event.prev, event.at, key_id);
                let payload = key
                    .open(Purpose::Event, sealed, &aad)
                    .filter(|plaintext| plaintext.len() <= MAX_PAYLOAD_BYTES)
                    .and_then(|plaintext| serde_json::from_slice::<EventPayload>(&plaintext).ok())
                    .filter(|payload| {
                        is_device_id(&payload.device)
                            && payload.ops.len() <= MAX_OPS_PER_EVENT
                            && payload.retire.len() <= super::replica::MAX_DEVICE_TABLE
                    });
                if let Some(payload) = payload {
                    let ops: Vec<Op> = payload.ops.iter().filter_map(parse_op).collect();
                    change = Some((payload.device.clone(), event.at, payload, ops));
                }
            }
            _ => {}
        }
        self.seq = seq;
        self.head = sha256_hex(bytes);
        self.events = self.events.saturating_add(1);
        self.bytes = self.bytes.saturating_add(bytes.len() as u64);
        if let Some(seal) = seal {
            self.sealed = Some(seal);
        }
        if let Some((device, at, payload, ops)) = change {
            self.replica.saw(&device, seq, at, payload.ack);
            for retired in payload.retire.iter().filter(|id| is_device_id(id)) {
                if *retired != device {
                    self.replica.retire(retired);
                }
            }
            for op in &ops {
                self.replica.merge(op, &device, seq);
            }
            received = Received {
                device: Some(device),
                ops,
            };
        }
        Ok(received)
    }

    /// 本机要写的下一个事件：接在链头后面，用这个 epoch 的密钥加密。`key` 必须是 [`Ledger::key_id`] 那一把。
    pub fn event(
        &self,
        device: &str,
        draft: &Draft,
        at: i64,
        key: &DataKey,
        nonce: [u8; NONCE_BYTES],
    ) -> Vec<u8> {
        let seq = self.seq + 1;
        let payload = EventPayload {
            device: device.to_owned(),
            ack: draft.ack.min(self.seq),
            ops: draft.ops.iter().map(op_value).collect(),
            retire: draft.retire.clone(),
        };
        let plaintext = serde_json::to_vec(&payload).unwrap_or_default();
        let sealed = key.seal(
            Purpose::Event,
            &plaintext,
            &event_aad(seq, &self.head, at, &self.key_id),
            nonce,
        );
        serde_json::to_vec(&EventFile {
            format: FORMAT.to_owned(),
            version: FORMAT_VERSION,
            seq,
            prev: self.head.clone(),
            at,
            body: EventBody::Ops {
                key_id: self.key_id.clone(),
                sealed,
            },
        })
        .unwrap_or_default()
    }

    /// 封口事件：抢下一个序号，关闭这个 epoch。不需要密钥，任何设备都写得出来。
    pub fn seal(&self, at: i64) -> Vec<u8> {
        serde_json::to_vec(&EventFile {
            format: FORMAT.to_owned(),
            version: FORMAT_VERSION,
            seq: self.seq + 1,
            prev: self.head.clone(),
            at,
            body: EventBody::Seal {
                next_epoch: self.epoch + 1,
                upgrade: None,
            },
        })
        .unwrap_or_default()
    }

    /// 封口之后下一个 epoch 开始时的位置：共享状态回收过墓碑，`key_id` 是下一个 epoch 用的密钥（换钥时与这一个不同，代数加一）。它的 [`Ledger::checkpoint`] 就是新目录里的检查点。
    pub fn next_epoch(&self, key_id: &str) -> Option<Self> {
        self.sealed.as_ref()?;
        let mut replica = self.replica.clone();
        replica.collect(self.seq);
        Some(Self {
            epoch: self.epoch + 1,
            seq: self.seq,
            head: self.head.clone(),
            events: 0,
            bytes: 0,
            sealed: None,
            key_id: key_id.to_owned(),
            generation: self.generation + u64::from(key_id != self.key_id),
            lineage: self.lineage.clone(),
            replica,
        })
    }

    /// 以本机为准重新开始：接在这个 epoch 的封口后面，换一个新的共享词库编号和新密钥，共享状态换成 `replica`。用于读不了旧 epoch 的设备重置词库同步。
    pub fn restart(&self, lineage: &str, key_id: &str, replica: Replica) -> Option<Self> {
        self.sealed.as_ref()?;
        Some(Self {
            epoch: self.epoch + 1,
            seq: self.seq,
            head: self.head.clone(),
            events: 0,
            bytes: 0,
            sealed: None,
            key_id: key_id.to_owned(),
            generation: self.generation + 1,
            lineage: lineage.to_owned(),
            replica,
        })
    }

    /// 要不要封口：事件或字节攒多了。
    pub fn compaction_due(&self) -> bool {
        self.sealed.is_none()
            && self.events > 0
            && (self.events >= COMPACT_AFTER_EVENTS || self.bytes >= COMPACT_AFTER_BYTES)
    }
}

/// 只认封口和链：不用密钥读事件，只推进位置。读不了这个 epoch（没有密钥）的设备要「重新开始」时用它走到封口为止。
pub fn skip_without_key(ledger: &mut Ledger, bytes: &[u8]) -> Result<(), LogError> {
    if ledger.sealed.is_some() {
        return Err(LogError::Sealed);
    }
    #[derive(Deserialize)]
    struct Position {
        prev: String,
    }
    if let Ok(position) = serde_json::from_slice::<Position>(bytes) {
        if position.prev != ledger.head {
            return Err(LogError::Chain);
        }
    }
    let seq = ledger.seq + 1;
    if let Ok(event) = serde_json::from_slice::<EventFile>(bytes) {
        if let EventBody::Seal {
            next_epoch,
            upgrade,
        } = event.body
        {
            if event.seq == seq
                && event.format == FORMAT
                && event.version == FORMAT_VERSION
                && next_epoch == ledger.epoch + 1
            {
                ledger.sealed = Some(SealInfo { upgrade });
            }
        }
    }
    ledger.seq = seq;
    ledger.head = sha256_hex(bytes);
    ledger.events = ledger.events.saturating_add(1);
    ledger.bytes = ledger.bytes.saturating_add(bytes.len() as u64);
    Ok(())
}

#[cfg(test)]
#[path = "log_tests.rs"]
mod tests;
