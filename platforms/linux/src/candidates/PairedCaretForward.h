#pragma once

namespace msime::linux_host {

// 成对标点补全后要把光标退回两半之间。IBus 没有直接设置文档光标的接口，只能转发方向键。
// Wayland 下虚拟键盘沿用物理修饰状态，Shift 还按着时转发方向键会被应用当成
// Shift+方向键去选中文字，所以方向键要排队到松开 Shift 之后，与 Fcitx5 宿主一致；
// 纯 X11 会话下直接转发，不引入排队带来的延迟。
struct PairedCaretForward {
  bool wayland_session = false;
  bool shift_held = false;

  constexpr bool defer() const { return wayland_session && shift_held; }
};

}  // namespace msime::linux_host
