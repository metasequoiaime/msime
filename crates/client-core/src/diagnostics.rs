//! 诊断包：开发者选项里「导出诊断包」写出的 zip，以及「上传 MCP 日志快照」上传的 `sections`。
//!
//! 输入事件和性能记录按 P19 的白名单逐行校验：一行只能是 `{"t_ms","kind","duration_ms"}` 这几个键，`kind` 只能是固定的事件种类，值只能是数字。任何多出来的键（例如误写进去的 `text`）、超出枚举的种类或不是 JSON 对象的行都整行丢弃并计数，绝不把输入内容带出本机。配置快照用 [`crate::preferences::Preferences::redacted_for_diagnostics`]，凭据一律换成 `"<redacted>"`。
//!
//! 来源文件由宿主给出绝对路径；文件不存在时这一类为空，不算失败。日志文件可能很大，只读最后 [`MAX_SOURCE_BYTES`] 字节，每类最多保留最近的若干行。

use crate::preferences::PreferencesStore;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use thiserror::Error;

/// 每个来源文件最多读的字节数（从文件末尾往前）。
pub const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
/// 输入事件最多保留的行数。
pub const MAX_INPUT_EVENTS: usize = 5_000;
/// 性能记录最多保留的行数。
pub const MAX_PERFORMANCE_RECORDS: usize = 5_000;
/// 崩溃记录最多保留的条数。
pub const MAX_CRASH_LOGS: usize = 50;
/// 崩溃信息 `message` 的字节上限，与服务端相同。
pub const MAX_CRASH_MESSAGE_BYTES: usize = 2 * 1024;
/// 崩溃堆栈 `stack` 的字节上限，与服务端相同。
pub const MAX_CRASH_STACK_BYTES: usize = 16 * 1024;
/// 单行的字节上限；更长的行直接丢弃。
const MAX_LINE_BYTES: usize = 32 * 1024;

/// P19 规定的事件种类，输入事件和性能记录共用。
pub const EVENT_KINDS: [&str; 10] = [
    "key_down",
    "key_up",
    "candidate_shown",
    "candidate_selected",
    "commit",
    "backspace",
    "panel_open",
    "panel_close",
    "ime_start",
    "ime_finish",
];

/// 要包含哪几类内容。缺省全部不包含。
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DiagnosticInclude {
    pub crash_logs: bool,
    pub performance_logs: bool,
    pub input_events: bool,
    pub config_snapshot: bool,
}

/// 各类日志的来源文件（绝对路径），没有时为空。
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DiagnosticSources {
    pub crash_logs: Option<String>,
    pub performance_logs: Option<String>,
    pub input_events: Option<String>,
}

/// 一次诊断包请求。`state_root` 是偏好目录（配置快照从这里读）。`destination` 是要写出的 zip 的绝对路径；为空时不写文件，而是把 `sections` 原样返回，供上传 MCP 快照。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticBundleRequest {
    pub state_root: String,
    #[serde(default)]
    pub include: DiagnosticInclude,
    #[serde(default)]
    pub sources: DiagnosticSources,
    #[serde(default)]
    pub destination: Option<String>,
}

/// 一类内容保留和丢弃的行数。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct SectionCount {
    pub kept: usize,
    /// 不符合白名单 schema 而丢弃的行。
    pub dropped: usize,
    /// 超过保留上限而略去的较早的行。
    pub truncated: usize,
}

/// 各类内容的计数；没有包含的类别为空。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DiagnosticCounts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash_logs: Option<SectionCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance_logs: Option<SectionCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_events: Option<SectionCount>,
    pub config_snapshot: bool,
}

/// 诊断包的结果。写了 zip 时 `path`、`bytes` 有值；没有 `destination` 时 `sections` 是上传接口要的对象（`crash_logs`、`perf_trace`、`input_events`、`config_snapshot`）。
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DiagnosticBundle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    pub counts: DiagnosticCounts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<Map<String, Value>>,
}

#[derive(Debug, Error)]
pub enum DiagnosticsError {
    #[error("diagnostic bundle request is invalid")]
    Invalid,
    #[error("diagnostic source could not be read")]
    Source(#[source] std::io::Error),
    #[error("preferences could not be read for the configuration snapshot")]
    Preferences,
    #[error("diagnostic bundle could not be written")]
    Write(#[source] std::io::Error),
}

impl DiagnosticsError {
    /// 交给宿主的稳定错误码。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid => "diagnostics_invalid",
            Self::Source(_) => "diagnostics_source",
            Self::Preferences => "diagnostics_preferences",
            Self::Write(_) => "diagnostics_write",
        }
    }
}

/// 一类日志的校验结果。
struct Section {
    records: Vec<Value>,
    count: SectionCount,
}

/// 按请求生成诊断包。
pub fn build_bundle(
    request: &DiagnosticBundleRequest,
) -> Result<DiagnosticBundle, DiagnosticsError> {
    if !Path::new(&request.state_root).is_absolute() {
        return Err(DiagnosticsError::Invalid);
    }
    for source in [
        &request.sources.crash_logs,
        &request.sources.performance_logs,
        &request.sources.input_events,
    ]
    .into_iter()
    .flatten()
    {
        if !Path::new(source).is_absolute() {
            return Err(DiagnosticsError::Invalid);
        }
    }
    let destination = match &request.destination {
        Some(path) => {
            let path = Path::new(path);
            if !path.is_absolute() || path.file_name().is_none() || path.is_dir() {
                return Err(DiagnosticsError::Invalid);
            }
            Some(path)
        }
        None => None,
    };

    let crash = request
        .include
        .crash_logs
        .then(|| read_crash_logs(request.sources.crash_logs.as_deref()))
        .transpose()?;
    let performance = request
        .include
        .performance_logs
        .then(|| {
            read_section(
                request.sources.performance_logs.as_deref(),
                MAX_PERFORMANCE_RECORDS,
                performance_record,
            )
        })
        .transpose()?;
    let input = request
        .include
        .input_events
        .then(|| {
            read_section(
                request.sources.input_events.as_deref(),
                MAX_INPUT_EVENTS,
                input_event_record,
            )
        })
        .transpose()?;
    let config = if request.include.config_snapshot {
        let snapshot = PreferencesStore::new(&request.state_root)
            .load()
            .map_err(|_| DiagnosticsError::Preferences)?;
        Some(snapshot.preferences.redacted_for_diagnostics())
    } else {
        None
    };

    let counts = DiagnosticCounts {
        crash_logs: crash.as_ref().map(|section| section.count),
        performance_logs: performance.as_ref().map(|section| section.count),
        input_events: input.as_ref().map(|section| section.count),
        config_snapshot: config.is_some(),
    };

    match destination {
        Some(path) => {
            let bytes = write_zip(path, &counts, &crash, &performance, &input, &config)?;
            Ok(DiagnosticBundle {
                path: Some(path.to_string_lossy().into_owned()),
                bytes: Some(bytes),
                counts,
                sections: None,
            })
        }
        None => {
            let mut sections = Map::new();
            if let Some(section) = crash {
                sections.insert("crash_logs".into(), Value::Array(section.records));
            }
            if let Some(section) = performance {
                sections.insert("perf_trace".into(), Value::Array(section.records));
            }
            if let Some(section) = input {
                sections.insert("input_events".into(), Value::Array(section.records));
            }
            if let Some(config) = config {
                sections.insert("config_snapshot".into(), config);
            }
            Ok(DiagnosticBundle {
                path: None,
                bytes: None,
                counts,
                sections: Some(sections),
            })
        }
    }
}

/// 崩溃记录的来源有两种：遥测的崩溃目录（[`crate::telemetry::CRASH_DIRECTORY`]，里面每个 `*.crash` 文件是一条记录，Android 宿主传的就是它），或者每行一条 `{at, message, stack}` 的文件。来源是目录时按目录读，其余照常按行读。
fn read_crash_logs(source: Option<&str>) -> Result<Section, DiagnosticsError> {
    if let Some(source) = source {
        let path = Path::new(source);
        if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir()) {
            return read_crash_directory(path);
        }
    }
    read_section(source, MAX_CRASH_LOGS, crash_record)
}

/// 读崩溃目录里的 `*.crash` 记录，按修改时间保留最近的 [`MAX_CRASH_LOGS`] 条。记录的格式与遥测写的相同：第一行是异常摘要，其余是栈帧；`at` 取文件的修改时间。只读普通文件，符号链接和子目录跳过；读不出或清洗后为空的记录计入丢弃，不让整个诊断包失败。
fn read_crash_directory(directory: &Path) -> Result<Section, DiagnosticsError> {
    let mut section = Section {
        records: Vec::new(),
        count: SectionCount::default(),
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(DiagnosticsError::Source)? {
        let entry = entry.map_err(DiagnosticsError::Source)?;
        let path = entry.path();
        let is_record = entry.file_type().is_ok_and(|kind| kind.is_file())
            && path.extension().and_then(|value| value.to_str())
                == Some(crate::telemetry::CRASH_EXTENSION);
        if !is_record {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        files.push((modified, path));
    }
    files.sort();
    if files.len() > MAX_CRASH_LOGS {
        let excess = files.len() - MAX_CRASH_LOGS;
        section.count.truncated = excess;
        files.drain(..excess);
    }
    for (modified, path) in files {
        match crash_file_record(&path, modified) {
            Some(record) => section.records.push(record),
            None => section.count.dropped += 1,
        }
    }
    section.count.kept = section.records.len();
    Ok(section)
}

/// 一个 `*.crash` 文件换成与 [`crash_record`] 相同的记录。
fn crash_file_record(path: &Path, modified: std::time::SystemTime) -> Option<Value> {
    let file = crate::storage::open_private_file_in(path).ok()?;
    let mut bytes = Vec::new();
    file.take(crate::telemetry::MAX_CRASH_RECORD_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let (message, stack) = text.split_once('\n').unwrap_or((&text, ""));
    let message = if message.trim().is_empty() {
        "unknown crash"
    } else {
        message
    };
    let at = time::OffsetDateTime::from(modified);
    let at = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    );
    sanitized_crash(&at, message, stack)
}

/// 读一个来源文件的最后 [`MAX_SOURCE_BYTES`] 字节，逐行交给 `accept` 校验，保留最近的 `limit` 行。
fn read_section(
    source: Option<&str>,
    limit: usize,
    accept: fn(&[u8]) -> Option<Value>,
) -> Result<Section, DiagnosticsError> {
    let mut section = Section {
        records: Vec::new(),
        count: SectionCount::default(),
    };
    let Some(source) = source else {
        return Ok(section);
    };
    let path = Path::new(source);
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(section),
        Err(error) => return Err(DiagnosticsError::Source(error)),
    };
    if !metadata.file_type().is_file() {
        return Err(DiagnosticsError::Invalid);
    }
    let mut file = crate::storage::open_private_file_in(path).map_err(DiagnosticsError::Source)?;
    let skip_partial = metadata.len() > MAX_SOURCE_BYTES;
    if skip_partial {
        file.seek(SeekFrom::Start(metadata.len() - MAX_SOURCE_BYTES))
            .map_err(DiagnosticsError::Source)?;
    }
    let mut reader = BufReader::new(file.take(MAX_SOURCE_BYTES));
    let mut line = Vec::new();
    let mut first = true;
    let mut kept = std::collections::VecDeque::with_capacity(limit.min(1024));
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(DiagnosticsError::Source)?;
        if read == 0 {
            break;
        }
        let complete = line.last() == Some(&b'\n');
        if !complete && line.len() > MAX_LINE_BYTES {
            // 超长的行：丢掉它剩下的部分。
            let mut rest = Vec::new();
            reader
                .read_until(b'\n', &mut rest)
                .map_err(DiagnosticsError::Source)?;
            if !(first && skip_partial) {
                section.count.dropped += 1;
            }
            first = false;
            continue;
        }
        // 从文件中间开始读时，第一行是不完整的半行。
        if first && skip_partial {
            first = false;
            continue;
        }
        first = false;
        let text = line.strip_suffix(b"\n").unwrap_or(&line);
        let text = text.strip_suffix(b"\r").unwrap_or(text);
        if text.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match accept(text) {
            Some(record) => {
                if kept.len() == limit {
                    kept.pop_front();
                    section.count.truncated += 1;
                }
                kept.push_back(record);
            }
            None => section.count.dropped += 1,
        }
    }
    section.records = kept.into_iter().collect();
    section.count.kept = section.records.len();
    Ok(section)
}

fn parse_object(line: &[u8]) -> Option<Map<String, Value>> {
    match serde_json::from_slice::<Value>(line).ok()? {
        Value::Object(object) => Some(object),
        _ => None,
    }
}

fn event_kind(object: &Map<String, Value>) -> Option<&'static str> {
    let kind = object.get("kind")?.as_str()?;
    EVENT_KINDS.iter().copied().find(|known| *known == kind)
}

/// 输入事件：`t_ms`、`kind` 必有，`duration_ms` 可选，此外不许有任何键。
pub fn input_event_record(line: &[u8]) -> Option<Value> {
    let object = parse_object(line)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "t_ms" | "kind" | "duration_ms"))
    {
        return None;
    }
    let mut record = Map::new();
    record.insert("t_ms".into(), object.get("t_ms")?.as_u64()?.into());
    record.insert("kind".into(), event_kind(&object)?.into());
    if let Some(duration) = object.get("duration_ms") {
        record.insert("duration_ms".into(), duration.as_u64()?.into());
    }
    Some(Value::Object(record))
}

/// 性能记录：恰好 `t_ms`、`kind`、`duration_ms` 三个键。
pub fn performance_record(line: &[u8]) -> Option<Value> {
    let object = parse_object(line)?;
    if object.len() != 3 || !object.contains_key("duration_ms") {
        return None;
    }
    input_event_record(line)
}

/// 崩溃记录：恰好 `at`、`message`、`stack` 三个字符串键，长度不超过服务端上限。异常说明可能带着正在输入的文字（比如解析错误引用了组字内容），所以 `message` 只留异常类型，`stack` 里栈帧原样保留、说明行只留异常类型，路径都只留文件名。
pub fn crash_record(line: &[u8]) -> Option<Value> {
    let object = parse_object(line)?;
    if object.len() != 3 {
        return None;
    }
    let at = object.get("at")?.as_str()?;
    let message = object.get("message")?.as_str()?;
    let stack = object.get("stack")?.as_str()?;
    if at.is_empty()
        || at.len() > 64
        || message.len() > MAX_CRASH_MESSAGE_BYTES
        || stack.len() > MAX_CRASH_STACK_BYTES
    {
        return None;
    }
    sanitized_crash(at, message, stack)
}

/// 崩溃记录清洗后的样子：`message` 只留异常类型，`stack` 里栈帧原样保留、说明行只留异常类型，路径都只留文件名。
fn sanitized_crash(at: &str, message: &str, stack: &str) -> Option<Value> {
    let message = crate::telemetry::clean_message(exception_type(message.lines().next()?));
    let stack: Vec<String> = stack
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if ["at ", "... ", "#"]
                .iter()
                .any(|frame| trimmed.starts_with(frame))
            {
                line.to_owned()
            } else if let Some(cause) = trimmed.strip_prefix("Caused by: ") {
                format!("Caused by: {}", exception_type(cause))
            } else {
                exception_type(trimmed).to_owned()
            }
        })
        .collect();
    let stack = crate::telemetry::clean_stack(&stack.join("\n"));
    let mut record = Map::new();
    record.insert("at".into(), at.into());
    record.insert("message".into(), message.into());
    record.insert("stack".into(), stack.into());
    Some(Value::Object(record))
}

/// `java.lang.IllegalStateException: 说明` 里冒号前的异常类型；没有冒号时是整行。
fn exception_type(line: &str) -> &str {
    line.split(':').next().unwrap_or(line).trim()
}

fn ndjson(records: &[Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend_from_slice(record.to_string().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

fn write_zip(
    destination: &Path,
    counts: &DiagnosticCounts,
    crash: &Option<Section>,
    performance: &Option<Section>,
    input: &Option<Section>,
    config: &Option<Value>,
) -> Result<u64, DiagnosticsError> {
    let parent = destination.parent().ok_or(DiagnosticsError::Invalid)?;
    let temporary = tempfile::NamedTempFile::new_in(parent).map_err(DiagnosticsError::Write)?;
    let mut writer = zip::ZipWriter::new(temporary);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut entry = |name: &str, bytes: &[u8]| -> Result<(), DiagnosticsError> {
        writer
            .start_file(name, options)
            .map_err(|error| DiagnosticsError::Write(std::io::Error::other(error)))?;
        writer.write_all(bytes).map_err(DiagnosticsError::Write)
    };
    let manifest = serde_json::json!({
        "format": "msime-diagnostics",
        "version": 1,
        "counts": counts,
    });
    entry("manifest.json", manifest.to_string().as_bytes())?;
    if let Some(section) = crash {
        entry("crash_logs.ndjson", &ndjson(&section.records))?;
    }
    if let Some(section) = performance {
        entry("performance_logs.ndjson", &ndjson(&section.records))?;
    }
    if let Some(section) = input {
        entry("input_events.ndjson", &ndjson(&section.records))?;
    }
    if let Some(config) = config {
        let text = serde_json::to_vec_pretty(config)
            .map_err(|error| DiagnosticsError::Write(std::io::Error::other(error)))?;
        entry("config_snapshot.json", &text)?;
    }
    let temporary = writer
        .finish()
        .map_err(|error| DiagnosticsError::Write(std::io::Error::other(error)))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(DiagnosticsError::Write)?;
    let file = temporary
        .persist(destination)
        .map_err(|error| DiagnosticsError::Write(error.error))?;
    let bytes = file.metadata().map_err(DiagnosticsError::Write)?.len();
    Ok(bytes)
}

#[cfg(test)]
mod tests;
