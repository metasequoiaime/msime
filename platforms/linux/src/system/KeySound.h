#pragma once

#include <cstdint>
#include <string>

namespace msime::linux_host {

// 一次按下的按键类别：1 空格，2 回车，3 退格，0 其它键。两个宿主经 lifecycle_key（LifecycleFeedback.h）把它交给 msime_client_key，位布局同 msime_client_key_sound 的 key_class。IBus 和 Fcitx5 报的都是 X11 keysym，一张表两个宿主共用。
constexpr std::uint32_t key_sound_class(std::uint32_t keysym) {
  switch (keysym) {
  case 0x0020: // space
  case 0xff80: // KP_Space
    return 1;
  case 0xff0d: // Return
  case 0xff8d: // KP_Enter
    return 2;
  case 0xff08: // BackSpace
    return 3;
  default:
    return 0;
  }
}

// 一次按下算不算打字，也就是宿主要不要为它调 msime_client_key（出按键音、开新的一轮）。松开、单独的修饰键和快捷键（按着 Control、Alt、Super、Hyper 或 Meta 的键）不算：声音是给打字的，应用处理的组合键不是打字。Shift 和第三层 Shift 只决定这个键打出哪个字符，不让它变成快捷键。不算打字的按下暂时不开新的一轮，见 msime_client.h 顺序契约里的过渡期例外。
constexpr bool key_press_sounds(bool release, bool modifier_key, bool shortcut) {
  return !release && !modifier_key && !shortcut;
}

// msime_client_typing_effect's events beyond the key classes above, and its flag for an auto-repeated key, which is drawn but not counted (msime_client.h).
constexpr std::uint32_t kTypingEffectCommit = 4;
constexpr std::uint32_t kTypingEffectRepeat = 0x100;

// The combo count in a msime_client_typing_effect answer: its low 16 bits, zero while the combo counter is off or the call was refused.
constexpr std::uint32_t typing_effect_combo(std::uint32_t answer) { return answer & 0xffffu; }

// The combo as the Linux hosts show it, as one more segment of the candidate aux line; empty below two, as on the other hosts, so a single key is not called a combo. Linux draws no flash or sparks: neither IBus nor Fcitx5 gives an input method a reliable place on screen for an overlay under Wayland, so the count in the text the panel already shows is the whole effect.
inline std::string typing_combo_label(std::uint32_t combo) {
  return combo < 2 ? std::string{} : "连击 ×" + std::to_string(combo);
}

// 一次按下是不是还按着的那个键的自动重复。IBus 和 Fcitx5 都不告诉输入法按下是不是重复，但都把松开交给它：按住的键以同一个物理键的多次按下到来、中间没有松开（Wayland，以及开了可检测自动重复的 X11 客户端，GDK 会打开它），那就是重复；X11 在每次重复前补一个假松开时，重复算作新的按下。按硬件键码认键，不按 keysym：先松开 Shift 再松开 1 时，松开事件带的是 '1' 而按下时是 '!'，按 keysym 记就清不掉，下一次 Shift+1 会被当成重复。键码 0 没有物理键在后面（面板或屏幕键盘合成的键），这样的按下从不算重复。失焦丢掉的松开最多让那个键的下一次按下认错，失焦时 reset() 连这个也避免了。
class KeyRepeat {
public:
  bool press(std::uint32_t keycode) {
    const bool repeat = keycode != 0 && keycode == held_;
    held_ = keycode;
    return repeat;
  }
  void release(std::uint32_t keycode) {
    if (keycode == held_) held_ = 0;
  }
  void reset() { held_ = 0; }

private:
  std::uint32_t held_ = 0;
};

// 宿主这一侧对 msime_client_set_active 的记账：只在激活状态变了时才告诉生命周期核心，调用被接受了才记作已告诉。核心对选择加入的会话总是接受，所以"被拒就下次再说"的重试已经不会发生；进程里什么声音都没开时核心不起播放器，激活那一刻的音乐请求没人收到，设置里的声音（preferences 的 plugins）变了时宿主要在本会话激活着时再声明一次 set_active(true)，不能指望这里重试。
class MusicActivity {
public:
  // 告诉核心本会话现在是否激活：输入法获得焦点、没有在录音。字段是不是 SECURE 由核心按字段属性判断。没有会话时什么都不发。
  template <class Call> void sync(std::uint64_t session, bool active, Call &&call) {
    if (session == 0 || active == told_) return;
    if (call(session, active)) told_ = active;
  }
  // 会话销毁之前调用：核心还以为本会话激活着的话，所有者被销毁时虽然也会关掉音乐，这里仍先明确说一声停用；下一个会话从未激活开始。
  template <class Call> void release(std::uint64_t session, Call &&call) {
    if (session != 0 && told_) call(session, false);
    told_ = false;
  }
  bool told() const { return told_; }

private:
  bool told_ = false;
};

} // namespace msime::linux_host
