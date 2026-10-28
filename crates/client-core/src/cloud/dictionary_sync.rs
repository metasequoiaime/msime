//! 「我的词条」经用户自己的 WebDAV 网盘在设备之间同步的协议核心。不碰网络、不碰 Engine：网盘、本机词库、时钟和写盘都经接口注入（[`RemoteStore`]、[`LocalDictionary`]、[`Environment`]、[`StateSink`]）。存储布局、合并规则、上限和请求量是契约，见 `docs/dictionary-webdav-sync.md`；设计取舍见 `.agents/notes/` 下的决策笔记。
//!
//! 同步的是用户在「我的词条」里看到的那些词（Engine 日志里 `user_inserted` 的词条，包括打字时自动造的词）以及它们的删除，身份是（词库类别、编码、词）；打字学到的权重、固定位置和选择次数，主词库、模型、皮肤、剪贴板、输入记录和凭据都不在里面。
//!
//! - [`clock`]：混合逻辑时钟，每条操作带一个时间戳，合并结果与到达顺序无关；
//! - [`entry`]：词条身份的规范化；
//! - [`replica`]：最后写入者胜的词条寄存器、设备表和墓碑回收；
//! - [`crypto`]：事件和检查点的加密，密钥来自词库自己的密钥串；
//! - [`log`]：网盘上的文件格式、哈希链和日志位置；
//! - [`sync`]：一次同步的步骤和本机状态；
//! - [`schedule`]：什么时候联网、额度多少。

pub mod clock;
pub mod crypto;
pub mod entry;
pub mod log;
pub mod replica;
pub mod schedule;
pub mod sync;

pub use clock::{Clock, Hlc};
pub use crypto::DataKey;
pub use entry::{WordKey, WordKind};
pub use schedule::{
    drive, execute, plan, record_failure, record_requests, Charge, Outcome, Plan, Trigger,
};
pub use sync::{
    capture, restart, run, Environment, KeyRing, LocalChange, LocalDictionary, LocalError,
    LocalOutcome, Options, Parts, RemoteStore, Report, StateSink, Status, SyncError, SyncState,
};

#[cfg(test)]
#[path = "dictionary_sync/simulation_tests.rs"]
mod simulation_tests;
