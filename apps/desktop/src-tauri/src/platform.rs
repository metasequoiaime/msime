//! Host integrations that exist on some targets only.
//!
//! Each child mirrors a directory under `src/platform/` and is gated on the targets it speaks to (one OS, or a family such as `desktop` and `mobile`), so the crate root can name a module without repeating the platform condition at every use site.

pub(crate) mod account_helpers;
#[cfg(any(target_os = "android", test))]
pub(crate) mod android;
pub(crate) mod cloud_clipboard;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos", test))]
pub(crate) mod desktop;
#[cfg(any(target_os = "ios", test))]
pub(crate) mod ios;
#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(any(target_os = "macos", test))]
pub(crate) mod macos;
#[cfg(any(target_os = "ios", target_os = "android", test))]
pub(crate) mod mobile;
#[cfg(windows)]
pub(crate) mod windows;
