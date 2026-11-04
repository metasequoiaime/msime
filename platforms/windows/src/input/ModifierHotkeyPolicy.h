#pragma once

#include <cstdint>

namespace msime::windows {

// RDP 切换后系统可能残留远程会话释放的修饰键状态；这些位在本地再次观察到
// 对应按键前不能参与单按判定，否则单按 Shift 会被永久判成组合键。
constexpr std::uint32_t kModifierHotkeyShift = 1u << 0;
constexpr std::uint32_t kModifierHotkeyControl = 1u << 1;
constexpr std::uint32_t kModifierHotkeyAlt = 1u << 2;
constexpr std::uint32_t kModifierHotkeyWin = 1u << 3;

constexpr bool modifier_hotkey_allows_bare(
    std::uint32_t physical_down, std::uint32_t stale_after_focus_loss,
    std::uint32_t kept_modifier) {
  const auto relevant = physical_down & ~stale_after_focus_loss;
  return (relevant & kept_modifier) != 0 &&
         (relevant & ~kept_modifier) == 0;
}

}  // 命名空间 msime::windows
