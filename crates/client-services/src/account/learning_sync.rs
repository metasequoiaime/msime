//! 输入习惯同步的账号进度。密钥驱动在 client-core，学习数据合并在 Engine。
//!
//! 账号请求、密钥持久化、显式轮次和密文待合并在这里接入；宿主入口和自动调度仍待接线。

mod http;
mod pending;
pub mod round;
mod session;
pub mod state;
pub mod transport;
pub mod upload;

pub use pending::PendingError;
pub use session::AccountLearningError;
