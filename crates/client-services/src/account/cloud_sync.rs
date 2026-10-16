//! 多分类自动云同步（设置、皮肤库、无编码常用语、个人词库）的编排：游标、待上传标记、合并策略和提交，原来在 iOS 的 `CloudSync` 加 `BackendSyncPolicy` 和 Android 的 `home/CloudSync` 加 `SyncMergePolicy`、`SyncSwitch` 里各有一份。
//!
//! 这里只有不碰网络的部分：纯决策（[`policy`]）、跨进程的状态文件（[`state`]）和单轮状态机（[`machine`]）。联网和本机写入的驱动要用 Engine 导出和激活词库，放在 host-api 的 `cloud_sync`，C 入口是 `msime_client_cloud_sync`。

pub mod machine;
pub mod policy;
pub mod state;
