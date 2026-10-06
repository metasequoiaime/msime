//! Host-independent helpers for the desktop shell.

pub(crate) mod account_dto;
pub(crate) mod ai_url;
pub(crate) mod atomic_file;
pub(crate) mod bounded_body;
pub(crate) mod export_file;
// 只有 iOS 用它发请求；测试在每个平台都编译它，那里没有调用方。
#[cfg(any(target_os = "ios", test))]
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(crate) mod mobile_ai;
#[cfg(any(target_os = "linux", test))]
pub(crate) mod omarchy_skin;
pub(crate) mod skin_directory;
pub(crate) mod voice;
