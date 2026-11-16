#pragma once

#include <cstdint>

namespace msime::input {

// 「单按 Caps Lock 切换中英文」里轻点与长按的分界，三个桌面宿主共用：从按下（macOS 是大写锁定打开的那一刻）到松开不超过这个时长才算轻点。300 毫秒比单按 Shift、Control 的 500 毫秒短，因为长按 Caps Lock 是在要大写锁定，那一侧更该早点认定。
inline constexpr std::int64_t kCapsLockTapThresholdMs = 300;

// `GetKeyState(VK_CAPITAL)` 的低位就是切换后的大写锁定状态；高位只是当前按键是否按下。
constexpr bool caps_lock_enabled_from_key_state(short keyState) {
  return (keyState & 0x0001) != 0;
}

// 判定一次 Caps Lock 按键是不是轻点：开关打开、按下前大写锁定是关的、按下时没有别的键或修饰键按着、按着期间没有按别的键、并且在分界内松开。
//
// 时间由宿主给出（毫秒，同一个单调时钟），这里不读时钟也不碰系统的锁定状态，宿主的单测可以直接驱动它。认定为轻点之后，宿主负责切换中英文并把大写锁定撤回；其余情况一律照常保留大写锁定。
class CapsLockTap {
public:
  // Caps Lock 按下。lockWasOn 为真表示这一下是在关掉已经开着的大写锁定，不算轻点；otherKeysHeld 为真表示它是组合键的一部分。
  void press(std::int64_t nowMs, bool enabled, bool lockWasOn, bool otherKeysHeld) {
    armed_ = enabled && !lockWasOn && !otherKeysHeld;
    pressedAtMs_ = nowMs;
  }

  // 按着 Caps Lock 时按了别的键：这是在用大写锁定打字，不再算轻点。
  void interrupt() { armed_ = false; }

  // Caps Lock 松开。返回 true 表示这是一次轻点。
  bool release(std::int64_t nowMs) {
    const bool tap = armed_ && nowMs >= pressedAtMs_ &&
                     nowMs - pressedAtMs_ <= kCapsLockTapThresholdMs;
    armed_ = false;
    return tap;
  }

  // 还按着，但已经超过分界：可以认定为长按。macOS 收不到 Caps Lock 的松开事件，靠轮询按键状态，用它决定什么时候停。
  bool heldTooLong(std::int64_t nowMs) const {
    return armed_ && nowMs - pressedAtMs_ > kCapsLockTapThresholdMs;
  }

  bool armed() const { return armed_; }

  void reset() {
    armed_ = false;
    pressedAtMs_ = 0;
  }

private:
  std::int64_t pressedAtMs_ = 0;
  bool armed_ = false;
};

} // namespace msime::input
