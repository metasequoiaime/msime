//! 清空前在偏好写锁内确认开关，避免迟到的关闭快照删除重新开启后的记录。

use super::{PreferencesError, PreferencesStore};
use std::path::Path;

impl PreferencesStore {
    /// 宿主指定自己的历史文件；偏好目录可与移动端主模式、访客模式的历史目录分开。
    /// 先锁偏好再锁历史，与保存偏好及桌面捕获保持相同顺序。
    /// 返回是否执行了清空；偏好不可读时返回错误并保留历史。
    pub fn clear_disabled_clipboard_history_at(
        &self,
        history_path: &Path,
    ) -> Result<bool, PreferencesError> {
        let _lock = self.lock()?;
        if self.read_locked()?.preferences.clipboard_history {
            return Ok(false);
        }
        crate::clipboard::ClipboardHistoryStore::open(history_path).clear()?;
        Ok(true)
    }
}
