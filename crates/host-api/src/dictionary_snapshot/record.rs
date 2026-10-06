//! Apple snapshot record mapping at 2b0250f4dd7012520392b310dfcc0288c3208a75.
use msime_engine::host::{DictionaryKind, DictionaryStateRecord, SnapshotReadError};
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

pub(super) fn decode(bytes: &[u8]) -> Result<DictionaryStateRecord, SnapshotReadError> {
    // Parse deletion separately: it is an envelope field, mandatory for overlays.
    #[derive(Deserialize)]
    struct Envelope {
        deleted: Option<bool>,
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| SnapshotReadError)?;
    Ok(match record {
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
                "quick" => DictionaryKind::QuickPhrase,
                "english" => DictionaryKind::English,
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
    })
}
