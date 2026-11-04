#pragma once
#include <cstdint>

// 按住右 Alt 说话时又按了别的键，用户按的是组合键而不是要说话（#10580，对齐 Windows #10581 的 voice_hold_becomes_chord）。IBus 宿主和 Fcitx5 插件共用这一条判定；两边的键值都是 X keysym，所以这里直接写数值，tests/voice/voice_hold_chord.cpp 拿 IBus 的常量逐个核对。
//
// 和 Windows 不同，这里不需要补发修饰键：输入法吞掉的只是右 Alt 自己的事件，后面按键事件里的修饰状态由 X 服务器或合成器给出，仍然带着 Alt，应用收到的就是组合键。宿主要做的只是结束按住、取消这次按住自己开始的录音。
namespace msime::linux_host {

// 修饰键（含 Caps Lock、Num Lock 这类锁定键）不算让按住变成组合键的那个键：用户还在拼组合，它照常交给应用。逐个列出，不用范围，免得哪天有人以为 Shift_L 到 Hyper_R 之间还夹着别的键。
constexpr bool voice_hold_chord_modifier(uint32_t keysym) {
  switch (keysym) {
  case 0xffe1: // Shift_L
  case 0xffe2: // Shift_R
  case 0xffe3: // Control_L
  case 0xffe4: // Control_R
  case 0xffe5: // Caps_Lock
  case 0xffe6: // Shift_Lock
  case 0xffe7: // Meta_L
  case 0xffe8: // Meta_R
  case 0xffe9: // Alt_L
  case 0xffea: // Alt_R
  case 0xffeb: // Super_L
  case 0xffec: // Super_R
  case 0xffed: // Hyper_L
  case 0xffee: // Hyper_R
  case 0xfe03: // ISO_Level3_Shift（AltGr 布局下的右 Alt）
  case 0xfe11: // ISO_Level5_Shift
  case 0xff7e: // Mode_switch
  case 0xff7f: // Num_Lock
    return true;
  default:
    return false;
  }
}

// 右 Alt（或右 Ctrl+右 Alt）按住期间新按下的这个键是否让按住变成组合键。调用方只在按下时、只在右 Alt 的按住还在时问。Esc 保持取消录音的含义；空格锁定开着时空格保持锁定；已经用空格锁定的按住是用户要留下的听写，不再算组合键。
constexpr bool voice_hold_becomes_chord(uint32_t keysym, bool space_lock, bool locked) {
  if (locked || voice_hold_chord_modifier(keysym))
    return false;
  if (keysym == 0xff1b) // Escape
    return false;
  return !(keysym == 0x20 && space_lock); // space
}

} // namespace msime::linux_host
