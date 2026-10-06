//! Integrations shared by the three desktop hosts.

pub(crate) mod desktop_data_directory;

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_account;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_candidate_skin_community;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_community_report;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_plugin_community;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_plugins;
pub(crate) mod desktop_preferences_monitor;
pub(crate) mod desktop_resource_packs;
