//! 输入习惯同步的账号进度。密钥驱动在 client-core，学习数据合并在 Engine。
//!
//! 账号请求、密钥持久化和显式轮次在这里接入；宿主入口、自动调度和持久待合并仍待接线。

mod http;
pub mod round;
mod session;
pub mod state;
pub mod transport;
pub mod upload;

pub use session::AccountLearningError;
