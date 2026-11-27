#pragma once

#include <linux/input-event-codes.h>

#include <vector>

namespace msime::linux_host {

// 成对标点补全后要把光标退回两半之间。IBus 没有直接设置文档光标的接口，只能转发方向键。Wayland 下虚拟键盘沿用物理修饰状态，Shift 还按着时转发方向键会被应用当成 Shift+方向键去选中文字，所以方向键要排队到松开 Shift 之后，与 Fcitx5 宿主一致；纯 X11 会话下直接转发，不引入排队带来的延迟。
struct PairedCaretForward {
  bool wayland_session = false;
  bool shift_held = false;

  constexpr bool defer() const { return wayland_session && shift_held; }
};

// 转发方向键用的扫描码。IBus 的 keycode 是 evdev 扫描码，客户端自己再加 8 换成 XKB keycode（`process_key` 收到的也是 evdev 码，见 TypingStatistics.h），所以直接用 `KEY_LEFT`（105）、`KEY_RIGHT`（106）；不再从 IBus 的 us 键位表查，那个文件缺失时会回落到 0，客户端不认，正是这里要修的原始问题。
constexpr unsigned paired_caret_keycode(bool leftward) { return leftward ? KEY_LEFT : KEY_RIGHT; }

// 排队的方向键只对当前焦点有效：换焦点、重置或松开 Shift 时必须清空或取走，不能漏到下一个输入框。
template <typename Key>
class PairedCaretQueue {
 public:
  void push(Key key) { queued_.push_back(key); }
  std::vector<Key> take() {
    std::vector<Key> taken;
    taken.swap(queued_);
    return taken;
  }
  void clear() { queued_.clear(); }
  bool empty() const { return queued_.empty(); }
  // 排队方向键对逻辑光标的净移动，左移为负。读文档里光标后面的字时要把它算上：Shift 按着连打 `(` `)` 时左移还没发出，文档光标仍在右半之后，不算上就认不出要跨过的右半，会再补一个。
  long net_offset(Key left) const {
    long offset = 0;
    for (const auto key : queued_) offset += key == left ? -1 : 1;
    return offset;
  }

 private:
  std::vector<Key> queued_;
};

}  // namespace msime::linux_host
