//! Native-only snapshot preparation. Staged paths stay private until a future
//! activation transaction can own publication and session coordination.
use super::{response, DictionaryAccess, HostOptions, HOST_OPTIONS_DOCUMENT_LIMIT};
use msime_client_core::account::{
    AccountDictionarySnapshotRestore, AccountError, BackendAccountClient,
};
use msime_client_core::cloud::snapshot_queue::{
    local_version, local_version_digest, DictionarySnapshotQueue, SnapshotQueueError,
};
use msime_client_core::cloud::snapshot_validation::{
    has_keys as snapshot_has_keys, parse_strict_object, required_integer as snapshot_integer,
    required_text as snapshot_text, valid_timestamp as snapshot_timestamp,
};
use msime_client_core::resources::{ResourceSet, ResourceStore};
use msime_engine::host::{
    dictionary_state_revision, stage_dictionary_state, EngineOptions, Session, SnapshotReadError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ffi::{c_char, c_void, OsStr, OsString},
    io::{self, BufRead, BufReader, Write},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};

pub(crate) mod habits;
mod record;

const BUFFER_LIMIT: usize = 65536;
const REQUEST_LIMIT: usize = HOST_OPTIONS_DOCUMENT_LIMIT;
const HANDLE_LIMIT: usize = 8;
const ACTIVATION_RECEIPT_NAME: &str = ".msime-snapshot-activation";
const MAX_ACTIVATION_RECEIPT_BYTES: u64 = 36;
const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SNAPSHOT_LINE_BYTES: usize = 65_536;
const MAX_SNAPSHOT_RECORDS: usize = 500_000;
// The guard's own file, which stays put while everything around it is swapped.
const DICTIONARY_ACCESS_LOCK_NAME: &str = ".msime-dictionary-access.lock";
static NEXT: AtomicU64 = AtomicU64::new(1);
static PREPARED: OnceLock<Mutex<HashMap<u64, Prepared>>> = OnceLock::new();
fn registry() -> &'static Mutex<HashMap<u64, Prepared>> {
    PREPARED.get_or_init(Default::default)
}

struct Prepared {
    directory: tempfile::TempDir,
    active_options: EngineOptions,
    options: EngineOptions,
    source_version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareRequest {
    options: HostOptions,
    staging_root: String,
    expected_version: String,
    records: usize,
    #[serde(default)]
    activation_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreRequest {
    revision: i64,
    expected_sha256: String,
    access_token: String,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum SnapshotQueueAction {
    State {
        directory: String,
        options: HostOptions,
        #[serde(default)]
        acknowledge: bool,
    },
    Enqueue {
        directory: String,
        source: String,
        account_id: String,
        cloud_revision: i64,
        expected_local_version: String,
        file_sha256: String,
    },
    Cancel {
        directory: String,
        account_id: String,
    },
    Process {
        directory: String,
        staging_root: String,
        options: HostOptions,
        account_id: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotMetadata {
    cloud_revision: i64,
    sha256: String,
    file_sha256: String,
    bytes: u64,
    records: usize,
    entries: usize,
    overlays: usize,
    positions: usize,
    selections: usize,
    engine_records: usize,
}

#[derive(Default)]
struct SnapshotIdentities {
    entry_keys: HashMap<(String, String, String), i64>,
    entry_ids: HashSet<String>,
    overlays: HashMap<(String, String, String), (bool, bool, i64)>,
    positions: HashSet<(String, String, String)>,
    position_slots: HashSet<(String, i64)>,
    selections: HashSet<(String, String, String)>,
}

fn inspect_snapshot_record(
    map: &serde_json::Map<String, Value>,
    revision: i64,
    identities: &mut SnapshotIdentities,
) -> Result<usize, &'static str> {
    let kind = map
        .get("type")
        .and_then(Value::as_str)
        .ok_or("invalid snapshot document")?;
    let data = map
        .get("data")
        .and_then(Value::as_object)
        .ok_or("invalid snapshot document")?;
    let code = snapshot_text(data, "code", 512, "invalid snapshot document")?.to_owned();
    let word = snapshot_text(data, "word", 2048, "invalid snapshot document")?.to_owned();
    match kind {
        "entry" | "overlay" => {
            let outer_keys_valid = if kind == "overlay" {
                snapshot_has_keys(map, &["type", "data", "deleted"])
            } else {
                snapshot_has_keys(map, &["type", "data"])
            };
            if !outer_keys_valid {
                return Err("invalid snapshot document");
            }
            let deleted = if kind == "overlay" {
                map.get("deleted")
                    .and_then(Value::as_bool)
                    .ok_or("invalid snapshot document")?
            } else {
                false
            };
            let expected = [
                "id",
                "kind",
                "code",
                "word",
                "weight",
                "revision",
                "updated_at",
            ];
            let mut data_keys = data.keys().map(String::as_str).collect::<HashSet<_>>();
            if data_keys.remove("user_inserted") {
                let user_inserted = data
                    .get("user_inserted")
                    .and_then(Value::as_bool)
                    .ok_or("invalid snapshot document")?;
                if kind == "entry" && !user_inserted {
                    return Err("invalid snapshot document");
                }
            }
            if data_keys.len() != expected.len()
                || expected.iter().any(|key| !data_keys.contains(key))
            {
                return Err("invalid snapshot document");
            }
            let dictionary_kind = data
                .get("kind")
                .and_then(Value::as_str)
                .filter(|value| {
                    matches!(*value, "pinyin" | "wubi" | "wubi98" | "english" | "quick")
                })
                .ok_or("invalid snapshot document")?
                .to_owned();
            let id = data
                .get("id")
                .and_then(Value::as_str)
                .ok_or("invalid snapshot document")?
                .to_owned();
            if kind == "entry"
                && (id.is_empty()
                    || id.len() > 128
                    || id
                        .bytes()
                        .any(|byte| matches!(byte, 0 | b'\t' | b'\n' | b'\r')))
            {
                return Err("invalid snapshot document");
            }
            let weight = snapshot_integer(data, "weight", "invalid snapshot document")?;
            let record_revision = snapshot_integer(data, "revision", "invalid snapshot document")?;
            if !(0..=100_000_000).contains(&weight)
                || (weight == 0 && !deleted)
                || !(1..=revision).contains(&record_revision)
                || !snapshot_timestamp(
                    data.get("updated_at")
                        .and_then(Value::as_str)
                        .ok_or("invalid snapshot document")?,
                )
            {
                return Err("invalid snapshot document");
            }
            let identity = (dictionary_kind, code, word);
            if kind == "entry" {
                if identities.entry_keys.insert(identity, weight).is_some()
                    || !identities.entry_ids.insert(id)
                {
                    return Err("invalid snapshot document");
                }
            } else {
                let user_inserted = data
                    .get("user_inserted")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                if identities
                    .overlays
                    .insert(identity, (deleted, user_inserted, weight))
                    .is_some()
                {
                    return Err("invalid snapshot document");
                }
            }
            Ok(if kind == "entry" { 1 } else { 2 })
        }
        "position" | "selection" => {
            if !snapshot_has_keys(map, &["type", "data"]) {
                return Err("invalid snapshot document");
            }
            let value_key = if kind == "position" {
                "position"
            } else {
                "count"
            };
            if !snapshot_has_keys(data, &["context", "code", "word", value_key]) {
                return Err("invalid snapshot document");
            }
            let context =
                snapshot_text(data, "context", 512, "invalid snapshot document")?.to_owned();
            if context.len() + code.len() + word.len() > 2048 {
                return Err("invalid snapshot document");
            }
            let identity = (context.clone(), code, word);
            if kind == "position" {
                let position = snapshot_integer(data, "position", "invalid snapshot document")?;
                if !(1..=5).contains(&position)
                    || !identities.positions.insert(identity)
                    || !identities.position_slots.insert((context, position))
                {
                    return Err("invalid snapshot document");
                }
                Ok(3)
            } else {
                let count = snapshot_integer(data, "count", "invalid snapshot document")?;
                if !(0..=10).contains(&count) || !identities.selections.insert(identity) {
                    return Err("invalid snapshot document");
                }
                Ok(4)
            }
        }
        _ => Err("invalid snapshot document"),
    }
}

fn reject_symlinked_snapshot_path(path: &Path) -> Result<(), &'static str> {
    msime_path_trust::reject_symlinked_components(path).map_err(|_| "snapshot file unavailable")
}

fn open_snapshot_file(path: &Path) -> io::Result<std::fs::File> {
    crate::bounded_file::open_private(path)
}

/// Validate the complete NDJSON envelope before a host calls the expensive Engine staging path.
/// Header/footer order, exact body checksum, category order and record bounds are all part of the
/// cloud format. Engine records receive their deeper scheme-specific validation during prepare.
pub(crate) fn inspect_snapshot(path: &Path) -> Result<SnapshotMetadata, &'static str> {
    reject_symlinked_snapshot_path(path)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "snapshot file unavailable")?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > MAX_SNAPSHOT_BYTES
    {
        return Err("invalid snapshot file");
    }
    let file = open_snapshot_file(path).map_err(|_| "snapshot file unavailable")?;
    let mut reader = BufReader::with_capacity(MAX_SNAPSHOT_LINE_BYTES, file);
    let mut line = Vec::with_capacity(MAX_SNAPSHOT_LINE_BYTES);
    let mut body_digest = Sha256::new();
    let mut file_digest = Sha256::new();
    let mut total_bytes = 0u64;
    let mut records = 0usize;
    let mut counts = [0usize; 4];
    let mut category = 0usize;
    let mut revision = None;
    let mut checksum = None;
    let mut identities = SnapshotIdentities::default();
    loop {
        line.clear();
        let complete = loop {
            let chunk = reader.fill_buf().map_err(|_| "snapshot file unavailable")?;
            if chunk.is_empty() {
                break false;
            }
            if let Some(index) = chunk.iter().position(|byte| *byte == b'\n') {
                if line.len() + index + 1 > MAX_SNAPSHOT_LINE_BYTES {
                    return Err("invalid snapshot document");
                }
                line.extend_from_slice(&chunk[..=index]);
                reader.consume(index + 1);
                break true;
            }
            if line.len() + chunk.len() >= MAX_SNAPSHOT_LINE_BYTES {
                return Err("invalid snapshot document");
            }
            line.extend_from_slice(chunk);
            let length = chunk.len();
            reader.consume(length);
        };
        let has_newline = complete;
        file_digest.update(&line);
        if has_newline {
            line.pop();
            if line.ends_with(b"\r") {
                return Err("invalid snapshot document");
            }
        } else if line.is_empty() {
            break;
        }
        total_bytes = total_bytes
            .checked_add(line.len() as u64 + u64::from(has_newline))
            .ok_or("invalid snapshot document")?;
        if total_bytes > MAX_SNAPSHOT_BYTES || line.is_empty() {
            return Err("invalid snapshot document");
        }
        let map = parse_strict_object(&line).map_err(|_| "invalid snapshot document")?;
        let kind = map
            .get("type")
            .and_then(Value::as_str)
            .ok_or("invalid snapshot document")?;
        match kind {
            "header" => {
                if records != 0
                    || revision.is_some()
                    || !snapshot_has_keys(&map, &["type", "format", "version", "revision"])
                    || map.get("format").and_then(Value::as_str)
                        != Some("msime-dictionary-snapshot")
                    || map.get("version").and_then(Value::as_i64) != Some(1)
                {
                    return Err("invalid snapshot document");
                }
                revision = Some(
                    map.get("revision")
                        .and_then(Value::as_i64)
                        .filter(|value| *value >= 0)
                        .ok_or("invalid snapshot document")?,
                );
                records = 1;
                body_digest.update(&line);
                body_digest.update(b"\n");
            }
            "entry" | "overlay" | "position" | "selection" => {
                let snapshot_revision = revision.ok_or("invalid snapshot document")?;
                if checksum.is_some() {
                    return Err("invalid snapshot document");
                }
                let next = inspect_snapshot_record(&map, snapshot_revision, &mut identities)?;
                if next < category {
                    return Err("invalid snapshot document");
                }
                category = next;
                counts[next - 1] = counts[next - 1]
                    .checked_add(1)
                    .ok_or("invalid snapshot document")?;
                records = records.checked_add(1).ok_or("invalid snapshot document")?;
                if records > MAX_SNAPSHOT_RECORDS || counts[0] > 100_000 {
                    return Err("invalid snapshot document");
                }
                body_digest.update(&line);
                body_digest.update(b"\n");
            }
            "footer" => {
                if revision.is_none()
                    || checksum.is_some()
                    || !snapshot_has_keys(&map, &["type", "records", "sha256"])
                {
                    return Err("invalid snapshot document");
                }
                let expected_records = map
                    .get("records")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or("invalid snapshot document")?;
                let expected_sha = map
                    .get("sha256")
                    .and_then(Value::as_str)
                    .filter(|value| crate::valid_sha256(value))
                    .ok_or("invalid snapshot document")?;
                let actual = hex::encode(body_digest.clone().finalize());
                if expected_records != records || expected_sha != actual {
                    return Err("invalid snapshot document");
                }
                checksum = Some(expected_sha.to_owned());
            }
            _ => return Err("invalid snapshot document"),
        }
        if !has_newline {
            break;
        }
    }
    let cloud_revision = revision.ok_or("invalid snapshot document")?;
    let sha256 = checksum.ok_or("invalid snapshot document")?;
    for (identity, entry_weight) in &identities.entry_keys {
        let Some((deleted, user_inserted, weight)) = identities.overlays.get(identity) else {
            return Err("invalid snapshot document");
        };
        if *deleted || !*user_inserted || *weight != *entry_weight {
            return Err("invalid snapshot document");
        }
    }
    for (identity, (deleted, user_inserted, weight)) in &identities.overlays {
        if !*deleted && *user_inserted {
            let Some(entry_weight) = identities.entry_keys.get(identity) else {
                return Err("invalid snapshot document");
            };
            if *entry_weight != *weight {
                return Err("invalid snapshot document");
            }
        }
    }
    Ok(SnapshotMetadata {
        cloud_revision,
        sha256,
        file_sha256: hex::encode(file_digest.finalize()),
        bytes: total_bytes,
        records,
        entries: counts[0],
        overlays: counts[1],
        positions: counts[2],
        selections: counts[3],
        engine_records: counts[1] + counts[2] + counts[3],
    })
}

/// 把本机用户词库写成与 `GET /v1/users/me/dictionary/snapshot` 相同的 NDJSON（`msime-dictionary-snapshot` 第 1 版：header、每个词一条 `entry` 和一条同权重的 `overlay`、footer 带正文 SHA-256），写完再用 [`inspect_snapshot`] 按云端格式校验一遍，返回同样的元数据。
///
/// 这是离线导出：不需要登录，修订号固定为 1。调用方已经持有词库的会话访问权。
///
/// `include_learning` 为真时（本地备份用，云同步不用）在用户的词之后再写输入记录：Engine 日志里的学习调权和删除记录写成 `overlay`（学习调权 `user_inserted:false`，删除记录 `deleted:true`），固定位置写成 `position`，选词计数写成 `selection`，都是这个格式第 1 版本来就有的记录，旧版本恢复时照样认得。快照格式装不下的行（编码或词含换行、制表符，位置不在 1 到 5，总记录数超出上限）跳过并计入 `learning_skipped`，选词计数截到 0 到 10。返回值多出 `learning`（写进去的输入记录条数）和 `learning_skipped`。日志整体读不出来时只导出词，`learning` 为 0，原因在 `learning_error`。为假时输出与加这个参数之前逐字节相同。
pub(crate) fn export_local_snapshot(
    options: &EngineOptions,
    destination: &Path,
    include_learning: bool,
) -> Result<Value, &'static str> {
    use msime_engine::host::DictionaryKind;
    const REVISION: i64 = 1;
    const CHUNK: usize = 1000;
    destination
        .parent()
        .filter(|_| destination.is_absolute() && destination.file_name().is_some())
        .ok_or("invalid snapshot destination")?;
    if destination.is_dir() {
        return Err("invalid snapshot destination");
    }
    let now = time::OffsetDateTime::now_utc();
    let updated_at = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    // Rows the cloud snapshot format cannot carry: a quick phrase may hold a line break or a tab, which `required_text` refuses. They are left out and counted so the host can tell the user, instead of the whole export failing.
    let mut skipped = 0usize;
    let mut offset = 0usize;
    loop {
        let page = msime_engine::host::dictionary_entries(options, offset, CHUNK)
            .map_err(|_| "dictionary read rejected")?;
        let count = page.entries.len();
        for entry in page.entries {
            let kind = match entry.kind {
                DictionaryKind::Pinyin => "pinyin",
                DictionaryKind::Wubi => "wubi",
                DictionaryKind::Wubi98 => "wubi98",
                DictionaryKind::QuickPhrase => "quick",
                DictionaryKind::English => "english",
                _ => continue,
            };
            if !snapshot_safe(&entry.key) || !snapshot_safe(&entry.value) {
                skipped += 1;
                continue;
            }
            if entry.key.is_empty()
                || entry.key.len() > 512
                || entry.value.is_empty()
                || entry.value.len() > 2048
                || !seen.insert((kind, entry.key.clone(), entry.value.clone()))
            {
                continue;
            }
            rows.push((
                kind,
                entry.key,
                entry.value,
                entry.weight.clamp(1, 100_000_000),
            ));
        }
        offset = offset.saturating_add(count);
        if !page.has_more || count == 0 || rows.len() >= 100_000 {
            break;
        }
    }
    rows.truncate(100_000);
    // 输入记录读不出来（日志里有不是 UTF-8 的行、未知的词库种类或超长的行，`stream_dictionary_state` 会整体拒绝）时不让整份备份失败：照样导出词，`learning_error` 告诉宿主这份里没有输入记录。
    let mut learning_error = None;
    let learning = if include_learning {
        Some(
            learning_records(
                options,
                REVISION,
                &updated_at,
                MAX_SNAPSHOT_RECORDS - 1 - rows.len() * 2,
            )
            .unwrap_or_else(|error| {
                learning_error = Some(error);
                LearningRecords::default()
            }),
        )
    } else {
        None
    };
    let mut body = Vec::new();
    let mut push = |line: Value| {
        body.extend_from_slice(line.to_string().as_bytes());
        body.push(b'\n');
    };
    push(json!({
        "type": "header",
        "format": "msime-dictionary-snapshot",
        "version": 1,
        "revision": REVISION,
    }));
    let id = snapshot_record_id;
    for (kind, code, word, weight) in &rows {
        push(json!({"type": "entry", "data": {
            "id": id(kind, code, word),
            "kind": kind,
            "code": code,
            "word": word,
            "weight": weight,
            "revision": REVISION,
            "updated_at": updated_at,
        }}));
    }
    for (kind, code, word, weight) in &rows {
        push(json!({"type": "overlay", "deleted": false, "data": {
            "id": id(kind, code, word),
            "kind": kind,
            "code": code,
            "word": word,
            "weight": weight,
            "revision": REVISION,
            "updated_at": updated_at,
            "user_inserted": true,
        }}));
    }
    let mut records = 1 + rows.len() * 2;
    let learning_counts = learning
        .as_ref()
        .map(|learning| (learning.records(), learning.skipped));
    if let Some(learning) = learning {
        records += learning.records();
        // 类别顺序是格式的一部分：overlay 在 position 之前，position 在 selection 之前。
        for line in learning
            .overlays
            .into_iter()
            .chain(learning.positions)
            .chain(learning.selections)
        {
            push(line);
        }
    }
    let checksum = hex::encode(Sha256::digest(&body));
    body.extend_from_slice(
        json!({"type": "footer", "records": records, "sha256": checksum})
            .to_string()
            .as_bytes(),
    );
    body.push(b'\n');
    // 自检用的临时文件建在目标文件旁边，不用系统临时目录：Android 9 的应用进程没有可写的系统临时目录（`std::env::temp_dir` 落到应用写不了的 /data/local/tmp），在那里建文件会让每一次导出都失败。
    let directory = destination.parent().ok_or("invalid snapshot destination")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|_| "snapshot file unavailable")?;
    temporary
        .write_all(&body)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| "snapshot file unavailable")?;
    let metadata = inspect_snapshot(temporary.path())?;
    publish_snapshot(destination, &body)?;
    let mut value = serde_json::to_value(metadata).map_err(|_| "snapshot file unavailable")?;
    value["path"] = json!(destination.to_string_lossy());
    value["skipped"] = json!(skipped);
    if let Some((learning, learning_skipped)) = learning_counts {
        value["learning"] = json!(learning);
        value["learning_skipped"] = json!(learning_skipped);
    }
    if let Some(error) = learning_error {
        value["learning_error"] = json!(error);
    }
    Ok(value)
}

/// 快照里一个词的 `id`：种类、编码和词用制表符连起来的 SHA-256 前 16 字节。
fn snapshot_record_id(kind: &str, code: &str, word: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update(b"\t");
    digest.update(code.as_bytes());
    digest.update(b"\t");
    digest.update(word.as_bytes());
    hex::encode(&digest.finalize()[..16])
}

/// [`export_local_snapshot`] 写进快照的输入记录，按类别分开放，写的时候按格式要求的顺序拼起来。
#[derive(Default)]
struct LearningRecords {
    overlays: Vec<Value>,
    positions: Vec<Value>,
    selections: Vec<Value>,
    skipped: usize,
}

impl LearningRecords {
    fn records(&self) -> usize {
        self.overlays.len() + self.positions.len() + self.selections.len()
    }
}

/// 读出 Engine 日志里的输入记录（`stream_dictionary_state`），转成快照记录。用户自己的词（`user_inserted` 的 upsert）已经作为 `entry`/`overlay` 写过，这里跳过。最多收 `budget` 条，其余计入 `skipped`。
fn learning_records(
    options: &EngineOptions,
    revision: i64,
    updated_at: &str,
    budget: usize,
) -> Result<LearningRecords, &'static str> {
    use msime_engine::host::{DictionaryKind, DictionaryStateRecord};
    // 与 `inspect_snapshot_record` 对编码、词和上下文的要求相同，写出去的记录不会让自检失败。
    let fits = |text: &str, maximum: usize| {
        !text.is_empty() && text.len() <= maximum && snapshot_safe(text)
    };
    let fits_context = |context: &str, code: &str, word: &str| {
        fits(context, 512)
            && fits(code, 512)
            && fits(word, 2048)
            && context.len() + code.len() + word.len() <= 2048
    };
    let mut learning = LearningRecords::default();
    msime_engine::host::stream_dictionary_state(options, &mut |record| {
        if matches!(
            record,
            DictionaryStateRecord::Entry {
                user_inserted: true,
                deleted: false,
                ..
            }
        ) {
            return true;
        }
        if learning.records() >= budget {
            learning.skipped += 1;
            return true;
        }
        match record {
            DictionaryStateRecord::Entry {
                kind,
                key,
                value,
                weight,
                deleted,
                user_inserted,
                ..
            } => {
                let kind = match kind {
                    DictionaryKind::Pinyin => "pinyin",
                    DictionaryKind::Wubi => "wubi",
                    DictionaryKind::Wubi98 => "wubi98",
                    DictionaryKind::QuickPhrase => "quick",
                    DictionaryKind::English => "english",
                    // 快照格式只认上面这几种，Engine 以后加的种类装不下。
                    _ => {
                        learning.skipped += 1;
                        return true;
                    }
                };
                if !fits(key, 512) || !fits(value, 2048) {
                    learning.skipped += 1;
                    return true;
                }
                // 删除记录的权重没有意义，格式只允许它为 0 到上限；其余的权重至少为 1。
                let weight = if *deleted {
                    (*weight).clamp(0, 100_000_000)
                } else {
                    (*weight).clamp(1, 100_000_000)
                };
                learning
                    .overlays
                    .push(json!({"type": "overlay", "deleted": deleted, "data": {
                        "id": snapshot_record_id(kind, key, value),
                        "kind": kind,
                        "code": key,
                        "word": value,
                        "weight": weight,
                        "revision": revision,
                        "updated_at": updated_at,
                        "user_inserted": user_inserted,
                    }}));
            }
            DictionaryStateRecord::Position {
                context,
                key,
                value,
                position,
            } => {
                if !fits_context(context, key, value) || !(1..=5).contains(position) {
                    learning.skipped += 1;
                    return true;
                }
                learning.positions.push(json!({"type": "position", "data": {
                    "context": context,
                    "code": key,
                    "word": value,
                    "position": position,
                }}));
            }
            DictionaryStateRecord::Selection {
                context,
                key,
                value,
                count,
            } => {
                if !fits_context(context, key, value) {
                    learning.skipped += 1;
                    return true;
                }
                learning
                    .selections
                    .push(json!({"type": "selection", "data": {
                        "context": context,
                        "code": key,
                        "word": value,
                        "count": (*count).clamp(0, 10),
                    }}));
            }
        }
        true
    })
    .map_err(|_| "dictionary read rejected")?;
    Ok(learning)
}

fn publish_snapshot(destination: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    msime_client_core::file_lock::replace_private_file(destination, bytes)
        .map_err(|_| "snapshot file unavailable")
}

/// The same byte test `snapshot_validation::required_text` applies to a code or word, so an exported row is never one `inspect_snapshot` refuses.
fn snapshot_safe(text: &str) -> bool {
    !text
        .bytes()
        .any(|byte| matches!(byte, 0 | b'\t' | b'\n' | b'\r'))
}

/// 待合并的输入记录在 `preferences_directory` 下的文件名，见 [`queue_learning_merge`]。
const PENDING_LEARNING_NAME: &str = "pending-learning-merge.ndjson";
/// 键盘认领待合并的文件后改成的名字，见 [`merge_pending_learning`]。
const CLAIMED_LEARNING_NAME: &str = "pending-learning-merge.claimed.ndjson";
/// 认领的那份已经连续失败了几次，一个十进制数。
const LEARNING_ATTEMPTS_NAME: &str = "pending-learning-merge.attempts";
/// 连续失败这么多次就放弃认领的那份。
const MAX_LEARNING_MERGE_ATTEMPTS: u32 = 3;

/// 一份快照里的记录是不是输入记录：学习调权（`user_inserted:false` 的 overlay）、删除记录（`deleted:true` 的 overlay）、固定位置和选词计数。用户自己的词（`entry` 和与它配对的 overlay）不是。
fn is_learning_record(map: &serde_json::Map<String, Value>) -> bool {
    match map.get("type").and_then(Value::as_str) {
        Some("position" | "selection") => true,
        Some("overlay") => {
            map.get("deleted").and_then(Value::as_bool) == Some(true)
                || map
                    .get("data")
                    .and_then(|data| data.get("user_inserted"))
                    .and_then(Value::as_bool)
                    == Some(false)
        }
        _ => false,
    }
}

/// 把 `source`（本地备份里的词库快照）中的输入记录挑出来，另存成一份只有这些记录的快照，放在 `preferences` 下等键盘合并：Android 上改工作词库要独占维护权，只有键盘没有会话时才拿得到，所以设置页不直接合并，而是由键盘收起后的空闲处理（[`merge_pending_learning`]）合并进去。还没被认领的一份会被替换。
///
/// `source` 先按云端格式完整校验；挑出来的文件写完再校验一遍。返回 `{queued, learning}`：快照里没有输入记录（旧版本导出的备份）时 `queued` 为假，什么也不写。
pub(crate) fn queue_learning_merge(
    preferences: &Path,
    source: &Path,
) -> Result<Value, &'static str> {
    if !preferences.is_absolute() || !source.is_absolute() {
        return Err("invalid snapshot path");
    }
    let metadata = inspect_snapshot(source)?;
    let mut records = SnapshotFileRecords::open(source)?;
    let mut body = Vec::new();
    body.extend_from_slice(
        json!({
            "type": "header",
            "format": "msime-dictionary-snapshot",
            "version": 1,
            "revision": metadata.cloud_revision,
        })
        .to_string()
        .as_bytes(),
    );
    body.push(b'\n');
    let mut learning = 0usize;
    while records
        .read_line()
        .map_err(|_| "snapshot file unavailable")?
    {
        let map = parse_strict_object(&records.line).map_err(|_| "invalid snapshot document")?;
        if is_learning_record(&map) {
            body.extend_from_slice(&records.line);
            body.push(b'\n');
            learning += 1;
        }
    }
    if learning == 0 {
        return Ok(json!({"queued": false, "learning": 0}));
    }
    let checksum = hex::encode(Sha256::digest(&body));
    body.extend_from_slice(
        json!({"type": "footer", "records": 1 + learning, "sha256": checksum})
            .to_string()
            .as_bytes(),
    );
    body.push(b'\n');
    let pending = preferences.join(PENDING_LEARNING_NAME);
    msime_client_core::file_lock::replace_private_file(&pending, &body)
        .map_err(|_| "snapshot file unavailable")?;
    if let Err(error) = inspect_snapshot(&pending) {
        let _ = std::fs::remove_file(&pending);
        return Err(error);
    }
    Ok(json!({"queued": true, "learning": learning}))
}

/// 有待合并的输入记录（[`queue_learning_merge`]）时把它合并进本机的日志和词库：本机已有的保留本机，选词计数取大（`msime_engine::host::merge_dictionary_state`）；接着用同样的认领方式合并待合并的输入习惯（[`habits::queue_habits_merge`]）。都没有待合并的文件时返回 `{merged:false}`，合并了输入记录返回 `{merged:true, entries, positions, selections, kept, skipped}`；处理过输入习惯时再多一个 `habits` 字段，是 `{merged:true, written, kept, trimmed}`，或失败时的 `{merged:false, error}`。输入记录合并失败时报它的错，输入习惯照样尝试。Android 键盘收起、会话销毁后在空闲时调用（与整份快照激活同一时机，排在它之后），不在建会话前的个人词库同步里做，免得一份大备份拖慢恢复后第一次弹出键盘。
///
/// 先拿独占维护权，拿不到（还有会话开着）时什么也不动，下次再试。拿到后把待合并的文件改名认领（[`CLAIMED_LEARNING_NAME`]），只合并、只删认领的那一份：合并期间设置页又排了一份新的，新的那份写在原来的名字下，不会被这边删掉，下次空闲时再合并。上次没合并完的认领文件还在时先合并它，新排的等下一次。
///
/// 文件格式不对（不是合法快照）时直接删掉。别的失败（读文件出错、写库出错）保留文件下次再试，但连续失败 [`MAX_LEARNING_MERGE_ATTEMPTS`] 次后放弃并删掉，报 `learning merge abandoned`，不会每次空闲都把整份合并再跑一遍再回滚。合并成功后删掉文件，删不掉也无妨：再合并一次时本机已有的都保留，结果不变。
pub(crate) fn merge_pending_learning(
    options: &EngineOptions,
    preferences: &Path,
) -> Result<Value, &'static str> {
    let learning = merge_pending(options, preferences, &LEARNING_FILES, |claimed| {
        let metadata = inspect_snapshot(claimed)?;
        let stream = SnapshotFileRecords::open(claimed)?;
        let merged = msime_engine::host::merge_dictionary_state(
            options,
            metadata.engine_records.max(1),
            stream,
        )
        .map_err(|_| "learning merge rejected")?;
        Ok(json!({
            "merged": true,
            "entries": merged.entries,
            "positions": merged.positions,
            "selections": merged.selections,
            "kept": merged.kept,
            "skipped": merged.skipped,
        }))
    });
    let habits = merge_pending(options, preferences, &habits::HABITS_FILES, |claimed| {
        habits::merge_claimed_habits(options, claimed)
    });
    let habits = match habits {
        Ok(value) if value.get("merged") == Some(&Value::Bool(false)) => return learning,
        Ok(value) => value,
        Err(error) => json!({"merged": false, "error": error}),
    };
    learning.map(|mut value| {
        value["habits"] = habits;
        value
    })
}

/// 一类待合并文件的名字：设置页写下的、键盘认领后改成的、记连续失败次数的，以及放弃时报的错和哪些错误说明文件本身不对（直接删掉，不重试）。
pub(crate) struct PendingFiles {
    pending: &'static str,
    claimed: &'static str,
    attempts: &'static str,
    abandoned: &'static str,
    invalid: &'static [&'static str],
}

const LEARNING_FILES: PendingFiles = PendingFiles {
    pending: PENDING_LEARNING_NAME,
    claimed: CLAIMED_LEARNING_NAME,
    attempts: LEARNING_ATTEMPTS_NAME,
    abandoned: "learning merge abandoned",
    invalid: &["invalid snapshot document", "invalid snapshot file"],
};

/// [`merge_pending_learning`] 的认领与重试：没有文件时返回 `{merged:false}`；有的话拿独占维护权、认领、交给 `merge`，按结果删掉或记下失败次数。
fn merge_pending(
    options: &EngineOptions,
    preferences: &Path,
    files: &PendingFiles,
    merge: impl FnOnce(&Path) -> Result<Value, &'static str>,
) -> Result<Value, &'static str> {
    let pending = preferences.join(files.pending);
    let claimed = preferences.join(files.claimed);
    let attempts = preferences.join(files.attempts);
    let present = |path: &Path| match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("snapshot file unavailable"),
    };
    if !present(&claimed)? && !present(&pending)? {
        return Ok(json!({"merged": false}));
    }
    let _access = DictionaryAccess::try_maintenance(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "dictionary access unavailable")?
    .ok_or("dictionary maintenance busy")?;
    if !present(&claimed)? {
        std::fs::rename(&pending, &claimed).map_err(|_| "snapshot file unavailable")?;
        // 新认领的一份，重试次数从头算。
        remove_if_present(&attempts)?;
    }
    let discard = || {
        let _ = std::fs::remove_file(&claimed);
        let _ = std::fs::remove_file(&attempts);
    };
    match merge(&claimed) {
        Ok(merged) => {
            discard();
            Ok(merged)
        }
        Err(error) if files.invalid.contains(&error) => {
            discard();
            Err(error)
        }
        Err(error) => {
            let failures = std::fs::read_to_string(&attempts)
                .ok()
                .and_then(|text| text.trim().parse::<u32>().ok())
                .unwrap_or(0)
                .saturating_add(1);
            if failures >= MAX_LEARNING_MERGE_ATTEMPTS {
                discard();
                return Err(files.abandoned);
            }
            msime_client_core::file_lock::replace_private_file(
                &attempts,
                failures.to_string().as_bytes(),
            )
            .map_err(|_| "snapshot file unavailable")?;
            Err(error)
        }
    }
}

fn remove_if_present(path: &Path) -> Result<(), &'static str> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err("snapshot file unavailable"),
        _ => Ok(()),
    }
}

/// 本机日志里输入记录的条数（学习调权、删除记录、固定位置和选词计数，不含用户自己的词）`count`，以及输入习惯的行数 `habits`（[`habits`]），只读。本地备份恢复时用它判断本机是不是还什么都没学过。
pub(crate) fn learning_count(options: &EngineOptions) -> Result<Value, &'static str> {
    use msime_engine::host::DictionaryStateRecord;
    let mut count = 0usize;
    msime_engine::host::stream_dictionary_state(options, &mut |record| {
        if !matches!(
            record,
            DictionaryStateRecord::Entry {
                user_inserted: true,
                deleted: false,
                ..
            }
        ) {
            count += 1;
        }
        true
    })
    .map_err(|_| "dictionary read rejected")?;
    // 整份激活会连同输入习惯一起换掉，所以调用方也要知道本机有没有输入习惯；读不出来时为 null，调用方按「不确定」处理。
    let habits = msime_engine::host::count_learning_habits(options).ok();
    Ok(json!({"count": count, "habits": habits}))
}

fn restore_snapshot_with(
    request: RestoreRequest,
    path: &Path,
    upload: impl FnOnce(&Path, i64, &str) -> Result<AccountDictionarySnapshotRestore, AccountError>,
) -> Result<Value, String> {
    if request.revision < 0 || !msime_client_core::is_lower_hex(&request.expected_sha256, 64) {
        return Err("account_invalid".to_owned());
    }
    let metadata = inspect_snapshot(path).map_err(|_| "account_invalid".to_owned())?;
    if metadata.file_sha256 != request.expected_sha256 {
        return Err("account_invalid".to_owned());
    }
    let restored = upload(path, request.revision, &request.access_token)
        .map_err(|error| error.code().to_owned())?;
    serde_json::to_value(restored).map_err(|_| "account_unavailable".to_owned())
}

/// Called synchronously: positive UTF-8 JSON length, zero only at verified EOF,
/// negative for cancellation/corruption. It must not throw, unwind or retain buffer.
pub type SnapshotNext = unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> isize;

fn parse_options(bytes: &[u8]) -> Result<EngineOptions, &'static str> {
    if bytes.len() > REQUEST_LIMIT {
        return Err("invalid snapshot options");
    }
    let options = serde_json::from_slice(bytes)
        .ok()
        .and_then(HostOptions::from_document)
        .ok_or("invalid snapshot options")?;
    validate_options(options)
}
fn validate_options(options: HostOptions) -> Result<EngineOptions, &'static str> {
    if options.api_version != 1 || options.preferences.validate().is_err() {
        return Err("invalid snapshot options");
    }
    Ok(options.into_engine_options())
}

fn version(options: &EngineOptions) -> Result<String, &'static str> {
    let _access = DictionaryAccess::try_session(
        Path::new(&options.user_data),
        Path::new(&options.dictionaries),
    )
    .map_err(|_| "snapshot access unavailable")?
    .ok_or("snapshot access busy")?;
    let mut hash = Sha256::new();
    hash.update(b"msime-host-dictionary-version-v1");
    for path in [
        &options.resources,
        &options.user_data,
        &options.cache,
        &options.dictionaries,
    ] {
        if !Path::new(path).is_absolute() {
            return Err("invalid snapshot path");
        }
        let canonical = Path::new(path)
            .canonicalize()
            .map_err(|_| "snapshot path unavailable")?;
        let text = canonical.to_str().ok_or("invalid snapshot path")?;
        hash.update((text.len() as u64).to_be_bytes());
        hash.update(text.as_bytes());
    }
    hash.update(dictionary_state_revision(options).map_err(|_| "snapshot revision unavailable")?);
    Ok(hex::encode(hash.finalize()))
}

fn activation_receipt(options: &EngineOptions) -> Result<Option<String>, &'static str> {
    let path = Path::new(&options.user_data).join(ACTIVATION_RECEIPT_NAME);
    reject_symlinked_snapshot_path(&path).map_err(|_| "snapshot activation receipt unavailable")?;
    let file = match open_snapshot_file(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("snapshot activation receipt unavailable"),
    };
    let value = crate::bounded_file::read(file, MAX_ACTIVATION_RECEIPT_BYTES).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            "invalid snapshot activation receipt"
        } else {
            "snapshot activation receipt unavailable"
        }
    })?;
    let value = std::str::from_utf8(&value).map_err(|_| "invalid snapshot activation receipt")?;
    if !crate::valid_uuid_string(value) {
        return Err("invalid snapshot activation receipt");
    }
    Ok(Some(value.to_owned()))
}

fn write_activation_receipt(
    options: &EngineOptions,
    activation_id: &str,
) -> Result<(), &'static str> {
    let directory = Path::new(&options.user_data);
    let path = directory.join(ACTIVATION_RECEIPT_NAME);
    write_activation_receipt_at(&path, activation_id)
}

fn write_activation_receipt_at(path: &Path, activation_id: &str) -> Result<(), &'static str> {
    msime_client_core::file_lock::replace_private_file(path, activation_id.as_bytes())
        .map_err(|_| "snapshot activation receipt unavailable")
}

fn prepare(
    request: PrepareRequest,
    specification: &ResourceSet,
    on_demand: &[&str],
    stream: impl Iterator<Item = Result<msime_engine::host::DictionaryStateRecord, SnapshotReadError>>
        + 'static,
) -> Result<Prepared, &'static str> {
    if request.records > 500_000 || request.expected_version.len() != 64 {
        return Err("invalid snapshot bounds");
    }
    if request
        .activation_id
        .as_deref()
        .is_some_and(|value| !crate::valid_uuid_string(value))
    {
        return Err("invalid snapshot activation id");
    }
    let options = validate_options(request.options)?;
    let current = version(&options)?;
    if current != request.expected_version {
        return Err("snapshot source changed");
    }
    let root = Path::new(&request.staging_root);
    if !root.is_absolute() {
        return Err("invalid snapshot staging root");
    }
    let root = root
        .canonicalize()
        .map_err(|_| "snapshot staging root unavailable")?;
    for path in [
        &options.resources,
        &options.user_data,
        &options.cache,
        &options.dictionaries,
    ] {
        let path = Path::new(path)
            .canonicalize()
            .map_err(|_| "snapshot path unavailable")?;
        if root.starts_with(&path) || path.starts_with(&root) {
            return Err("snapshot staging overlaps active paths");
        }
    }
    // 与 `prepare_host_configuration` 相同的发货规则；内容标识仍按完整清单计算。
    let shipped =
        crate::shipped_specification(specification, Path::new(&options.resources), on_demand);
    ResourceStore::new(&options.resources)
        .verify(Path::new(&options.resources), &shipped)
        .map_err(|_| "snapshot resources rejected")?;
    let content_id = specification
        .generation()
        .map_err(|_| "snapshot resources rejected")?;
    let directory = tempfile::Builder::new()
        .prefix("snapshot-")
        .tempdir_in(root)
        .map_err(|_| "snapshot staging unavailable")?;
    let generation = directory.path().join("generation");
    let mut count = 0;
    let expected = request.records;
    let mut source = stream;
    let checked = std::iter::from_fn(move || match source.next() {
        Some(Ok(record)) if count < expected => {
            count += 1;
            Some(Ok(record))
        }
        Some(_) => Some(Err(SnapshotReadError)),
        None if count == expected => None,
        None => Some(Err(SnapshotReadError)),
    });
    let staged = stage_dictionary_state(
        &options,
        generation.to_str().ok_or("invalid snapshot path")?,
        &content_id,
        expected.max(1),
        checked,
    )
    .map_err(|_| "snapshot preparation rejected")?;
    if let Some(activation_id) = request.activation_id.as_deref() {
        write_activation_receipt(&staged, activation_id)?;
    }
    // Learning may continue during expensive preparation. Reject a changed preview.
    if version(&options)? != current {
        return Err("snapshot source changed");
    }
    Ok(Prepared {
        directory,
        active_options: options,
        options: staged,
        source_version: current,
    })
}

struct ActivationRoot {
    path: std::path::PathBuf,
    name: OsString,
    directory: msime_client_core::file_lock::PrivateDirectory,
    parent: msime_client_core::file_lock::PrivateDirectory,
}

struct ActivationBackup {
    name: OsString,
    directory: msime_client_core::file_lock::PrivateDirectory,
    parent: msime_client_core::file_lock::PrivateDirectory,
}

#[derive(Clone, Copy)]
enum ActivationRootSlot {
    Active(usize),
    Staged(usize),
    Backup(usize),
}

struct MovedActivationEntry {
    from: ActivationRootSlot,
    to: ActivationRootSlot,
    name: OsString,
}

fn bind_activation_root(
    path: &Path,
    directory: msime_client_core::file_lock::PrivateDirectory,
) -> Result<ActivationRoot, &'static str> {
    let name = path
        .file_name()
        .ok_or("invalid snapshot activation path")?
        .to_os_string();
    let parent =
        msime_client_core::file_lock::open_private_directory_at(&directory, OsStr::new(".."))
            .map_err(|_| "snapshot activation unavailable")?;
    Ok(ActivationRoot {
        path: path.to_path_buf(),
        name,
        directory,
        parent,
    })
}

fn prepare_snapshot_backup_at(
    parent: &msime_client_core::file_lock::PrivateDirectory,
    name: &OsStr,
) -> std::io::Result<msime_client_core::file_lock::PrivateDirectory> {
    match msime_client_core::file_lock::open_private_directory_at(parent, name) {
        Ok(directory) => {
            if !msime_client_core::file_lock::read_private_directory(&directory)?.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "snapshot backup is not empty",
                ));
            }
            Ok(directory)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            msime_client_core::file_lock::create_private_directory_at(parent, name)?;
            msime_client_core::file_lock::open_private_directory_at(parent, name)
        }
        Err(error) => Err(error),
    }
}

fn activation_slot_directory<'a>(
    slot: ActivationRootSlot,
    active: &'a [ActivationRoot],
    staged: &'a [ActivationRoot],
    backups: &'a [ActivationBackup],
) -> &'a msime_client_core::file_lock::PrivateDirectory {
    match slot {
        ActivationRootSlot::Active(index) => &active[index].directory,
        ActivationRootSlot::Staged(index) => &staged[index].directory,
        ActivationRootSlot::Backup(index) => &backups[index].directory,
    }
}

fn activation_entry_leads_to_nested_root(
    root: &ActivationRoot,
    entry_name: &OsStr,
    all: &[ActivationRoot],
) -> bool {
    let entry = root.path.join(entry_name);
    all.iter()
        .any(|other| other.path != root.path && other.path.starts_with(&entry))
}

fn activate(handle: u64, expected: &str) -> Result<Value, &'static str> {
    activate_with_hook(handle, expected, || {})
}

fn activate_with_hook<F>(handle: u64, expected: &str, before_swap: F) -> Result<Value, &'static str>
where
    F: FnOnce(),
{
    let mut entries = registry()
        .lock()
        .map_err(|_| "snapshot registry unavailable")?;
    let prepared = entries.get(&handle).ok_or("unknown snapshot handle")?;
    if prepared.source_version != expected {
        return Err("snapshot source changed");
    }
    let active = &prepared.active_options;
    let staged = &prepared.options;
    let _access = DictionaryAccess::try_maintenance(
        Path::new(&active.user_data),
        Path::new(&active.dictionaries),
    )
    .map_err(|_| "snapshot access unavailable")?
    .ok_or("snapshot access busy")?;
    if version_without_access(active)? != expected {
        return Err("snapshot source changed");
    }
    // Construct the replacement engine while the current generation is still
    // untouched.  A malformed or otherwise unusable generation must not make
    // the active dictionaries unavailable after publication.
    let replacement_session = Session::new(staged).map_err(|_| "snapshot engine unavailable")?;
    // Windows cannot rename SQLite files while the probe keeps them open.
    drop(replacement_session);
    // The same step `reset_learned_data` takes before it replaces files in place: write the queued personal context into the journal that is about to become the backup, then close every cached journal, personal-context and local-mode connection, so nothing in this process keeps reading or writing the files being moved out (Windows would also refuse to move them).
    msime_engine::close_cached_databases();
    let suffix = format!(".msime-snapshot-old-{handle}");
    let pairs = [
        (&active.user_data, &staged.user_data),
        (&active.cache, &staged.cache),
        (&active.dictionaries, &staged.dictionaries),
    ];
    let active_directories = [
        _access
            .directory(Path::new(&active.user_data))
            .map_err(|_| "snapshot activation unavailable")?,
        msime_client_core::file_lock::open_private_directory(&active.cache)
            .map_err(|_| "snapshot activation unavailable")?,
        _access
            .directory(Path::new(&active.dictionaries))
            .map_err(|_| "snapshot activation unavailable")?,
    ];
    let staged_directories = [
        msime_client_core::file_lock::open_private_directory(&staged.user_data)
            .map_err(|_| "snapshot activation unavailable")?,
        msime_client_core::file_lock::open_private_directory(&staged.cache)
            .map_err(|_| "snapshot activation unavailable")?,
        msime_client_core::file_lock::open_private_directory(&staged.dictionaries)
            .map_err(|_| "snapshot activation unavailable")?,
    ];
    let active_roots = active_directories
        .into_iter()
        .zip(pairs.iter().map(|(current, _)| Path::new(current.as_str())))
        .map(|(directory, path)| bind_activation_root(path, directory))
        .collect::<Result<Vec<_>, _>>()?;
    let staged_roots = staged_directories
        .into_iter()
        .zip(
            pairs
                .iter()
                .map(|(_, replacement)| Path::new(replacement.as_str())),
        )
        .map(|(directory, path)| bind_activation_root(path, directory))
        .collect::<Result<Vec<_>, _>>()?;
    let mut backups: Vec<ActivationBackup> = Vec::with_capacity(active_roots.len());
    for root in &active_roots {
        let mut name = root.name.clone();
        name.push(&suffix);
        let parent = root
            .parent
            .try_clone()
            .map_err(|_| "snapshot activation unavailable")?;
        let directory = match prepare_snapshot_backup_at(&parent, &name) {
            Ok(directory) => directory,
            Err(_) => {
                for backup in &backups {
                    let _ = msime_client_core::file_lock::remove_private_directory_at(
                        &backup.parent,
                        &backup.name,
                    );
                }
                return Err("snapshot activation failed");
            }
        };
        backups.push(ActivationBackup {
            name,
            directory,
            parent,
        });
    }
    before_swap();
    // Swap each root's contents rather than the root itself.
    //
    // Renaming the roots cannot work on Windows: the maintenance guard holds
    // an open handle on a lock file inside them, and Windows refuses to rename
    // a directory containing any open handle - share mode does not help. So
    // activation has never succeeded there. Moving the entries leaves the lock
    // files exactly where they are, which is also what they are documented to
    // require: they are stable coordination objects, and renaming a root moved
    // one out from under every other process using it.
    let mut moved = Vec::new();
    let rollback = |moved: &[MovedActivationEntry]| {
        // Anything opened on a moved file while the swap ran would outlive its move back.
        msime_engine::close_cached_databases();
        for entry in moved.iter().rev() {
            let from = activation_slot_directory(entry.to, &active_roots, &staged_roots, &backups);
            let to = activation_slot_directory(entry.from, &active_roots, &staged_roots, &backups);
            let _ = msime_client_core::file_lock::rename_private_entry(
                from,
                &entry.name,
                to,
                &entry.name,
            );
        }
        for backup in &backups {
            let _ = msime_client_core::file_lock::remove_private_directory_at(
                &backup.parent,
                &backup.name,
            );
        }
    };
    // An entry that leads to another root nested below this one is left alone:
    // that root does its own swap, and it holds its own lock file.
    for index in 0..active_roots.len() {
        let current = &active_roots[index];
        let replacement = &staged_roots[index];
        let backup = &backups[index];
        // Out with the old.
        let listing = match msime_client_core::file_lock::read_private_directory(&current.directory)
        {
            Ok(listing) => listing,
            Err(_) => {
                rollback(&moved);
                return Err("snapshot activation failed");
            }
        };
        for entry in listing {
            if entry == OsStr::new(DICTIONARY_ACCESS_LOCK_NAME)
                || activation_entry_leads_to_nested_root(current, &entry, &active_roots)
            {
                continue;
            }
            if msime_client_core::file_lock::rename_private_entry(
                &current.directory,
                &entry,
                &backup.directory,
                &entry,
            )
            .is_err()
            {
                rollback(&moved);
                return Err("snapshot activation failed");
            }
            moved.push(MovedActivationEntry {
                from: ActivationRootSlot::Active(index),
                to: ActivationRootSlot::Backup(index),
                name: entry,
            });
        }
        // In with the new.
        let listing =
            match msime_client_core::file_lock::read_private_directory(&replacement.directory) {
                Ok(listing) => listing,
                Err(_) => {
                    rollback(&moved);
                    return Err("snapshot activation failed");
                }
            };
        for entry in listing {
            if entry == OsStr::new(DICTIONARY_ACCESS_LOCK_NAME)
                || activation_entry_leads_to_nested_root(replacement, &entry, &staged_roots)
            {
                continue;
            }
            if msime_client_core::file_lock::rename_private_entry(
                &replacement.directory,
                &entry,
                &current.directory,
                &entry,
            )
            .is_err()
            {
                rollback(&moved);
                return Err("snapshot activation failed");
            }
            moved.push(MovedActivationEntry {
                from: ActivationRootSlot::Staged(index),
                to: ActivationRootSlot::Active(index),
                name: entry,
            });
        }
    }
    // A reader that opened a file while the swap ran holds the old one; the next access opens the restored files.
    msime_engine::close_cached_databases();
    for backup in &backups {
        let _ =
            msime_client_core::file_lock::remove_private_directory_at(&backup.parent, &backup.name);
    }
    entries.remove(&handle);
    Ok(json!({"activated": true}))
}

/// Remove a backup directory only once rollback has emptied it.
///
/// `remove_dir` refuses a directory that still has anything in it, and that refusal is the point.
/// The renames that put the original contents back are best effort - one of them failing is
/// exactly the case where the backup is the only remaining copy of the user's dictionaries, and
/// `remove_dir_all` would delete it on the way out of a failure that had already been survived.
/// Leaving the directory on disk costs some space and keeps the data.
#[cfg(test)]
fn discard_recovered_backup(backup: &Path) {
    let _ = std::fs::remove_dir(backup);
}

fn version_without_access(options: &EngineOptions) -> Result<String, &'static str> {
    let mut hash = Sha256::new();
    hash.update(b"msime-host-dictionary-version-v1");
    for path in [
        &options.resources,
        &options.user_data,
        &options.cache,
        &options.dictionaries,
    ] {
        let canonical = Path::new(path)
            .canonicalize()
            .map_err(|_| "snapshot path unavailable")?;
        let text = canonical.to_str().ok_or("invalid snapshot path")?;
        hash.update((text.len() as u64).to_be_bytes());
        hash.update(text.as_bytes());
    }
    hash.update(dictionary_state_revision(options).map_err(|_| "snapshot revision unavailable")?);
    Ok(hex::encode(hash.finalize()))
}

fn register(prepared: Prepared) -> Result<Value, &'static str> {
    let mut entries = registry()
        .lock()
        .map_err(|_| "snapshot registry unavailable")?;
    if entries.len() >= HANDLE_LIMIT {
        return Err("too many prepared snapshots");
    }
    let handle = NEXT
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map_err(|_| "snapshot handle unavailable")?;
    let output = json!({"handle": handle, "source_version": prepared.source_version});
    entries.insert(handle, prepared);
    Ok(output)
}

fn snapshot_queue_error(error: SnapshotQueueError) -> String {
    match error {
        SnapshotQueueError::Unavailable => "snapshot_unavailable",
        SnapshotQueueError::Busy => "snapshot_busy",
        SnapshotQueueError::Invalid => "snapshot_invalid",
        SnapshotQueueError::Conflict => "snapshot_conflict",
    }
    .to_owned()
}

fn snapshot_queue(directory: &str) -> Result<DictionarySnapshotQueue, String> {
    if directory.len() > 16_384 {
        return Err("snapshot_invalid".to_owned());
    }
    DictionarySnapshotQueue::new(directory).map_err(snapshot_queue_error)
}

fn durable_local_version(options: &EngineOptions) -> Result<String, &'static str> {
    let digest = version(options)?;
    let generation = activation_receipt(options)?;
    local_version(generation.as_deref(), &digest).map_err(|_| "invalid snapshot local version")
}

struct SnapshotFileRecords {
    reader: BufReader<std::fs::File>,
    line: Vec<u8>,
    failed: bool,
}

impl SnapshotFileRecords {
    fn open(path: &Path) -> Result<Self, &'static str> {
        reject_symlinked_snapshot_path(path)?;
        let file = open_snapshot_file(path).map_err(|_| "snapshot file unavailable")?;
        Ok(Self {
            reader: BufReader::with_capacity(MAX_SNAPSHOT_LINE_BYTES, file),
            line: Vec::with_capacity(MAX_SNAPSHOT_LINE_BYTES),
            failed: false,
        })
    }

    fn read_line(&mut self) -> Result<bool, SnapshotReadError> {
        self.line.clear();
        loop {
            let chunk = self.reader.fill_buf().map_err(|_| SnapshotReadError)?;
            if chunk.is_empty() {
                return Ok(!self.line.is_empty());
            }
            if let Some(index) = chunk.iter().position(|byte| *byte == b'\n') {
                if self.line.len() + index + 1 > MAX_SNAPSHOT_LINE_BYTES {
                    return Err(SnapshotReadError);
                }
                self.line.extend_from_slice(&chunk[..index]);
                self.reader.consume(index + 1);
                return (!self.line.is_empty() && !self.line.ends_with(b"\r"))
                    .then_some(true)
                    .ok_or(SnapshotReadError);
            }
            if self.line.len() + chunk.len() >= MAX_SNAPSHOT_LINE_BYTES {
                return Err(SnapshotReadError);
            }
            self.line.extend_from_slice(chunk);
            let length = chunk.len();
            self.reader.consume(length);
        }
    }
}

impl Iterator for SnapshotFileRecords {
    type Item = Result<msime_engine::host::DictionaryStateRecord, SnapshotReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        loop {
            match self.read_line() {
                Ok(false) => return None,
                Err(error) => {
                    self.failed = true;
                    return Some(Err(error));
                }
                Ok(true) => {}
            }
            let kind = match parse_strict_object(&self.line)
                .ok()
                .and_then(|map| map.get("type").and_then(Value::as_str).map(str::to_owned))
            {
                Some(kind) => kind,
                None => {
                    self.failed = true;
                    return Some(Err(SnapshotReadError));
                }
            };
            match kind.as_str() {
                "overlay" | "position" | "selection" => {
                    return Some(record::decode(&self.line));
                }
                "header" | "entry" | "footer" => continue,
                _ => {
                    self.failed = true;
                    return Some(Err(SnapshotReadError));
                }
            }
        }
    }
}

fn snapshot_queue_state(
    queue: &DictionarySnapshotQueue,
    options: HostOptions,
    acknowledge: bool,
) -> Result<Value, String> {
    let options = validate_options(options).map_err(str::to_owned)?;
    let current = durable_local_version(&options).map_err(str::to_owned)?;
    queue
        .publish_local_version(&current)
        .map_err(snapshot_queue_error)?;
    let state = if acknowledge {
        queue.take_state()
    } else {
        queue.read()
    }
    .map_err(snapshot_queue_error)?;
    serde_json::to_value(state).map_err(|_| "snapshot_unavailable".to_owned())
}

fn snapshot_queue_process(
    queue: &DictionarySnapshotQueue,
    staging_root: String,
    options: HostOptions,
    account_id: Option<String>,
) -> Result<Value, String> {
    let engine_options = validate_options(options.clone()).map_err(str::to_owned)?;
    let current = durable_local_version(&engine_options).map_err(str::to_owned)?;
    queue
        .publish_local_version(&current)
        .map_err(snapshot_queue_error)?;
    // 键盘这次读不到共享的会话文件（读取失败或未登录），不知道当前账号是谁。请求留到下一次空闲时再处理，不在未知账号下应用。
    let Some(account_id) = account_id else {
        return serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
            .map_err(|_| "snapshot_unavailable".to_owned());
    };
    let lease = match queue.acquire_worker_lease() {
        Ok(lease) => lease,
        Err(SnapshotQueueError::Busy) => {
            return serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
                .map_err(|_| "snapshot_unavailable".to_owned());
        }
        Err(error) => return Err(snapshot_queue_error(error)),
    };
    let Some(request) = queue
        .claim(&lease, &account_id)
        .map_err(snapshot_queue_error)?
    else {
        return serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
            .map_err(|_| "snapshot_unavailable".to_owned());
    };
    if request.expected_local_version != current {
        let _ = queue
            .complete(request.id, &lease, &current, false, &account_id, || {
                Err(SnapshotQueueError::Conflict)
            })
            .map_err(snapshot_queue_error)?;
        return serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
            .map_err(|_| "snapshot_unavailable".to_owned());
    }
    let path = queue.file_path(request.id).map_err(snapshot_queue_error)?;
    let metadata = match inspect_snapshot(&path) {
        Ok(metadata) if metadata.file_sha256 == request.file_sha256 => metadata,
        _ => {
            queue
                .fail(request.id, &lease)
                .map_err(snapshot_queue_error)?;
            return serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
                .map_err(|_| "snapshot_unavailable".to_owned());
        }
    };
    let raw_expected = local_version_digest(&request.expected_local_version)
        .map_err(snapshot_queue_error)?
        .to_owned();
    let stream = SnapshotFileRecords::open(&path).map_err(str::to_owned)?;
    // 与准备宿主时相同：按文档记录的版本的锁校验资源、计算代次。
    let specification = options
        .edition()
        .resource_set()
        .map_err(|_| "snapshot resources rejected".to_owned())?;
    let prepared = prepare(
        PrepareRequest {
            options,
            staging_root,
            expected_version: raw_expected.clone(),
            records: metadata.engine_records,
            activation_id: Some(request.id.to_string()),
        },
        &specification,
        crate::ON_DEMAND_ARTIFACTS,
        stream,
    );
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(_) => {
            let latest = durable_local_version(&engine_options).map_err(str::to_owned)?;
            if latest != current {
                let _ = queue
                    .complete(request.id, &lease, &latest, false, &account_id, || {
                        Err(SnapshotQueueError::Conflict)
                    })
                    .map_err(snapshot_queue_error)?;
            } else {
                queue
                    .fail(request.id, &lease)
                    .map_err(snapshot_queue_error)?;
            }
            return serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
                .map_err(|_| "snapshot_unavailable".to_owned());
        }
    };
    let registered = register(prepared).map_err(str::to_owned)?;
    let handle = registered
        .get("handle")
        .and_then(Value::as_u64)
        .ok_or_else(|| "snapshot_unavailable".to_owned())?;
    let mut consumed = false;
    let completion = queue.complete(request.id, &lease, &current, false, &account_id, || {
        activate(handle, &raw_expected).map_err(|_| SnapshotQueueError::Unavailable)?;
        consumed = true;
        durable_local_version(&engine_options).map_err(|_| SnapshotQueueError::Unavailable)
    });
    if !consumed {
        let _ = discard(handle);
    }
    completion.map_err(snapshot_queue_error)?;
    serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
        .map_err(|_| "snapshot_unavailable".to_owned())
}

fn run_snapshot_queue(action: SnapshotQueueAction) -> Result<Value, String> {
    match action {
        SnapshotQueueAction::State {
            directory,
            options,
            acknowledge,
        } => snapshot_queue_state(&snapshot_queue(&directory)?, options, acknowledge),
        SnapshotQueueAction::Enqueue {
            directory,
            source,
            account_id,
            cloud_revision,
            expected_local_version,
            file_sha256,
        } => {
            let queue = snapshot_queue(&directory)?;
            queue
                .enqueue(
                    Path::new(&source),
                    &account_id,
                    cloud_revision,
                    &expected_local_version,
                    &file_sha256,
                )
                .map_err(snapshot_queue_error)?;
            serde_json::to_value(queue.read().map_err(snapshot_queue_error)?)
                .map_err(|_| "snapshot_unavailable".to_owned())
        }
        SnapshotQueueAction::Cancel {
            directory,
            account_id,
        } => {
            let queue = snapshot_queue(&directory)?;
            queue.cancel(&account_id).map_err(snapshot_queue_error)?;
            serde_json::to_value(queue.take_state().map_err(snapshot_queue_error)?)
                .map_err(|_| "snapshot_unavailable".to_owned())
        }
        SnapshotQueueAction::Process {
            directory,
            staging_root,
            options,
            account_id,
        } => snapshot_queue_process(
            &snapshot_queue(&directory)?,
            staging_root,
            options,
            account_id,
        ),
    }
}

/// Persist, inspect, process or query the one crash-safe native snapshot queue.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes.
#[no_mangle]
pub unsafe extern "C" fn msime_client_snapshot_queue(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > REQUEST_LIMIT {
            return Err("snapshot_invalid".to_owned());
        }
        let action: SnapshotQueueAction =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, length) })
                .map_err(|_| "snapshot_invalid".to_owned())?;
        run_snapshot_queue(action)
    })
}

/// Inspect one host-private snapshot file without returning its contents.
/// # Safety
/// `path` points to `length` readable UTF-8 bytes naming an absolute file path.
#[no_mangle]
pub unsafe extern "C" fn msime_client_snapshot_inspect(
    path: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if path.is_null() || length == 0 || length > 16_384 {
            return Err("invalid snapshot path".into());
        }
        let text = std::str::from_utf8(unsafe { std::slice::from_raw_parts(path, length) })
            .map_err(|_| "invalid snapshot path")?;
        let path = Path::new(text);
        if !path.is_absolute() {
            return Err("invalid snapshot path".into());
        }
        inspect_snapshot(path)
            .and_then(|metadata| serde_json::to_value(metadata).map_err(|_| "snapshot unavailable"))
            .map_err(Into::into)
    })
}

/// Reinspect and upload one host-private snapshot file without buffering it in the host bridge.
/// # Safety
/// `request` points to `request_length` readable JSON bytes and `path` points to
/// `path_length` readable UTF-8 bytes naming an absolute private file path.
#[no_mangle]
pub unsafe extern "C" fn msime_client_snapshot_restore(
    request: *const u8,
    request_length: usize,
    path: *const u8,
    path_length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null()
            || request_length == 0
            || request_length > BUFFER_LIMIT
            || path.is_null()
            || path_length == 0
            || path_length > 16_384
        {
            return Err("account_invalid".to_owned());
        }
        let request: RestoreRequest =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, request_length) })
                .map_err(|_| "account_invalid".to_owned())?;
        let path = std::str::from_utf8(unsafe { std::slice::from_raw_parts(path, path_length) })
            .map_err(|_| "account_invalid".to_owned())?;
        let path = Path::new(path);
        if !path.is_absolute() {
            return Err("account_invalid".to_owned());
        }
        let client = BackendAccountClient::new().map_err(|error| error.code().to_owned())?;
        restore_snapshot_with(request, path, |path, revision, access_token| {
            client.restore_dictionary_snapshot_file(path, revision, access_token)
        })
    })
}

/// Read a preview version binding canonical paths and the consistent Engine journal.
/// # Safety
/// `options` points to `length` readable bytes. Trusted native paths only.
#[no_mangle]
pub unsafe extern "C" fn msime_client_snapshot_version(
    options: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if options.is_null() || length > REQUEST_LIMIT {
            return Err("invalid snapshot buffer".into());
        }
        let options = parse_options(unsafe { std::slice::from_raw_parts(options, length) })?;
        let version = version(&options)?;
        let generation = activation_receipt(&options)?.unwrap_or_else(|| "legacy".to_owned());
        Ok(json!({"version": version, "generation": generation}))
    })
}

/// Prepare from a native callback; holds no registry lock while calling the host.
/// # Safety
/// Request/context remain valid for this synchronous call. The callback obeys
/// SnapshotNext, writes at most capacity bytes, and does not unwind or retain buffer.
#[no_mangle]
pub unsafe extern "C" fn msime_client_snapshot_prepare(
    request: *const u8,
    length: usize,
    next: Option<SnapshotNext>,
    context: *mut c_void,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > REQUEST_LIMIT {
            return Err("invalid snapshot buffer".into());
        }
        let next = next.ok_or("missing snapshot reader")?;
        let request: PrepareRequest =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, length) })
                .map_err(|_| "invalid snapshot request")?;
        // 与准备宿主时相同：按文档记录的版本的锁校验资源、计算代次。
        let specification = request
            .options
            .edition()
            .resource_set()
            .map_err(|_| "snapshot resources rejected")?;
        let mut buffer = vec![0; BUFFER_LIMIT];
        let stream = std::iter::from_fn(move || {
            let length = unsafe { next(context, buffer.as_mut_ptr(), buffer.len()) };
            if length == 0 {
                return None;
            }
            if length < 0 || length as usize > buffer.len() {
                return Some(Err(SnapshotReadError));
            }
            Some(record::decode(&buffer[..length as usize]))
        });
        let prepared = prepare(request, &specification, crate::ON_DEMAND_ARTIFACTS, stream)?;
        register(prepared).map_err(Into::into)
    })
}

/// Discard only a process-owned, unpublished preparation. Unknown/consumed IDs fail.
fn discard(handle: u64) -> Result<Value, &'static str> {
    let mut entries = registry()
        .lock()
        .map_err(|_| "snapshot registry unavailable")?;
    let prepared = entries.get(&handle).ok_or("unknown snapshot handle")?;
    // A prepared directory is outside all active roots (validated by prepare),
    // so cleaning it never touches the live journal or dictionaries. Requiring
    // the maintenance lock here made cancellation fail while an input session
    // held its normal shared lock, leaking the process-owned handle.
    std::fs::remove_dir_all(prepared.directory.path()).map_err(|_| "snapshot cleanup failed")?;
    entries.remove(&handle);
    Ok(json!({"discarded": true}))
}

#[no_mangle]
pub extern "C" fn msime_client_snapshot_discard(handle: u64) -> *mut c_char {
    response(|| discard(handle).map_err(|error| error.to_string()))
}

#[no_mangle]
pub extern "C" fn msime_client_snapshot_activate(
    handle: u64,
    expected: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if expected.is_null() || length != 64 {
            return Err("invalid snapshot version".into());
        }
        let expected = std::str::from_utf8(unsafe { std::slice::from_raw_parts(expected, length) })
            .map_err(|_| "invalid snapshot version")?;
        activate(handle, expected).map_err(Into::into)
    })
}

#[cfg(test)]
mod tests;
