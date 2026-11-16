use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::stream::LearningSyncReadError;

pub const MAX_LEARNING_SYNC_TIMESTAMP: u64 = 1 << 40;

/// v1 只传调权、删除、置顶和固定位置，不带用户词、整句联想或纠错记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LearningSyncRecord {
    Weight {
        dictionary: String,
        key: String,
        value: String,
        weight: u64,
        display: String,
        updated_at: u64,
    },
    Delete {
        dictionary: String,
        key: String,
        value: String,
        updated_at: u64,
    },
    Pin {
        context: String,
        value: String,
        updated_at: u64,
    },
    Position {
        context: String,
        key: String,
        value: String,
        position: u8,
        updated_at: u64,
    },
}

// 变体顺序同时规定明文流的顺序：操作混排、置顶、固定位置。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Identity {
    Operation(String, String, String),
    Pin(String),
    Position(String, String, String),
}

fn bounded(text: &str, maximum: usize, allow_empty: bool) -> bool {
    (allow_empty || !text.is_empty()) && text.len() <= maximum && !text.contains('\0')
}

impl LearningSyncRecord {
    pub fn updated_at(&self) -> u64 {
        match self {
            Self::Weight { updated_at, .. }
            | Self::Delete { updated_at, .. }
            | Self::Pin { updated_at, .. }
            | Self::Position { updated_at, .. } => *updated_at,
        }
    }

    pub(super) fn valid(&self) -> bool {
        let key = |text: &str| bounded(text, 512, false);
        let value = |text: &str| bounded(text, 2048, false);
        let context = |text: &str| bounded(text, 1024, false);
        if !(1..=MAX_LEARNING_SYNC_TIMESTAMP).contains(&self.updated_at()) {
            return false;
        }
        match self {
            Self::Weight {
                dictionary,
                key: code,
                value: word,
                weight,
                display,
                ..
            } => {
                bounded(dictionary, 512, false)
                    && key(code)
                    && value(word)
                    && (1..=100_000_000).contains(weight)
                    && bounded(display, 2048, true)
            }
            Self::Delete {
                dictionary,
                key: code,
                value: word,
                ..
            } => bounded(dictionary, 512, false) && key(code) && value(word),
            Self::Pin {
                context: code,
                value: word,
                ..
            } => context(code) && value(word),
            Self::Position {
                context: code,
                key: entry,
                value: word,
                position,
                ..
            } => context(code) && key(entry) && value(word) && (1..=5).contains(position),
        }
    }

    pub(super) fn supported(&self) -> bool {
        match self {
            Self::Weight { dictionary, .. } | Self::Delete { dictionary, .. } => {
                matches!(
                    dictionary.as_str(),
                    "pinyin" | "wubi" | "wubi98" | "quick" | "english"
                )
            }
            _ => true,
        }
    }

    fn identity(&self) -> Identity {
        match self {
            Self::Weight {
                dictionary,
                key,
                value,
                ..
            }
            | Self::Delete {
                dictionary,
                key,
                value,
                ..
            } => Identity::Operation(dictionary.clone(), key.clone(), value.clone()),
            Self::Pin { context, .. } => Identity::Pin(context.clone()),
            Self::Position {
                context,
                key,
                value,
                ..
            } => Identity::Position(context.clone(), key.clone(), value.clone()),
        }
    }

    /// 只比较相同身份的记录；平局规则让任意设备顺序和重复传输都收敛。
    fn priority(&self, other: &Self) -> Ordering {
        self.updated_at()
            .cmp(&other.updated_at())
            .then_with(|| match (self, other) {
                (Self::Delete { .. }, Self::Weight { .. }) => Ordering::Greater,
                (Self::Weight { .. }, Self::Delete { .. }) => Ordering::Less,
                (
                    Self::Weight {
                        weight: a,
                        display: ad,
                        ..
                    },
                    Self::Weight {
                        weight: b,
                        display: bd,
                        ..
                    },
                ) => (a, ad).cmp(&(b, bd)),
                (Self::Pin { value: a, .. }, Self::Pin { value: b, .. }) => a.cmp(b),
                (Self::Position { position: a, .. }, Self::Position { position: b, .. }) => {
                    b.cmp(a)
                }
                _ => Ordering::Equal,
            })
    }
}

/// 保留全部固定位置候选；槽位冲突的输家必须参与后续合并与导出。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LearningSyncState {
    records: BTreeMap<Identity, LearningSyncRecord>,
}

impl LearningSyncState {
    /// 无效记录不改变状态；不认识的词库应由读流端计入跳过数量。
    pub fn insert(&mut self, record: LearningSyncRecord) -> Result<(), LearningSyncReadError> {
        if !record.valid() || !record.supported() {
            return Err(LearningSyncReadError::Invalid);
        }
        let identity = record.identity();
        if self
            .records
            .get(&identity)
            .is_none_or(|local| record.priority(local).is_gt())
        {
            self.records.insert(identity, record);
        }
        Ok(())
    }

    /// 可交换、可结合且幂等；不按显示槽位删掉候选。
    pub fn combine(&mut self, other: &Self) {
        for (identity, record) in &other.records {
            if self
                .records
                .get(identity)
                .is_none_or(|local| record.priority(local).is_gt())
            {
                self.records.insert(identity.clone(), record.clone());
            }
        }
    }

    pub fn records(&self) -> impl ExactSizeIterator<Item = &LearningSyncRecord> {
        self.records.values()
    }

    pub fn observed_max(&self) -> u64 {
        self.records
            .values()
            .map(LearningSyncRecord::updated_at)
            .max()
            .unwrap_or(0)
    }

    /// 重置屏障只过滤远端输入；调用方保留本机状态并在独占维护边界内应用结果。
    pub fn after_reset_floor(&self, reset_floor: u64) -> Self {
        Self {
            records: self
                .records
                .iter()
                .filter(|(_, record)| record.updated_at() > reset_floor)
                .map(|(identity, record)| (identity.clone(), record.clone()))
                .collect(),
        }
    }

    /// 同一槽位取较新候选，平局取 `(key, value)` 字节序较小者；只生成显示视图。
    pub fn position_projection(&self) -> Vec<&LearningSyncRecord> {
        let mut slots = BTreeMap::new();
        for record in self.records.values() {
            let LearningSyncRecord::Position {
                context,
                key,
                value,
                position,
                updated_at,
            } = record
            else {
                continue;
            };
            let slot = (context, position);
            let candidate = (*updated_at, std::cmp::Reverse((key, value)));
            match slots.get(&slot) {
                Some((priority, _)) if *priority >= candidate => {}
                _ => {
                    slots.insert(slot, (candidate, record));
                }
            }
        }
        slots.into_values().map(|(_, record)| record).collect()
    }
}
