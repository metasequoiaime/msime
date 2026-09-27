//! Integrations shared by the three desktop hosts.

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_account;
pub(crate) mod desktop_preferences_monitor;
