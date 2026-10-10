#pragma once
#include <cstddef>
#include <cstdint>
#include <cstring>

namespace msime::windows {
// 密码管理器和 Windows 自带的剪贴板历史在复制敏感内容时附带的注册格式名。Windows 上的约定：带 ExcludeClipboardContentFromMonitorProcessing 或 Clipboard Viewer Ignore 的内容不该被任何监视程序处理；CanIncludeInClipboardHistory、CanUploadToCloudClipboard 是 DWORD，值为 0 时不进历史、不上云。KeePass、1Password、Bitwarden 都按这套约定标记。设置应用侧（crates/host-windows/src/clipboard_privacy.rs）有同一份名单，两处必须一致；macOS 对应的是 BackendClipboardCapture.swift 的 excludedTypes。
inline constexpr wchar_t clipboard_exclude_monitor_format[] = L"ExcludeClipboardContentFromMonitorProcessing";
inline constexpr wchar_t clipboard_viewer_ignore_format[] = L"Clipboard Viewer Ignore";
inline constexpr wchar_t clipboard_history_permission_format[] = L"CanIncludeInClipboardHistory";
inline constexpr wchar_t clipboard_cloud_permission_format[] = L"CanUploadToCloudClipboard";

// 一个 DWORD 许可格式的状态。剪贴板上没有这个格式时是 absent。
enum class ClipboardPermission { absent, allowed, denied };

// 剪贴板上出现了许可格式，data 和 bytes 是它的内容（读不到时传 nullptr）。读不到或不足一个 DWORD 时按拒绝处理：写下这个格式的程序显然在表态，宁可漏记一条也不把可能的密码写进历史。
inline ClipboardPermission clipboard_permission_from_data(const void *data, std::size_t bytes) {
  if (!data || bytes < sizeof(std::uint32_t))
    return ClipboardPermission::denied;
  std::uint32_t value = 0;
  std::memcpy(&value, data, sizeof(value));
  return value == 0 ? ClipboardPermission::denied : ClipboardPermission::allowed;
}

// 一次剪贴板变化上观察到的隐私标记。
struct ClipboardPrivacyMarkers {
  bool exclude_from_monitor = false;
  bool viewer_ignore = false;
  ClipboardPermission history = ClipboardPermission::absent;
  ClipboardPermission cloud = ClipboardPermission::absent;
};

// 任一标记表示不该被记录时返回 true，调用方丢弃这次样本，并把这次变化记为已处理，不再重试。
inline bool clipboard_sample_excluded(const ClipboardPrivacyMarkers &markers) {
  return markers.exclude_from_monitor || markers.viewer_ignore ||
         markers.history == ClipboardPermission::denied ||
         markers.cloud == ClipboardPermission::denied;
}
} // namespace msime::windows
