#include "../src/voice/VoiceHoldChord.h"

#include <cassert>
#include <initializer_list>
#include <ibus.h>

using msime::linux_host::voice_hold_becomes_chord;
using msime::linux_host::voice_hold_chord_modifier;

int main() {
  // 头文件里的数值必须就是这些 keysym：IBus 和 Fcitx5 用的都是 X keysym。
  for (guint key : {IBUS_Shift_L, IBUS_Shift_R, IBUS_Control_L, IBUS_Control_R, IBUS_Caps_Lock,
                    IBUS_Shift_Lock, IBUS_Meta_L, IBUS_Meta_R, IBUS_Alt_L, IBUS_Alt_R, IBUS_Super_L,
                    IBUS_Super_R, IBUS_Hyper_L, IBUS_Hyper_R, IBUS_ISO_Level3_Shift,
                    IBUS_ISO_Level5_Shift, IBUS_Mode_switch, IBUS_Num_Lock}) {
    assert(voice_hold_chord_modifier(key));
    // 还在拼组合键：修饰键不让按住变成组合键。
    assert(!voice_hold_becomes_chord(key, true, false));
  }
  // 功能键、字母、数字、方向键都让按住变成组合键（右 Alt+F10、Alt+F 打开菜单）。
  for (guint key : {IBUS_F10, IBUS_F, IBUS_f, IBUS_1, IBUS_Tab, IBUS_Return, IBUS_Left, IBUS_BackSpace})
    assert(!voice_hold_chord_modifier(key) && voice_hold_becomes_chord(key, true, false));
  // Esc 保持取消录音的含义。
  assert(!voice_hold_becomes_chord(IBUS_Escape, true, false));
  assert(!voice_hold_becomes_chord(IBUS_Escape, false, false));
  // 空格锁定开着时空格是锁定；关着时空格是普通键，和 Windows 一样算组合键。
  assert(!voice_hold_becomes_chord(IBUS_space, true, false));
  assert(voice_hold_becomes_chord(IBUS_space, false, false));
  // 已经锁定的按住是用户要留下的听写，什么键都不算组合键。
  assert(!voice_hold_becomes_chord(IBUS_F10, true, true));
  assert(!voice_hold_becomes_chord(IBUS_f, false, true));
  return 0;
}
