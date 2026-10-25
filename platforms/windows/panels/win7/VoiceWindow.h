#pragma once

#include <windows.h>
#include <cstdint>
#include <functional>
#include <memory>
#include <string>
#include <string_view>

namespace msime::windows::win7 {
enum VoicePanelControl {
  voice_panel_start = 500, voice_panel_stop, voice_panel_cancel, voice_panel_result,
  voice_panel_level, voice_panel_status, voice_panel_copy, voice_panel_insert, voice_panel_settings,
};
// 只在会话协议边界注入合成识别；实际 HWND、配置、线程和统计仍运行。
struct VoiceSessionApi {
  using Progress = std::function<void(std::string_view)>;
  std::function<std::string()> create;
  std::function<std::string(uint64_t, unsigned)> control;
  std::function<std::string(uint64_t, const std::string&, const Progress&)> recognize;
};
class VoiceWindow final {
public:
  using InsertText = std::function<bool(std::wstring_view)>;
  using CopyText = std::function<bool(HWND, std::wstring_view)>;
  using OpenSettings = std::function<bool()>;
  explicit VoiceWindow(InsertText insert = {}, CopyText copy = {}, VoiceSessionApi api = {}, OpenSettings settings = {});
  ~VoiceWindow();
  VoiceWindow(const VoiceWindow&) = delete;
  VoiceWindow& operator=(const VoiceWindow&) = delete;
  bool create(HINSTANCE instance);
  int run(int show);
  HWND handle() const;
private:
  struct State;
  std::unique_ptr<State> state_;
};
}
