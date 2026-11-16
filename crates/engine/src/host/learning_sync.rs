//! 输入习惯同步的宿主路径适配；调用方负责维护租约与账号同步状态。

use super::options::{runtime_paths, EngineOptions};
use crate::error::Result;
use crate::user_dictionary::journal::release_thread_journal;
use crate::user_dictionary::learning_sync::{
    self, LearningSyncExport, LearningSyncMerge, LearningSyncRecord, LearningSyncState,
};

/// 账号屏障持久化后，在同一独占维护边界内推进日志时间下限。
pub fn advance_learning_sync_clock(options: &EngineOptions, floor: u64) -> Result<()> {
    let result = learning_sync::advance_learning_sync_clock(&runtime_paths(options), floor);
    release_thread_journal();
    result
}

/// 本地备份恢复和云端快照换代均保留本机时间下限，不取快照中的下限。
pub fn carry_learning_sync_clock(active: &EngineOptions, staged: &EngineOptions) -> Result<()> {
    let result =
        learning_sync::carry_learning_sync_clock(&runtime_paths(active), &runtime_paths(staged));
    release_thread_journal();
    result
}

/// 换代发布前原样保留本机调权、删除和完整固定候选；调用方持有独占维护权。
pub fn carry_learning_sync_state(active: &EngineOptions, staged: &EngineOptions) -> Result<usize> {
    let result = learning_sync::carry_learning_sync_state(
        &runtime_paths(active),
        &runtime_paths(staged),
        staged.enabled_schemes.reads_main_dictionary(),
    );
    release_thread_journal();
    result
}

/// 导出只读取一致快照；发布前须持久化返回的观察时间，并完成整份流编码。
pub fn stream_learning_sync_state(
    options: &EngineOptions,
    emit: &mut dyn FnMut(&LearningSyncRecord) -> bool,
) -> Result<LearningSyncExport> {
    let result = learning_sync::stream_learning_sync_state(&runtime_paths(options), emit);
    release_thread_journal();
    result
}

/// 调用方持有独占维护权，已持久化观察时间，并在应用时读取账号最新的重置屏障。
pub fn merge_learning_sync_state(
    options: &EngineOptions,
    remote: &LearningSyncState,
    reset_floor: u64,
) -> Result<LearningSyncMerge> {
    let result = learning_sync::merge_learning_sync_state(
        &runtime_paths(options),
        options.enabled_schemes.reads_main_dictionary(),
        remote,
        reset_floor,
    );
    release_thread_journal();
    result
}
