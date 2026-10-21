#include "../src/system/KeySound.h"

#include <cassert>
#include <cstdint>
#include <utility>
#include <vector>

int main() {
  using msime::linux_host::key_press_sounds;
  using msime::linux_host::key_sound_class;
  using msime::linux_host::MusicActivity;

  // The classes msime_client_key_sound takes, for the keysyms IBus and Fcitx5 report.
  assert(key_sound_class(0x0020) == 1);
  assert(key_sound_class(0xff80) == 1);
  assert(key_sound_class(0xff0d) == 2);
  assert(key_sound_class(0xff8d) == 2);
  assert(key_sound_class(0xff08) == 3);
  for (std::uint32_t other : {std::uint32_t{'a'}, std::uint32_t{'1'}, std::uint32_t{'@'},
                              std::uint32_t{0xffff}, std::uint32_t{0xff1b}, std::uint32_t{0xff51}})
    assert(key_sound_class(other) == 0);

  // Typing keys sound; releases, bare modifiers and shortcuts do not.
  assert(key_press_sounds(false, false, false));
  assert(!key_press_sounds(true, false, false));
  assert(!key_press_sounds(false, true, false));
  assert(!key_press_sounds(false, false, true));

  // The typing effect: the combo is the answer's low 16 bits, shown from two keys on.
  using msime::linux_host::KeyRepeat;
  using msime::linux_host::typing_combo_label;
  using msime::linux_host::typing_effect_combo;
  assert(msime::linux_host::kTypingEffectCommit == 4);
  assert(msime::linux_host::kTypingEffectRepeat == 0x100);
  assert(typing_effect_combo(0) == 0);
  assert(typing_effect_combo(0x0010000c) == 12);
  assert(typing_effect_combo(0x0005ffff) == 65535);
  assert(typing_combo_label(0).empty());
  assert(typing_combo_label(1).empty());
  assert(typing_combo_label(2) == "连击 ×2");
  assert(typing_combo_label(12) == "连击 ×12");

  // A press of the key still held is its auto-repeat; a release, another key or a reset ends that.
  KeyRepeat repeat;
  assert(!repeat.press('a'));
  assert(repeat.press('a'));
  assert(repeat.press('a'));
  repeat.release('a');
  assert(!repeat.press('a'));
  assert(!repeat.press('b'));
  repeat.release('a');
  assert(repeat.press('b'));
  repeat.reset();
  assert(!repeat.press('b'));
  // 按的是硬件键码：Shift+1 按下时 keysym 是 '!'，先松开 Shift 再松开 1 时松开带的是 '1'，但键码不变，下一次 Shift+1 仍是新的按下。
  constexpr std::uint32_t kKeycodeOne = 10;
  repeat.reset();
  assert(!repeat.press(kKeycodeOne));
  repeat.release(kKeycodeOne);
  assert(!repeat.press(kKeycodeOne));
  assert(repeat.press(kKeycodeOne));
  // 键码 0 是面板或屏幕键盘合成的键，连着两次也不算重复。
  repeat.reset();
  assert(!repeat.press(0));
  assert(!repeat.press(0));

  // Music: the player hears only changes, and only through a live session.
  std::vector<std::pair<std::uint64_t, bool>> calls;
  bool accept = true;
  const auto call = [&](std::uint64_t session, bool active) {
    calls.emplace_back(session, active);
    return accept;
  };
  MusicActivity music;
  music.sync(0, true, call);
  assert(calls.empty() && !music.told());
  music.sync(7, false, call);
  assert(calls.empty());
  music.sync(7, true, call);
  assert(calls.size() == 1 && calls.back() == std::make_pair(std::uint64_t{7}, true) &&
         music.told());
  music.sync(7, true, call);
  assert(calls.size() == 1);
  music.sync(7, false, call);
  assert(calls.size() == 2 && calls.back() == std::make_pair(std::uint64_t{7}, false) &&
         !music.told());

  // A call the library refused (nothing switched on yet) is not remembered, so the next sync repeats it.
  accept = false;
  music.sync(7, true, call);
  music.sync(7, true, call);
  assert(calls.size() == 4 && !music.told());
  accept = true;
  music.sync(7, true, call);
  assert(calls.size() == 5 && music.told());

  // A session going away tells a playing player the music is over; the next session starts from silence.
  music.release(7, call);
  assert(calls.size() == 6 && calls.back() == std::make_pair(std::uint64_t{7}, false) &&
         !music.told());
  music.release(8, call);
  assert(calls.size() == 6);
  music.sync(8, true, call);
  assert(calls.size() == 7 && calls.back() == std::make_pair(std::uint64_t{8}, true));
  // Without a session there is nobody to call, but the next session must not inherit the claim.
  music.release(0, call);
  assert(calls.size() == 7 && !music.told());
}
