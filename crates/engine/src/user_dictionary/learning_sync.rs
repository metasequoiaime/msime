//! 输入习惯同步的明文记录与收敛合并，契约见 `docs/learning-sync.md`。
//!
//! 这里不联网、不解密，也不把记录写进学习日志。调用方必须先完成信封认证，再完整读取并校验明文流；只有整份流成功后才能接纳它的状态和时间上限。

mod records;
mod stream;

pub use records::{LearningSyncRecord, LearningSyncState, MAX_LEARNING_SYNC_TIMESTAMP};
pub use stream::{
    read_learning_sync, write_learning_sync, LearningSyncRead, LearningSyncReadError,
};

#[cfg(test)]
mod tests;
