//! 输入习惯同步的明文记录与收敛合并，契约见 `docs/learning-sync.md`。
//!
//! 这里不联网、不解密。调用方必须先完成信封认证，再完整读取并校验明文流；只有整份流成功后才能接纳它的状态和时间上限。数据库应用另需独占维护权与账号持久化屏障。

mod database;
mod records;
mod stream;

pub use database::{
    merge_learning_sync_state, stream_learning_sync_state, LearningSyncExport, LearningSyncMerge,
};
pub use records::{
    LearningSyncRecord, LearningSyncState, MAX_LEARNING_SYNC_TIMESTAMP, MAX_LEARNING_SYNC_WEIGHT,
};
pub use stream::{
    read_learning_sync, write_learning_sync, LearningSyncRead, LearningSyncReadError,
};

#[cfg(test)]
mod tests;
