use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};

use super::records::{LearningSyncRecord, LearningSyncState};

const FORMAT: &str = "msime-learning-sync";
const MAX_RECORDS: usize = 500_000;
const MAX_PLAINTEXT_BYTES: usize = 128 * 1024 * 1024;

/// 不带输入内容或解析器原始错误的传输边界错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LearningSyncReadError {
    #[error("Invalid learning sync stream")]
    Invalid,
    #[error("Unsupported learning sync version")]
    UnsupportedVersion,
    #[error("Learning sync stream exceeds limit")]
    TooLarge,
    #[error("Learning sync stream I/O failed")]
    Io,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningSyncRead {
    pub content_id: String,
    pub state: LearningSyncState,
    /// 不认识的类型或词库仍计入 footer 的记录数，但不应用。
    pub skipped: usize,
    /// 包括格式有效但词库种类未知的记录；不能只从显示投影计算重置屏障。
    pub observed_max: u64,
}

#[derive(Serialize, Deserialize)]
struct Header {
    #[serde(rename = "type")]
    record_type: String,
    format: String,
    version: u64,
    content_id: String,
}

#[derive(Serialize, Deserialize)]
struct Footer {
    #[serde(rename = "type")]
    record_type: String,
    records: usize,
}

fn invalid<T>(_: T) -> LearningSyncReadError {
    LearningSyncReadError::Invalid
}

/// 整份通过页尾、数量、类型和大小检查后才返回，不向调用方泄露半份可应用状态。
pub fn read_learning_sync(reader: impl BufRead) -> Result<LearningSyncRead, LearningSyncReadError> {
    // take 的边界包含额外一个字节：超限输入即使是一行也不能无限分配内存。
    let mut reader = reader.take((MAX_PLAINTEXT_BYTES + 1) as u64);
    let mut line = Vec::new();
    let mut bytes = 0usize;
    let mut read_line = |line: &mut Vec<u8>| -> Result<usize, LearningSyncReadError> {
        line.clear();
        let count = reader
            .read_until(b'\n', line)
            .map_err(|_| LearningSyncReadError::Io)?;
        bytes += count;
        if bytes > MAX_PLAINTEXT_BYTES {
            return Err(LearningSyncReadError::TooLarge);
        }
        Ok(count)
    };
    if read_line(&mut line)? == 0 {
        return Err(LearningSyncReadError::Invalid);
    }
    let header: Header = serde_json::from_slice(&line).map_err(invalid)?;
    if header.record_type != "header"
        || header.format != FORMAT
        || header.version == 0
        || header.content_id.is_empty()
        || header.content_id.len() > 512
        || header.content_id.contains('\0')
    {
        return Err(LearningSyncReadError::Invalid);
    }
    if header.version > 1 {
        return Err(LearningSyncReadError::UnsupportedVersion);
    }
    let mut result = LearningSyncRead {
        content_id: header.content_id,
        state: LearningSyncState::default(),
        skipped: 0,
        observed_max: 0,
    };
    let mut records = 0;
    loop {
        if read_line(&mut line)? == 0 {
            return Err(LearningSyncReadError::Invalid);
        }
        let value: serde_json::Value = serde_json::from_slice(&line).map_err(invalid)?;
        let record_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or(LearningSyncReadError::Invalid)?;
        if record_type == "footer" {
            let footer: Footer = serde_json::from_slice(&line).map_err(invalid)?;
            if footer.records != records || read_line(&mut line)? != 0 {
                return Err(LearningSyncReadError::Invalid);
            }
            return Ok(result);
        }
        if record_type == "header" {
            return Err(LearningSyncReadError::Invalid);
        }
        if records == MAX_RECORDS {
            return Err(LearningSyncReadError::TooLarge);
        }
        records += 1;
        if !matches!(record_type, "weight" | "delete" | "pin" | "position") {
            result.skipped += 1;
            continue;
        }
        // 从原字节反序列化，保留 serde 对重复字段的检查；Value 会合并同名键。
        let record: LearningSyncRecord = serde_json::from_slice(&line).map_err(invalid)?;
        if !record.valid() {
            return Err(LearningSyncReadError::Invalid);
        }
        result.observed_max = result.observed_max.max(record.updated_at());
        if record.supported() {
            result.state.insert(record)?;
        } else {
            result.skipped += 1;
        }
    }
}

/// 输出与设备合并顺序无关的确定字节；大小超限时不写半份流，由上传层决定截断策略。
pub fn write_learning_sync(
    state: &LearningSyncState,
    content_id: &str,
    mut writer: impl Write,
) -> Result<(), LearningSyncReadError> {
    if content_id.is_empty() || content_id.len() > 512 || content_id.contains('\0') {
        return Err(LearningSyncReadError::Invalid);
    }
    if state.records().len() > MAX_RECORDS {
        return Err(LearningSyncReadError::TooLarge);
    }
    let mut bytes = Vec::new();
    let header = Header {
        record_type: "header".into(),
        format: FORMAT.into(),
        version: 1,
        content_id: content_id.into(),
    };
    serde_json::to_writer(&mut bytes, &header).map_err(invalid)?;
    bytes.push(b'\n');
    for record in state.records() {
        serde_json::to_writer(&mut bytes, record).map_err(invalid)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_PLAINTEXT_BYTES {
            return Err(LearningSyncReadError::TooLarge);
        }
    }
    let footer = Footer {
        record_type: "footer".into(),
        records: state.records().len(),
    };
    serde_json::to_writer(&mut bytes, &footer).map_err(invalid)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_PLAINTEXT_BYTES {
        return Err(LearningSyncReadError::TooLarge);
    }
    writer
        .write_all(&bytes)
        .map_err(|_| LearningSyncReadError::Io)
}
