//! 上传 MCP 日志快照：把 [`super::build_bundle`] 给出的 `sections` 按服务端的规则重建后 `POST /v1/users/me/diagnostics`（msime-cloud `internal/account/diagnostics.go` 的 `validateDiagnosticUpload` 与 `validateDiagnosticConfig`）。服务端只要有一处不合规就拒绝整份，所以这里对每条规则做截断或替换，而不只是检查：
//!
//! - 事件只保留服务端认的 10 种，只留 `t_ms`、`kind`、`duration_ms`；性能记录缺 `duration_ms` 时补 0；每类最多 5000 条，留最近的。
//! - 崩溃记录最多 200 条，留最新的；`at` 按 RFC 3339 解析后重新格式化，解析不了的整条丢掉；`message` 截到 2 KiB、`stack` 截到 16 KiB，都在字符边界上截；含 NUL 的整条丢掉。
//! - 配置快照：嵌套超过 8 层的子树换成字符串 `"<truncated>"`；按确定的顺序遍历，条目满 2000 后不再加入；字符串截到 1024 字节并去掉 NUL；键名超过 128 字节或含 NUL 的条目丢掉；键名像凭据的（与服务端的 `(?i)token|secret|password|api_key|key$` 相同，用字符串比较实现）值换成 `"<redacted>"`。
//! - 整份超过 2 MiB 时先截掉最旧的事件。
//!
//! 查看、换令牌和删除快照（`GET`、`POST .../token`、`DELETE`）只发请求、校验响应；绑定到哪个账号由调用方决定。

use super::{DiagnosticBundle, DiagnosticCounts, SectionCount};
use crate::account::{parse_rfc3339, AccountError, AccountFailure, BackendAccountClient};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::time::Duration;

pub const DIAGNOSTICS_PATH: &str = "/v1/users/me/diagnostics";
pub const DIAGNOSTICS_TOKEN_PATH: &str = "/v1/users/me/diagnostics/token";
/// 可选的保存时长（服务端 `diagnosticTTLs`）。
pub const DIAGNOSTIC_TTLS: [&str; 3] = ["one_hour", "one_day", "seven_days"];
/// 服务端认的事件种类（`DiagnosticEventKinds`）；本机诊断包里的 `input_failure_*`、`voice_*` 只在本地导出。
pub const UPLOAD_EVENT_KINDS: [&str; 10] = [
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
/// 整份上传的上限（服务端 `diagnosticsBodyBytes`）。
pub const MAX_UPLOAD_BYTES: usize = 2 << 20;
/// 每类事件最多上传的条数。服务端的上限是 50000，本地取更小的值。
pub const MAX_UPLOAD_EVENTS: usize = 5_000;
const MAX_UPLOAD_CRASHES: usize = 200;
const MAX_CRASH_MESSAGE_BYTES: usize = 2 << 10;
const MAX_CRASH_STACK_BYTES: usize = 16 << 10;
const MAX_CONFIG_DEPTH: usize = 8;
const MAX_CONFIG_KEY_BYTES: usize = 128;
const MAX_CONFIG_STRING_BYTES: usize = 1024;
const MAX_CONFIG_ENTRIES: usize = 2000;
const REDACTED: &str = "<redacted>";
const TRUNCATED: &str = "<truncated>";
/// 服务端的路由时限。
const DIAGNOSTICS_TIMEOUT: Duration = Duration::from_secs(15);

/// 重建好、可以上传的快照，以及重建后各类保留和丢掉的条数。
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticUpload {
    body: Vec<u8>,
    pub counts: DiagnosticCounts,
}

impl DiagnosticUpload {
    /// 要发的请求体 `{platform, app_version, ttl, sections}`。
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

/// 上传成功后服务端给的回执。`token` 只出现这一次，不写日志。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticUploadReceipt {
    pub id: String,
    pub mcp_url: String,
    pub token: String,
    pub expires_at: String,
}

/// 本人有效快照的元数据（不含内容和令牌）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticSnapshotInfo {
    pub id: String,
    pub created_at: String,
    pub expires_at: String,
    pub bytes: u64,
    pub sections: Vec<String>,
    pub token_hint: String,
}

/// MCP 端点读取快照的一条访问记录。`arguments` 是服务端记下的原样 JSON。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticAccess {
    pub at: String,
    pub tool: String,
    pub arguments: Value,
    pub result_count: u64,
    pub bytes: u64,
}

/// `GET /v1/users/me/diagnostics` 的结果：没有有效快照时 `snapshot` 为 `None`。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticState {
    pub snapshot: Option<DiagnosticSnapshotInfo>,
    pub accesses: Vec<DiagnosticAccess>,
}

/// `platform` 和 `app_version` 的形状：`^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$`（服务端 `diagnosticLabelPattern`）。
pub fn valid_diagnostic_label(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && bytes[0].is_ascii_alphanumeric()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(byte))
}

/// 键名是不是服务端要求脱敏的凭据：不分大小写地含 `token`、`secret`、`password` 或 `api_key`，或以 `key` 结尾。
fn credential_key(key: &str) -> bool {
    let key = key.to_lowercase();
    ["token", "secret", "password", "api_key"]
        .iter()
        .any(|part| key.contains(part))
        || key.ends_with("key")
}

/// 按服务端的规则重建 `bundle.sections` 并组好请求体。`ttl` 不是三种之一、`platform` 或 `app_version` 形状不对、一个分类都没有勾选时是 `account_invalid`；去掉全部事件后仍超过 2 MiB 时是 `account_too_large`。
pub fn prepare_upload(
    platform: &str,
    app_version: &str,
    ttl: &str,
    bundle: DiagnosticBundle,
) -> Result<DiagnosticUpload, AccountFailure> {
    if !valid_diagnostic_label(platform)
        || !valid_diagnostic_label(app_version)
        || !DIAGNOSTIC_TTLS.contains(&ttl)
    {
        return Err(AccountError::Invalid.into());
    }
    let mut sections = bundle.sections.unwrap_or_default();
    if sections.is_empty() {
        return Err(AccountError::Invalid.into());
    }
    let mut counts = bundle.counts;
    let mut rebuilt = Map::new();
    if let Some(Value::Array(crashes)) = sections.remove("crash_logs") {
        let (records, adjust) = rebuild_crashes(crashes);
        adjust.apply(
            counts.crash_logs.get_or_insert_with(SectionCount::default),
            records.len(),
        );
        rebuilt.insert("crash_logs".into(), Value::Array(records));
    }
    let mut perf = None;
    if let Some(Value::Array(events)) = sections.remove("perf_trace") {
        let (records, adjust) = rebuild_events(events, true);
        adjust.apply(
            counts
                .performance_logs
                .get_or_insert_with(SectionCount::default),
            records.len(),
        );
        perf = Some(records);
    }
    let mut input = None;
    if let Some(Value::Array(events)) = sections.remove("input_events") {
        let (records, adjust) = rebuild_events(events, false);
        adjust.apply(
            counts
                .input_events
                .get_or_insert_with(SectionCount::default),
            records.len(),
        );
        input = Some(records);
    }
    if let Some(config) = sections.remove("config_snapshot") {
        let config = match config {
            Value::Object(_) => {
                let mut entries = 0;
                rebuild_config(config, 1, &mut entries)
            }
            _ => Value::Object(Map::new()),
        };
        rebuilt.insert("config_snapshot".into(), config);
    }
    if rebuilt.is_empty() && perf.is_none() && input.is_none() {
        return Err(AccountError::Invalid.into());
    }
    let body =
        |rebuilt: &Map<String, Value>, perf: &Option<Vec<Value>>, input: &Option<Vec<Value>>| {
            let mut sections = rebuilt.clone();
            if let Some(perf) = perf {
                sections.insert("perf_trace".into(), Value::Array(perf.clone()));
            }
            if let Some(input) = input {
                sections.insert("input_events".into(), Value::Array(input.clone()));
            }
            serde_json::to_vec(&serde_json::json!({
                "platform": platform,
                "app_version": app_version,
                "ttl": ttl,
                "sections": sections,
            }))
            .map_err(|_| AccountFailure::from(AccountError::Invalid))
        };
    let mut encoded = body(&rebuilt, &perf, &input)?;
    while encoded.len() > MAX_UPLOAD_BYTES {
        let remaining = perf.as_ref().map_or(0, Vec::len) + input.as_ref().map_or(0, Vec::len);
        if remaining == 0 {
            return Err(AccountFailure::too_large());
        }
        // 按最前面那条的长度估计要去掉多少条，至少一条；每轮都按时间先去掉两类里更早的那条。
        let sample = [perf.as_ref(), input.as_ref()]
            .into_iter()
            .flatten()
            .find_map(|records| records.first())
            .map_or(16, |record| record.to_string().len() + 1);
        let excess = encoded.len() - MAX_UPLOAD_BYTES;
        for _ in 0..excess.div_ceil(sample).max(1) {
            let oldest = |records: &Option<Vec<Value>>| {
                records
                    .as_ref()
                    .and_then(|records| records.first())
                    .and_then(|record| record.get("t_ms"))
                    .and_then(Value::as_u64)
            };
            let from_perf = match (oldest(&perf), oldest(&input)) {
                (Some(left), Some(right)) => left <= right,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            };
            let (records, count) = if from_perf {
                (perf.as_mut(), counts.performance_logs.as_mut())
            } else {
                (input.as_mut(), counts.input_events.as_mut())
            };
            if let Some(records) = records {
                records.remove(0);
                if let Some(count) = count {
                    count.kept = records.len();
                    count.truncated += 1;
                }
            }
        }
        encoded = body(&rebuilt, &perf, &input)?;
    }
    Ok(DiagnosticUpload {
        body: encoded,
        counts,
    })
}

/// 重建时额外丢掉和略去的条数。
#[derive(Default)]
struct Adjust {
    dropped: usize,
    truncated: usize,
}

impl Adjust {
    fn apply(&self, count: &mut SectionCount, kept: usize) {
        count.kept = kept;
        count.dropped += self.dropped;
        count.truncated += self.truncated;
    }
}

fn rebuild_events(events: Vec<Value>, duration_required: bool) -> (Vec<Value>, Adjust) {
    let mut adjust = Adjust::default();
    let mut records: Vec<Value> = events
        .into_iter()
        .filter_map(|event| {
            let record = rebuild_event(&event, duration_required);
            if record.is_none() {
                adjust.dropped += 1;
            }
            record
        })
        .collect();
    if records.len() > MAX_UPLOAD_EVENTS {
        adjust.truncated += records.len() - MAX_UPLOAD_EVENTS;
        records.drain(..records.len() - MAX_UPLOAD_EVENTS);
    }
    (records, adjust)
}

fn rebuild_event(event: &Value, duration_required: bool) -> Option<Value> {
    let kind = event.get("kind")?.as_str()?;
    if !UPLOAD_EVENT_KINDS.contains(&kind) {
        return None;
    }
    let millis = |value: &Value| value.as_u64().filter(|value| i64::try_from(*value).is_ok());
    let t_ms = millis(event.get("t_ms")?)?;
    let duration = match event.get("duration_ms") {
        Some(value) => Some(millis(value)?),
        None if duration_required => Some(0),
        None => None,
    };
    let mut record = Map::new();
    record.insert("t_ms".into(), t_ms.into());
    record.insert("kind".into(), kind.into());
    if let Some(duration) = duration {
        record.insert("duration_ms".into(), duration.into());
    }
    Some(Value::Object(record))
}

fn rebuild_crashes(crashes: Vec<Value>) -> (Vec<Value>, Adjust) {
    let mut adjust = Adjust::default();
    let mut records: Vec<(time::OffsetDateTime, Value)> = crashes
        .into_iter()
        .filter_map(|crash| {
            let record = rebuild_crash(&crash);
            if record.is_none() {
                adjust.dropped += 1;
            }
            record
        })
        .collect();
    records.sort_by_key(|(at, _)| *at);
    if records.len() > MAX_UPLOAD_CRASHES {
        adjust.truncated += records.len() - MAX_UPLOAD_CRASHES;
        records.drain(..records.len() - MAX_UPLOAD_CRASHES);
    }
    (
        records.into_iter().map(|(_, record)| record).collect(),
        adjust,
    )
}

fn rebuild_crash(crash: &Value) -> Option<(time::OffsetDateTime, Value)> {
    let parsed = parse_rfc3339(crash.get("at")?.as_str()?)?;
    let at = parsed
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
        .filter(|at| at.len() <= 64)?;
    let text = |key: &str, maximum: usize| {
        let value = crash.get(key)?.as_str()?;
        if value.contains('\0') {
            return None;
        }
        let mut value = value.to_owned();
        crate::account::truncate_at_char_boundary(&mut value, maximum);
        Some(value)
    };
    let message = text("message", MAX_CRASH_MESSAGE_BYTES)?;
    let stack = text("stack", MAX_CRASH_STACK_BYTES)?;
    Some((
        parsed,
        serde_json::json!({ "at": at, "message": message, "stack": stack }),
    ))
}

/// 重建配置快照的一个节点。`depth` 是它所在的层数，顶层对象是 1；服务端拒绝第 9 层出现任何值，所以第 8 层非空的对象和数组换成 `"<truncated>"`。`entries` 是已经加入的条目数，对象的每个键和数组的每个元素各算一条。
fn rebuild_config(value: Value, depth: usize, entries: &mut usize) -> Value {
    match value {
        Value::Object(object) => {
            if depth >= MAX_CONFIG_DEPTH && !object.is_empty() {
                return Value::String(TRUNCATED.into());
            }
            let mut rebuilt = Map::new();
            for (key, child) in object {
                if *entries >= MAX_CONFIG_ENTRIES {
                    break;
                }
                if key.len() > MAX_CONFIG_KEY_BYTES || key.contains('\0') {
                    continue;
                }
                *entries += 1;
                let child = if credential_key(&key) {
                    Value::String(REDACTED.into())
                } else {
                    rebuild_config(child, depth + 1, entries)
                };
                rebuilt.insert(key, child);
            }
            Value::Object(rebuilt)
        }
        Value::Array(items) => {
            if depth >= MAX_CONFIG_DEPTH && !items.is_empty() {
                return Value::String(TRUNCATED.into());
            }
            let mut rebuilt = Vec::new();
            for item in items {
                if *entries >= MAX_CONFIG_ENTRIES {
                    break;
                }
                *entries += 1;
                rebuilt.push(rebuild_config(item, depth + 1, entries));
            }
            Value::Array(rebuilt)
        }
        Value::String(text) => {
            let mut text = text.replace('\0', "");
            crate::account::truncate_at_char_boundary(&mut text, MAX_CONFIG_STRING_BYTES);
            Value::String(text)
        }
        other => other,
    }
}

fn validate_receipt(receipt: &DiagnosticUploadReceipt) -> Result<(), AccountError> {
    if !crate::text::is_lower_hex(&receipt.id, 24)
        || !receipt.mcp_url.starts_with("https://")
        || !crate::text::is_bounded_text(&receipt.mcp_url, 512)
        || !receipt.token.starts_with("msk_")
        || receipt.token.len() > 128
        || !crate::text::is_ascii_graphic(&receipt.token)
        || parse_rfc3339(&receipt.expires_at).is_none()
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn validate_state(state: &DiagnosticState) -> Result<(), AccountError> {
    let snapshot_valid = state.snapshot.as_ref().is_none_or(|snapshot| {
        crate::text::is_lower_hex(&snapshot.id, 24)
            && parse_rfc3339(&snapshot.created_at).is_some()
            && parse_rfc3339(&snapshot.expires_at).is_some()
            && snapshot.sections.len() <= 4
            && snapshot
                .sections
                .iter()
                .all(|section| crate::text::is_bounded_text(section, 32))
            && crate::text::is_bounded_text(&snapshot.token_hint, 16)
    });
    let accesses_valid = state.accesses.len() <= 50
        && state.accesses.iter().all(|access| {
            parse_rfc3339(&access.at).is_some() && crate::text::is_bounded_text(&access.tool, 128)
        });
    if snapshot_valid && accesses_valid {
        Ok(())
    } else {
        Err(AccountError::Unavailable)
    }
}

impl BackendAccountClient {
    /// 上传 [`prepare_upload`] 重建好的快照，替换本人原有的那份。
    pub fn diagnostics_upload(
        &self,
        upload: &DiagnosticUpload,
        access_token: &str,
    ) -> Result<DiagnosticUploadReceipt, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let bytes = self.request_detailed(
            Method::POST,
            DIAGNOSTICS_PATH,
            Some(access_token),
            Some(upload.body.clone()),
            MAX_UPLOAD_BYTES,
            64 * 1024,
            DIAGNOSTICS_TIMEOUT,
            "application/json",
        )?;
        let receipt: DiagnosticUploadReceipt =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        validate_receipt(&receipt)?;
        Ok(receipt)
    }

    pub fn diagnostics_state(&self, access_token: &str) -> Result<DiagnosticState, AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let bytes = self.request_detailed(
            Method::GET,
            DIAGNOSTICS_PATH,
            Some(access_token),
            None,
            64 * 1024,
            1024 * 1024,
            DIAGNOSTICS_TIMEOUT,
            "application/json",
        )?;
        let state: DiagnosticState =
            serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        validate_state(&state)?;
        Ok(state)
    }

    pub fn diagnostics_regenerate_token(
        &self,
        access_token: &str,
    ) -> Result<String, AccountFailure> {
        #[derive(Deserialize)]
        struct Token {
            token: String,
        }
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        let bytes = self.request_detailed(
            Method::POST,
            DIAGNOSTICS_TOKEN_PATH,
            Some(access_token),
            None,
            64 * 1024,
            64 * 1024,
            DIAGNOSTICS_TIMEOUT,
            "application/json",
        )?;
        let token: Token = serde_json::from_slice(&bytes).map_err(|_| AccountError::Unavailable)?;
        if !token.token.starts_with("msk_")
            || token.token.len() > 128
            || !crate::text::is_ascii_graphic(&token.token)
        {
            return Err(AccountError::Unavailable.into());
        }
        Ok(token.token)
    }

    pub fn diagnostics_delete(&self, access_token: &str) -> Result<(), AccountFailure> {
        if !crate::text::is_lower_hex(access_token, 64) {
            return Err(AccountError::Invalid.into());
        }
        self.request_detailed(
            Method::DELETE,
            DIAGNOSTICS_PATH,
            Some(access_token),
            None,
            64 * 1024,
            64 * 1024,
            DIAGNOSTICS_TIMEOUT,
            "application/json",
        )
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(sections: Value) -> DiagnosticBundle {
        DiagnosticBundle {
            sections: Some(sections.as_object().unwrap().clone()),
            ..Default::default()
        }
    }

    fn sections(upload: &DiagnosticUpload) -> Value {
        serde_json::from_slice::<Value>(upload.body()).unwrap()["sections"].clone()
    }

    #[test]
    fn events_keep_only_the_server_kinds_and_fields() {
        let upload = prepare_upload(
            "android",
            "0.9.0+42",
            "one_day",
            bundle(serde_json::json!({
                "perf_trace": [
                    { "t_ms": 1, "kind": "commit", "duration_ms": 3 },
                    { "t_ms": 2, "kind": "voice_success", "duration_ms": 3 },
                    { "t_ms": 3, "kind": "key_down" },
                ],
                "input_events": [
                    { "t_ms": 4, "kind": "input_failure_jni_link" },
                    { "t_ms": 5, "kind": "backspace" },
                    { "t_ms": u64::MAX, "kind": "backspace" },
                ],
            })),
        )
        .unwrap();
        let sections = sections(&upload);
        assert_eq!(
            sections["perf_trace"],
            serde_json::json!([
                { "t_ms": 1, "kind": "commit", "duration_ms": 3 },
                { "t_ms": 3, "kind": "key_down", "duration_ms": 0 },
            ])
        );
        assert_eq!(
            sections["input_events"],
            serde_json::json!([{ "t_ms": 5, "kind": "backspace" }])
        );
        let input = upload.counts.input_events.unwrap();
        assert_eq!((input.kept, input.dropped), (1, 2));
        assert_eq!(upload.counts.performance_logs.unwrap().dropped, 1);
    }

    #[test]
    fn crash_times_are_reformatted_and_bad_records_dropped() {
        let upload = prepare_upload(
            "ios",
            "1.0",
            "one_hour",
            bundle(serde_json::json!({
                "crash_logs": [
                    { "at": "2026-10-16T09:30:00.250+08:00", "message": "SIGSEGV", "stack": "#0 a" },
                    { "at": "2026-10-16 09:30", "message": "x", "stack": "" },
                    { "at": "2026-10-15T00:00:00Z", "message": "nul\u{0}", "stack": "" },
                    { "at": "2026-10-14T00:00:00Z", "message": "长".repeat(1000), "stack": "" },
                ],
            })),
        )
        .unwrap();
        let crashes = sections(&upload)["crash_logs"].as_array().unwrap().clone();
        assert_eq!(crashes.len(), 2);
        // 按时间排序：较早的那条在前。
        assert_eq!(crashes[0]["message"].as_str().unwrap().len(), 2046);
        assert_eq!(crashes[1]["at"], "2026-10-16T09:30:00.25+08:00");
        assert_eq!(upload.counts.crash_logs.unwrap().dropped, 2);
    }

    #[test]
    fn the_config_snapshot_is_rebuilt_to_the_server_limits() {
        let mut deep = serde_json::json!("leaf");
        for _ in 0..10 {
            deep = serde_json::json!({ "level": deep });
        }
        let many: Map<String, Value> = (0..2500)
            .map(|index| (format!("k{index:04}x"), Value::from(index)))
            .collect();
        let mut snapshot = bundle(serde_json::json!({
            "config_snapshot": {
                "deep": deep,
                "doubao_api_key": "sk-synthetic",
                "Access_Token": "synthetic",
                "hotkey": "ctrl",
                "long": "a".repeat(2000),
                "nul": "a\u{0}b",
                "zz_many": many,
            },
        }));
        snapshot.sections.as_mut().unwrap()["config_snapshot"]
            .as_object_mut()
            .unwrap()
            .insert("k".repeat(129), Value::from(1));
        let upload = prepare_upload("macos", "0.9.0", "seven_days", snapshot).unwrap();
        let config = sections(&upload)["config_snapshot"].clone();
        let mut node = &config["deep"];
        let mut depth = 2;
        while let Some(child) = node.get("level") {
            node = child;
            depth += 1;
        }
        assert_eq!(*node, Value::String(TRUNCATED.into()));
        assert_eq!(depth, 8);
        for key in ["doubao_api_key", "Access_Token", "hotkey"] {
            assert_eq!(config[key], REDACTED, "{key}");
        }
        assert_eq!(config["long"].as_str().unwrap().len(), 1024);
        assert_eq!(config["nul"], "ab");
        assert!(config.get("k".repeat(129)).is_none());
        let mut entries = 0;
        fn count(value: &Value, entries: &mut usize) {
            match value {
                Value::Object(object) => object.values().for_each(|child| {
                    *entries += 1;
                    count(child, entries);
                }),
                Value::Array(items) => items.iter().for_each(|child| {
                    *entries += 1;
                    count(child, entries);
                }),
                _ => {}
            }
        }
        count(&config, &mut entries);
        assert_eq!(entries, MAX_CONFIG_ENTRIES);
    }

    #[test]
    fn an_oversized_upload_drops_the_oldest_events_first() {
        let events: Vec<Value> = (0..MAX_UPLOAD_EVENTS)
            .map(|index| serde_json::json!({ "t_ms": index * 2, "kind": "candidate_selected", "duration_ms": 1 }))
            .collect();
        let inputs: Vec<Value> = (0..MAX_UPLOAD_EVENTS)
            .map(|index| serde_json::json!({ "t_ms": index * 2 + 1, "kind": "candidate_shown" }))
            .collect();
        let config: Map<String, Value> = (0..1999)
            .map(|index| (format!("key{index}name"), Value::String("v".repeat(1000))))
            .collect();
        let upload = prepare_upload(
            "windows",
            "0.9.0",
            "one_day",
            bundle(serde_json::json!({ "perf_trace": events, "input_events": inputs, "config_snapshot": config })),
        )
        .unwrap();
        assert!(upload.body().len() <= MAX_UPLOAD_BYTES);
        let sections = sections(&upload);
        let perf = sections["perf_trace"].as_array().unwrap();
        let input = sections["input_events"].as_array().unwrap();
        assert!(perf.len() < MAX_UPLOAD_EVENTS && input.len() < MAX_UPLOAD_EVENTS);
        // 最新的事件还在，去掉的是最早的。
        assert_eq!(perf.last().unwrap()["t_ms"], (MAX_UPLOAD_EVENTS - 1) * 2);
        let counts = upload.counts.performance_logs.unwrap();
        assert_eq!(counts.kept + counts.truncated, MAX_UPLOAD_EVENTS);
    }

    #[test]
    fn labels_ttl_and_empty_selections_are_refused_locally() {
        let one = || bundle(serde_json::json!({ "input_events": [] }));
        assert!(prepare_upload("android", "0.9.0", "one_day", one()).is_ok());
        for (platform, version, ttl) in [
            ("", "0.9.0", "one_day"),
            ("-android", "0.9.0", "one_day"),
            ("android", "0.9 beta", "one_day"),
            ("android", "0.9.0", "forever"),
        ] {
            assert_eq!(
                prepare_upload(platform, version, ttl, one())
                    .unwrap_err()
                    .code(),
                "account_invalid"
            );
        }
        assert_eq!(
            prepare_upload("android", "0.9.0", "one_day", DiagnosticBundle::default())
                .unwrap_err()
                .code(),
            "account_invalid"
        );
        assert!(valid_diagnostic_label(&"a".repeat(64)));
        assert!(!valid_diagnostic_label(&"a".repeat(65)));
    }
}
