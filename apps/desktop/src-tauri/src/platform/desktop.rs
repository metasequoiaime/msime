//! Integrations shared by the three desktop hosts.

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) mod desktop_data_directory;

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_account;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_candidate_skin_community;
// Windows 和没有原生会话的 macOS 设置进程复用账号会话服务云词库。
#[cfg(any(target_os = "windows", target_os = "macos", test))]
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
pub(crate) mod desktop_cloud_dictionary;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_community_report;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_plugin_community;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_plugins;
pub(crate) mod desktop_preferences_monitor;
pub(crate) mod desktop_resource_packs;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_typing_upload;
