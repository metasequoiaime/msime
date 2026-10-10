#pragma once
#include <cstdint>
#include <optional>

namespace msime::windows {
// msime_client_key_sound 的按键类别：1 空格、2 回车、3 退格、0 其他键。单独的修饰键（Shift、Ctrl、Alt、Caps Lock、Windows 键）和 0 不是一次打字，返回空。Server 处理的键（KeySoundPolicy.h）和 TIP 交给应用的键（passthrough_key_sound_class）共用这一张表，两条路出声的键才一致。
inline std::optional<uint32_t> key_sound_class_for_virtual_key(uint32_t virtual_key) {
  switch (virtual_key) {
  case 0:
  case 0x10: // Shift 键
  case 0x11: // Ctrl 键
  case 0x12: // Alt 键
  case 0x14: // 大写锁定
  case 0x5B: // 左 Windows 键
  case 0x5C: // 右 Windows 键
  case 0xA0:
  case 0xA1:
  case 0xA2:
  case 0xA3:
  case 0xA4:
  case 0xA5:
    return std::nullopt;
  case 0x20:
    return 1u;
  case 0x0D:
    return 2u;
  case 0x08:
    return 3u;
  default:
    return 0u;
  }
}

// TIP 交给应用的一次按下，按键音和打字特效要知道的状态。
struct PassthroughKeyState {
  uint32_t virtual_key = 0;
  // lParam 第 30 位：按住不放的自动重复。
  bool auto_repeat = false;
  bool ctrl = false;
  bool alt = false;
  bool win = false;
  // 键盘开着（中文模式）。英文模式在每个桌面宿主上都不出声，和 macOS 一样。
  bool keyboard_open = false;
  // 上下文停用了输入法（经典 Edit 的密码框之类）。
  bool keyboard_disabled = false;
  // TSF 的安全模式（登录界面、UAC 这类安全桌面）。
  bool secure = false;
  // 设置应用的面板或 Server 语音用 SendInput 注入的文字，不是用户按的键。
  bool injected = false;
};

// TIP 交给应用的键要不要出按键音、计入连击，要的话是哪一类。和 macOS 的 playKeySound 一致：自动重复只算一次按下，Ctrl、Alt、Windows 组合键是快捷键，英文模式、停用的键盘和安全模式都不出声（按键音会泄露密码的节奏）。Shift 照样算打字。
inline std::optional<uint32_t> passthrough_key_sound_class(const PassthroughKeyState &key) {
  if (key.auto_repeat || key.ctrl || key.alt || key.win || !key.keyboard_open || key.keyboard_disabled || key.secure ||
      key.injected)
    return std::nullopt;
  return key_sound_class_for_virtual_key(key.virtual_key);
}
} // namespace msime::windows
