#pragma once
#include "../../../../shared/contracts/msime_edition.h"
#include <filesystem>
#include <string>
#include <vector>
#include <functional>
#include <atomic>
#ifdef _WIN32
#include <windows.h>
#endif

namespace msime::windows {
class ClipboardHistory final {
public:
  static constexpr size_t max_items = 50;
  static constexpr size_t max_chars = 4000;
  explicit ClipboardHistory(std::filesystem::path store);
  std::vector<std::string> load() const;
  bool add(std::string text);
  bool remove(const std::string &text);
  bool clear();
  // Preference notifications may be stale. Shared settings writers own
  // deletion under the preferences/history locks; this is only a local gate.
  void set_enabled(bool enabled) { enabled_.store(enabled); }
  bool enabled() const { return enabled_.load(); }
private:
  std::filesystem::path store_;
  std::atomic<bool> enabled_{true};
};
std::string normalize_clipboard_text(std::string text);

#ifdef _WIN32
// 带版本后缀：设置应用（crates/host-windows 的 `wait_for_clipboard_history_change`）按同一规则拼名字，只等自己版本的 Server。
inline constexpr wchar_t clipboard_history_change_event_name[] =
    L"Local\\MSIME.Client.ClipboardHistoryChanged" MSIME_EDITION_NAME_SUFFIX;

class ClipboardMonitor final {
public:
  using Callback = std::function<void(std::string)>;
  ClipboardMonitor(ClipboardHistory &history, Callback callback);
  ~ClipboardMonitor();
  ClipboardMonitor(const ClipboardMonitor &) = delete;
  bool start();
  void stop();
private:
  static LRESULT CALLBACK window_proc(HWND, UINT, WPARAM, LPARAM);
  ClipboardHistory &history_;
  Callback callback_;
  void *window_ = nullptr;
  HANDLE change_event_ = nullptr;
  unsigned long sequence_ = 0;
};
#endif
} // namespace msime::windows
