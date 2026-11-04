//! Host integrations that exist on some targets only.
//!
//! Each child mirrors a directory under `src/platform/` and is gated on the targets it speaks to (one OS, or the `desktop` family), so the crate root can name a module without repeating the platform condition at every use site.
//!
//! 带 `test` 的模块在其他平台上也会编译，只为让它们的单元测试在每个平台都能跑；那里没有调用方，所以在本平台以外放开 `dead_code`。

pub(crate) mod account_helpers;
pub(crate) mod cloud_clipboard;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos", test))]
#[cfg_attr(not(any(target_os = "windows", target_os = "linux")), allow(dead_code))]
pub(crate) mod cloud_dictionary;
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) mod community_resources;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos", test))]
pub(crate) mod desktop;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
#[cfg_attr(not(any(target_os = "linux", target_os = "macos")), allow(dead_code))]
pub(crate) mod dictionary_sync;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod macos;
#[cfg(windows)]
pub(crate) mod windows;
