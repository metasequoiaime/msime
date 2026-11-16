//! 输入习惯同步的宿主路径适配；调用方负责维护租约与账号同步状态。

use super::options::{runtime_paths, EngineOptions};
use crate::error::Result;
use crate::user_dictionary::journal::release_thread_journal;
use crate::user_dictionary::learning_sync::{
    self, LearningSyncExport, LearningSyncMerge, LearningSyncRecord, LearningSyncState,
};

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
