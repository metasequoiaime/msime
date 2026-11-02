//! Apple snapshot record mapping at 2b0250f4dd7012520392b310dfcc0288c3208a75.
use msime_engine::host::{DictionaryKind, SnapshotReadError};
use msime_engine::DictionaryStateRecord;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
enum Record {
    Overlay {
        kind: String,
        code: String,
        word: String,
        weight: i64,
        #[serde(default = "yes")]
        user_inserted: bool,
    },
    Position {
        context: String,
        code: String,
        word: String,
        position: i64,
    },
    Selection {
        context: String,
        code: String,
        word: String,
        count: i64,
    },
}
fn yes() -> bool {
    true
}

/// 这一版认得、能交给 Engine 的词库种类。
pub(super) const KNOWN_KINDS: [&str; 6] =
    ["pinyin", "wubi", "wubi98", "wubixsj", "quick", "english"];

/// 更新的版本加的、这一版不认识的词库种类：1 到 32 个小写字母、数字或下划线，且不在 [`KNOWN_KINDS`] 里。这种记录整条跳过，不让整份快照读不了；不是这个形状的种类仍让快照不合规。
pub(super) fn is_foreign_kind(kind: &str) -> bool {
    (1..=32).contains(&kind.len())
        && kind
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !KNOWN_KINDS.contains(&kind)
}

/// 一行快照记录转成 Engine 的记录。种类是 [`is_foreign_kind`] 的 overlay 为 `Ok(None)`，调用方跳过它读下一行。
pub(super) fn decode(bytes: &[u8]) -> Result<Option<DictionaryStateRecord>, SnapshotReadError> {
    // Parse deletion separately: it is an envelope field, mandatory for overlays.
    #[derive(Deserialize)]
    struct Envelope {
        deleted: Option<bool>,
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| SnapshotReadError)?;
    Ok(Some(match record {
        Record::Overlay {
            kind,
            code,
            word,
            weight,
            user_inserted,
        } => {
            let envelope: Envelope =
                serde_json::from_slice(bytes).map_err(|_| SnapshotReadError)?;
            let deleted = envelope.deleted.ok_or(SnapshotReadError)?;
            let kind = match kind.as_str() {
                "pinyin" => DictionaryKind::Pinyin,
                "wubi" => DictionaryKind::Wubi,
                "wubi98" => DictionaryKind::Wubi98,
                "wubixsj" => DictionaryKind::WubiXsj,
                "quick" => DictionaryKind::QuickPhrase,
                "english" => DictionaryKind::English,
                kind if is_foreign_kind(kind) => return Ok(None),
                _ => return Err(SnapshotReadError),
            };
            let display = if kind == DictionaryKind::English && !deleted {
                word.clone()
            } else {
                String::new()
            };
            DictionaryStateRecord::Entry {
                kind,
                key: code,
                value: word,
                weight,
                display,
                deleted,
                user_inserted,
            }
        }
        Record::Position {
            context,
            code,
            word,
            position,
        } => DictionaryStateRecord::Position {
            context,
            key: code,
            value: word,
            position,
        },
        Record::Selection {
            context,
            code,
            word,
            count,
        } => DictionaryStateRecord::Selection {
            context,
            key: code,
            value: word,
            count,
        },
    }))
}
