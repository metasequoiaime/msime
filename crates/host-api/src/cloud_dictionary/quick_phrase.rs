//! 云端完整快照的快捷短语筛选。
//!
//! 这段逻辑属于共享 Host API：桌面原生设置可以先导出快照，再让共享层移除
//! `quick` 记录，避免各个平台自己解析快照并重新计算校验和。

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const INVALID: &str = "snapshot_invalid";
const MAX_SNAPSHOT_BYTES: usize = 512 * 1024 * 1024;

/// 从一份已经导出的完整云词库快照中移除所有快捷短语及其学习记录。
///
/// 返回新的快照正文、被移除的用户快捷短语数量和原云端 revision。没有快捷短语
/// 时原文按字节返回，调用方可以直接提示“没有可清空的条目”而不发恢复请求。
pub fn filter_snapshot(text: &str) -> Result<Value, &'static str> {
    if text.is_empty() || text.len() > MAX_SNAPSHOT_BYTES {
        return Err(INVALID);
    }
    crate::dictionary_snapshot::inspect_snapshot_bytes(text.as_bytes()).map_err(|_| INVALID)?;
    let mut lines = text.split('\n').collect::<Vec<_>>();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if lines.len() < 2 {
        return Err(INVALID);
    }

    let header: Value = serde_json::from_str(lines[0]).map_err(|_| INVALID)?;
    if header.get("type").and_then(Value::as_str) != Some("header")
        || header.get("format").and_then(Value::as_str) != Some("msime-dictionary-snapshot")
        || header.get("version").and_then(Value::as_i64) != Some(1)
    {
        return Err(INVALID);
    }
    let revision = header
        .get("revision")
        .and_then(Value::as_i64)
        .filter(|revision| *revision >= 0)
        .ok_or(INVALID)?;

    let footer: Value = serde_json::from_str(lines[lines.len() - 1]).map_err(|_| INVALID)?;
    let expected_sha256 = footer
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or(INVALID)?;
    let expected_records = footer
        .get("records")
        .and_then(Value::as_u64)
        .ok_or(INVALID)?;
    let original_body = format!("{}\n", lines[..lines.len() - 1].join("\n"));
    let actual_sha256 = hex::encode(Sha256::digest(original_body.as_bytes()));
    if footer.get("type").and_then(Value::as_str) != Some("footer")
        || expected_records != (lines.len() - 1) as u64
        || expected_sha256 != actual_sha256
    {
        return Err(INVALID);
    }

    let records = lines[1..lines.len() - 1]
        .iter()
        .map(|line| serde_json::from_str::<Value>(line).map_err(|_| INVALID))
        .collect::<Result<Vec<_>, _>>()?;
    let quick_keys = records
        .iter()
        .filter(|record| record.get("type").and_then(Value::as_str) == Some("entry"))
        .filter_map(|record| {
            let data = record.get("data")?.as_object()?;
            if data.get("kind").and_then(Value::as_str) != Some("quick") {
                return None;
            }
            Some((
                data.get("code")?.as_str()?.to_owned(),
                data.get("word")?.as_str()?.to_owned(),
            ))
        })
        .collect::<HashSet<_>>();

    let mut kept = Vec::with_capacity(lines.len());
    kept.push(serde_json::to_string(&header).map_err(|_| INVALID)?);
    let mut entries = 0usize;
    for record in records {
        let record_type = record.get("type").and_then(Value::as_str);
        let data = record.get("data").and_then(Value::as_object);
        let kind_is_quick = data
            .and_then(|data| data.get("kind"))
            .and_then(Value::as_str)
            == Some("quick");
        let is_quick_learning = data
            .and_then(|data| Some((data.get("code")?.as_str()?, data.get("word")?.as_str()?)))
            .is_some_and(|key| quick_keys.contains(&(key.0.to_owned(), key.1.to_owned())));
        let removable = matches!(record_type, Some("entry" | "overlay")) && kind_is_quick
            || matches!(record_type, Some("position" | "selection")) && is_quick_learning;
        if removable {
            if record_type == Some("entry") {
                entries += 1;
            }
            continue;
        }
        kept.push(serde_json::to_string(&record).map_err(|_| INVALID)?);
    }

    if entries == 0 {
        return Ok(json!({"text": text, "entries": 0, "revision": revision}));
    }

    let body = format!("{}\n", kept.join("\n"));
    let sha256 = hex::encode(Sha256::digest(body.as_bytes()));
    let footer = json!({
        "type": "footer",
        "records": kept.len(),
        "sha256": sha256,
    });
    let footer = serde_json::to_string(&footer).map_err(|_| INVALID)?;
    let filtered = format!("{body}{footer}\n");
    Ok(json!({"text": filtered, "entries": entries, "revision": revision}))
}

#[cfg(test)]
mod tests {
    use super::filter_snapshot;
    use sha2::{Digest, Sha256};

    fn snapshot(records: &[&str]) -> String {
        let header =
            r#"{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}"#;
        let body = std::iter::once(header)
            .chain(records.iter().copied())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let digest = hex::encode(Sha256::digest(body.as_bytes()));
        format!(
            "{body}{{\"type\":\"footer\",\"records\":{},\"sha256\":\"{digest}\"}}\n",
            records.len() + 1
        )
    }

    #[test]
    fn removes_quick_entries_and_learning_records_but_keeps_other_kinds() {
        let text = snapshot(&[
            r#"{"type":"entry","data":{"id":"quick-entry","kind":"quick","code":"k","word":"快捷","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z","user_inserted":true}}"#,
            r#"{"type":"entry","data":{"id":"pinyin-entry","kind":"pinyin","code":"ni","word":"你","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z","user_inserted":true}}"#,
            r#"{"type":"overlay","deleted":false,"data":{"id":"quick-entry","kind":"quick","code":"k","word":"快捷","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z","user_inserted":true}}"#,
            r#"{"type":"overlay","deleted":false,"data":{"id":"pinyin-entry","kind":"pinyin","code":"ni","word":"你","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z","user_inserted":true}}"#,
            r#"{"type":"position","data":{"context":"k","code":"k","word":"快捷","position":1}}"#,
            r#"{"type":"selection","data":{"context":"k","code":"k","word":"快捷","count":1}}"#,
        ]);
        let result = filter_snapshot(&text).unwrap();
        assert_eq!(result["entries"], 1);
        let filtered = result["text"].as_str().unwrap();
        assert!(filtered.contains("\"kind\":\"pinyin\""));
        assert!(!filtered.contains("\"kind\":\"quick\""));
        assert!(filtered.ends_with('\n'));
        assert!(crate::dictionary_snapshot::inspect_snapshot_bytes(filtered.as_bytes()).is_ok());
    }

    #[test]
    fn preserves_original_bytes_when_no_quick_entry_exists() {
        let text = snapshot(&[
            r#"{"type":"entry","data":{"id":"pinyin-entry","kind":"pinyin","code":"ni","word":"你","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z","user_inserted":true}}"#,
            r#"{"type":"overlay","deleted":false,"data":{"id":"pinyin-entry","kind":"pinyin","code":"ni","word":"你","weight":1,"revision":1,"updated_at":"2026-09-01T00:00:00Z","user_inserted":true}}"#,
        ]);
        let result = filter_snapshot(&text).unwrap();
        assert_eq!(result["entries"], 0);
        assert_eq!(result["text"].as_str(), Some(text.as_str()));
    }

    #[test]
    fn rejects_malformed_snapshot_without_partial_output() {
        assert!(filter_snapshot("{\"type\":\"header\"}\n").is_err());
        assert!(filter_snapshot("{\"type\":\"header\",\"format\":\"msime-dictionary-snapshot\",\"version\":1,\"revision\":0}\nnot-json\n{}").is_err());
    }
}
