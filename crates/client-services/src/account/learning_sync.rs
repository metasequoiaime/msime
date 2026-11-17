//! 输入习惯同步的账号进度。密钥驱动在 client-core，学习数据合并在 Engine。
//!
//! 账号请求、密钥、轮次、密文待合并和持久调度在这里接入；宿主定时器与原生入口仍待接线。

mod http;
mod pending;
mod reset;
pub mod round;
pub mod schedule;
mod session;
pub mod state;
pub mod transport;
pub mod upload;

pub use pending::PendingError;
pub use session::AccountLearningError;
