//! 两种重置入口共享清除与集合重排；调用方在整个过程持有 Engine 独占维护权。

use super::*;
use crate::EngineOptions;

pub(crate) fn under_maintenance(
    options: &EngineOptions,
    preferences_directory: Option<&Path>,
) -> Result<serde_json::Value, String> {
    // 本地备份恢复排下、还没合并的输入记录和输入习惯先删掉，否则键盘空闲时会把清除前的学习并回来。放在重置之前：删不掉时什么也还没清，报错重试即可。
    if let Some(directory) = preferences_directory {
        crate::dictionary_snapshot::discard_pending_learning(directory)
            .map_err(|_| "learned-data reset rejected")?;
    }
    msime_engine::host::reset_learned_data(options).map_err(|_| "learned-data reset rejected")?;
    // 命名词库的词经个人词库队列写进用户词库，重置把它们一起清掉了，集合却还写着启用。还拿着维护租约时把已启用集合的词重新排进待发送队列，宿主随后照常送出（Android 设置页紧接着 `synchronize`）。未提供偏好目录的宿主无法定位命名词库，不排。重置已经做完，这一步失败时不报整个请求失败（宿主会以为什么都没清、不打云同步标记），而是照样回答 `reset:true`，`collections_requeued` 为 null 并带上 `collections_error`，由宿主告诉用户已添加词库的词没能写回。
    let requeued = match preferences_directory {
        Some(directory) => {
            msime_client_services::dictionary::collections::DictionaryCollectionsStore::new(
                directory,
                PersonalDictionaryStore::new(directory.join("PersonalDictionary")),
            )
            .requeue_enabled_after_reset()
            .map(|view| view.queued.unwrap_or(0))
        }
        None => Ok(0),
    };
    Ok(match requeued {
        Ok(requeued) => json!({ "reset": true, "collections_requeued": requeued }),
        Err(_) => json!({
            "reset": true,
            "collections_requeued": null,
            "collections_error": "learned-data reset collections unavailable",
        }),
    })
}
