#pragma once
// 由共享 Rust 按住录音目录导出；通过 export_voice_hold_key_catalog 更新。
#include <array>
#include <string_view>

namespace msime::voice {
struct VoiceHoldKeyCatalogEntry {
  std::wstring_view key;
  std::wstring_view label;
  bool fallback;
};
inline constexpr std::array<VoiceHoldKeyCatalogEntry, 3> kVoiceHoldKeys = {{
  {L"hotkey_ralt", L"右 Alt", true},
  {L"hotkey_rctrl_ralt", L"右 Ctrl+右 Alt", false},
  {L"hotkey_ctrl_win", L"Ctrl+Win", false},
}};
} // 命名空间 msime::voice
