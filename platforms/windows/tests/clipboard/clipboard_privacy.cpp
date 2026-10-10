#include "ClipboardPrivacyPolicy.h"

#include <cstdint>
#include <cstdio>
#include <cwchar>

namespace {
int failures = 0;

void require(bool value, const char *what) {
  if (!value) {
    std::fprintf(stderr, "clipboard privacy: %s\n", what);
    ++failures;
  }
}
} // namespace

int main() {
  using namespace msime::windows;
  // 格式名是跨进程约定，拼错一个字母就等于没有检查；设置应用侧的同一份名单由 crates/host-windows/tests/clipboard_privacy_policy.rs 对照本头文件。
  require(std::wcscmp(clipboard_exclude_monitor_format, L"ExcludeClipboardContentFromMonitorProcessing") == 0,
          "monitor exclusion format name");
  require(std::wcscmp(clipboard_viewer_ignore_format, L"Clipboard Viewer Ignore") == 0, "viewer ignore format name");
  require(std::wcscmp(clipboard_history_permission_format, L"CanIncludeInClipboardHistory") == 0,
          "history permission format name");
  require(std::wcscmp(clipboard_cloud_permission_format, L"CanUploadToCloudClipboard") == 0,
          "cloud permission format name");

  const std::uint32_t zero = 0;
  const std::uint32_t one = 1;
  require(clipboard_permission_from_data(&zero, sizeof(zero)) == ClipboardPermission::denied, "DWORD 0 denies");
  require(clipboard_permission_from_data(&one, sizeof(one)) == ClipboardPermission::allowed, "DWORD 1 allows");
  require(clipboard_permission_from_data(nullptr, 0) == ClipboardPermission::denied, "unreadable permission denies");
  require(clipboard_permission_from_data(&one, 2) == ClipboardPermission::denied, "short permission denies");

  require(!clipboard_sample_excluded({}), "ordinary text is captured");
  ClipboardPrivacyMarkers markers;
  markers.exclude_from_monitor = true;
  require(clipboard_sample_excluded(markers), "monitor exclusion skips the sample");
  markers = {};
  markers.viewer_ignore = true;
  require(clipboard_sample_excluded(markers), "viewer ignore skips the sample");
  markers = {};
  markers.history = ClipboardPermission::denied;
  require(clipboard_sample_excluded(markers), "history permission 0 skips the sample");
  markers = {};
  markers.cloud = ClipboardPermission::denied;
  require(clipboard_sample_excluded(markers), "cloud permission 0 skips the sample");
  markers = {};
  markers.history = ClipboardPermission::allowed;
  markers.cloud = ClipboardPermission::allowed;
  require(!clipboard_sample_excluded(markers), "explicit permissions keep the sample");

  if (failures == 0)
    std::puts("clipboard privacy policy passed");
  return failures == 0 ? 0 : 1;
}
