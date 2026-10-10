//! Integrations shared by the three desktop hosts.

pub(crate) mod desktop_data_directory;

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_account;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_candidate_skin_community;
// 只有 Windows 由设置应用自己服务云词库；测试在每个平台都编译它，那里没有调用方。
#[cfg(any(target_os = "windows", test))]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) mod desktop_cloud_dictionary;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_community_report;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_plugin_community;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
pub(crate) mod desktop_plugins;
pub(crate) mod desktop_preferences_monitor;
pub(crate) mod desktop_resource_packs;
