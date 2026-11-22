//! 上传前按确定顺序截断调权；删除、置顶和完整固定候选始终保留。

use msime_client_core::cloud::learning_e2e::{
    blob_envelope_size, Keystore, SealError, MAX_ENVELOPE_BYTES, MAX_STREAM_BYTES,
};
use msime_engine::{
    write_selected_learning_sync, LearningSyncReadError, LearningSyncRecord, LearningSyncState,
    MAX_LEARNING_SYNC_RECORDS, MAX_LEARNING_SYNC_TIMESTAMP,
};
use sha2::{Digest, Sha256};

/// 摘要只对应实际要上传的明文；不实现 Debug，避免调试输出带出学习记录。
pub struct PreparedUpload {
    stream: Vec<u8>,
    pub sha256: String,
    pub dropped_weights: usize,
    pub records: usize,
    /// 包括本轮因大小被省略的行，发布前仍须持久化完整导出的观察时间。
    pub observed_max: u64,
}

impl PreparedUpload {
    pub fn stream(&self) -> &[u8] {
        &self.stream
    }

    /// 代次由持久进度计划；准备明文不推进 uploaded，上传成功后才记录。
    pub fn seal(&self, keys: &Keystore, generation: u64) -> Result<Vec<u8>, SealError> {
        keys.seal_blob(generation, &self.stream)
    }
}

#[derive(Clone, Copy)]
struct Limits {
    records: usize,
    stream: usize,
    envelope: usize,
}

fn weight_order(record: &LearningSyncRecord) -> Option<(u64, &str, &str, &str)> {
    match record {
        LearningSyncRecord::Weight {
            dictionary,
            key,
            value,
            updated_at,
            ..
        } => Some((*updated_at, dictionary, key, value)),
        _ => None,
    }
}

/// 只丢按 `(updated_at,dictionary,key,value)` 排序的最旧调权前缀。
/// 受保护记录仍放不下时报 TooLarge，调用方继续下载合并其他设备，不清空本机数据。
/// `observed_max` 来自完整导出报告，包括未传输的未知词库时间。
pub fn prepare_upload(
    state: &LearningSyncState,
    content_id: &str,
    observed_max: u64,
) -> Result<PreparedUpload, LearningSyncReadError> {
    if observed_max < state.observed_max() || observed_max > MAX_LEARNING_SYNC_TIMESTAMP {
        return Err(LearningSyncReadError::Invalid);
    }
    let mut prepared = prepare(
        state,
        content_id,
        Limits {
            records: MAX_LEARNING_SYNC_RECORDS,
            stream: MAX_STREAM_BYTES,
            envelope: MAX_ENVELOPE_BYTES,
        },
    )?;
    // 导出报告还可能观察到未知词库的有效时间，不能只取可传输状态的最大值。
    prepared.observed_max = observed_max;
    Ok(prepared)
}

fn prepare(
    state: &LearningSyncState,
    content_id: &str,
    limits: Limits,
) -> Result<PreparedUpload, LearningSyncReadError> {
    let mut weights: Vec<_> = state.records().filter_map(weight_order).collect();
    weights.sort_unstable();
    let minimum = state.records().len().saturating_sub(limits.records);
    if minimum > weights.len() {
        return Err(LearningSyncReadError::TooLarge);
    }
    let attempt = |omitted: usize| -> Result<PreparedUpload, LearningSyncReadError> {
        let cutoff = omitted.checked_sub(1).map(|index| weights[index]);
        let mut stream = Vec::new();
        write_selected_learning_sync(
            state,
            content_id,
            |record| match (weight_order(record), cutoff) {
                (Some(order), Some(cutoff)) => order > cutoff,
                _ => true,
            },
            &mut stream,
        )?;
        if stream.len() > limits.stream
            || blob_envelope_size(&stream).map_err(|_| LearningSyncReadError::TooLarge)?
                > limits.envelope
        {
            return Err(LearningSyncReadError::TooLarge);
        }
        Ok(PreparedUpload {
            sha256: hex::encode(Sha256::digest(&stream)),
            stream,
            dropped_weights: omitted,
            records: state.records().len() - omitted,
            observed_max: state.observed_max(),
        })
    };
    match attempt(minimum) {
        Ok(prepared) => return Ok(prepared),
        Err(LearningSyncReadError::TooLarge) => {}
        Err(error) => return Err(error),
    }
    if minimum == weights.len() {
        return Err(LearningSyncReadError::TooLarge);
    }
    // 保留已实测放得下的上界；gzip 大小不严格单调，不声称找到最少丢弃量。
    // 二分最多再编码 log2(n) 次，避免逐条重压缩 50 万行的平方开销。
    let mut prepared = attempt(weights.len())?;
    let mut failed = minimum;
    let mut fitted = weights.len();
    while fitted - failed > 1 {
        let middle = failed + (fitted - failed) / 2;
        match attempt(middle) {
            Ok(next) => {
                fitted = middle;
                prepared = next;
            }
            Err(LearningSyncReadError::TooLarge) => failed = middle,
            Err(error) => return Err(error),
        }
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests;
