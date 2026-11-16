//! 输入习惯同步的账号进度。密钥驱动在 client-core，学习数据合并在 Engine。
//!
//! 账号请求和密钥持久化在这里接入；同步轮次仍须完整校验信封和明文后才能持久化接纳进度。

mod http;
mod session;
pub mod state;
pub mod transport;
pub mod upload;

pub use session::AccountLearningError;
