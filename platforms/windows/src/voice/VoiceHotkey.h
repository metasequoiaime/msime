#pragma once

#include "VoiceHotkeyPolicy.h"
#include "VoiceInputSession.h"

#include <windows.h>

#include <atomic>
#include <functional>

namespace msime::windows {
class VoiceHotkeyController final {
public:
  using ConfigProvider = std::function<VoiceInputConfig()>;
  using ActiveProvider = std::function<bool()>;

  VoiceHotkeyController(VoiceInputSession &voice, ConfigProvider config,
                        ActiveProvider active);
  ~VoiceHotkeyController();
  VoiceHotkeyController(const VoiceHotkeyController &) = delete;
  VoiceHotkeyController &operator=(const VoiceHotkeyController &) = delete;

  // Apply hotkey enable/shortcut changes published while the Server stays
  // alive. This mirrors the native service's RefreshKeyboardHook path and is
  // called from the Server control loop, never from the low-level hook.
  void refresh();

private:
  static LRESULT CALLBACK window_proc(HWND, UINT, WPARAM, LPARAM);
  static LRESULT CALLBACK keyboard_proc(int, WPARAM, LPARAM);
  LRESULT handle_window(HWND, UINT, WPARAM, LPARAM);
  void activate(VoiceHoldShortcut);
  void reset_state();
  VoiceModifierState modifiers() const;

  VoiceInputSession &voice_;
  ConfigProvider config_provider_;
  ActiveProvider active_provider_;
  HWND window_ = nullptr;
  HHOOK hook_ = nullptr;
  std::atomic<bool> ralt_pressed_{false};
  std::atomic<bool> lctrl_pressed_{false};
  std::atomic<bool> rctrl_pressed_{false};
  std::atomic<bool> lwin_pressed_{false};
  std::atomic<bool> rwin_pressed_{false};
  // 已消费的切换按键直到松开才释放，改绑、禁用或失焦不能把重复键交回应用。
  VoiceToggleStrokes toggle_strokes_;
  std::atomic<bool> cancel_posted_{false};
  std::atomic<bool> suppress_ralt_until_up_{false};
  std::atomic<bool> suppress_win_until_up_{false};
  std::atomic<VoiceHoldShortcut> active_hold_{VoiceHoldShortcut::None};
  bool observed_config_ = false;
  bool observed_enabled_ = true;
  bool observed_hotkey_ralt_ = true;
  bool observed_hotkey_ctrl_f9_ = true;
  bool observed_hotkey_ctrl_win_ = false;
  bool observed_hotkey_rctrl_ralt_ = false;
  bool observed_hotkey_hold_space_lock_ = true;
  static VoiceHotkeyController *instance_;
};
} // namespace msime::windows
