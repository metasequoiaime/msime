//! Integrations shared by the three desktop hosts.

pub(crate) mod desktop_data_directory;

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_account;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_candidate_skin_community;
pub(crate) mod desktop_preferences_monitor;
