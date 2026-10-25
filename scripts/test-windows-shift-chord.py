#!/usr/bin/env python3
"""执行生产 Shift 钩子和 TSF 判定，核对组合键与重复通知。"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent

HARNESS = r'''
#include <chrono>
#include <cstdint>
#include <initializer_list>
#include <iostream>
#include <stdexcept>

#define CALLBACK
#ifndef _Out_
#define _Out_
#endif
using UINT = unsigned;
using BYTE = uint8_t;
using WPARAM = uintptr_t;
using LPARAM = intptr_t;
using LRESULT = intptr_t;
using ULONG_PTR = uintptr_t;
using ULONGLONG = uint64_t;
using GUID = int;
using HHOOK = void *;
constexpr UINT VK_BACK = 8, VK_SHIFT = 16, VK_CONTROL = 17, VK_MENU = 18,
  VK_CAPITAL = 20, VK_LWIN = 91, VK_RWIN = 92, VK_LSHIFT = 160,
  VK_RSHIFT = 161, VK_LCONTROL = 162, VK_RCONTROL = 163,
  VK_LMENU = 164, VK_RMENU = 165, WM_BareShiftRelease = 1;
constexpr int HC_ACTION = 0;
constexpr auto kModifierHotkeyToggleLimit = std::chrono::milliseconds(500);
bool physical[256] = {};
int posted = 0;
ULONG_PTR extra_info = 0;
UINT LOWORD(WPARAM value) { return value & 0xffff; }
short GetAsyncKeyState(UINT key) { return physical[key] ? static_cast<short>(0x8000) : 0; }
ULONG_PTR GetMessageExtraInfo() { return extra_info; }
bool IsSelfGeneratedSendInputExtraInfo(ULONG_PTR info) { return info == 5; }
ULONGLONG GetTickCount64() { return 100; }
bool PostMessage(void *, UINT, WPARAM, LPARAM) { ++posted; return true; }
LRESULT CallNextHookEx(HHOOK, int, WPARAM, LPARAM) { return 0; }
namespace Global {
constexpr GUID MetasequoiaIMEGuidImeModePreserveKey = 1,
  MetasequoiaIMEGuidImeModePreserveKey03 = 3;
}
namespace FanyUtils {
struct Hotkeys { bool shift = true, ctrl = true; } hotkeys;
Hotkeys ReadConfiguredSwitchLanguageHotkeys() { return hotkeys; }
}
struct CMetasequoiaIME {
  static CMetasequoiaIME *_bareShiftHookOwner;
  HHOOK _bareShiftHook = reinterpret_cast<HHOOK>(1);
  BYTE _bareShiftDownMask = 0;
  bool _bareShiftArmed = false, _shiftHotkeyArmed = false, _ctrlHotkeyArmed = false;
  UINT _bareShiftSequence = 0, _bareShiftFocusGeneration = 0, _deferredKeyFocusGeneration = 1;
  ULONGLONG _bareShiftExpireTick = 0;
  void *_msgWndHandle = reinterpret_cast<void *>(1);
  std::chrono::steady_clock::time_point _modifierHotkeyExpire;
  struct { void interrupt() {} } _capsLockTap;
  static LRESULT CALLBACK _BareShiftKeyboardHookProc(int, WPARAM, LPARAM);
  void _TrackModifierHotkeyArming(WPARAM, LPARAM, bool);
  bool _MatchModifierReleaseHotkey(WPARAM, GUID *);
};
CMetasequoiaIME *CMetasequoiaIME::_bareShiftHookOwner = nullptr;

// 生产方法

void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}
LPARAM edge(bool up, bool right = false, bool repeated = false) {
  return (right ? 0x360001u : 0x2a0001u) |
    (up ? 0xc0000000u : repeated ? 0x40000000u : 0u);
}
void hook(UINT key, bool up, bool right = false) {
  CMetasequoiaIME::_BareShiftKeyboardHookProc(HC_ACTION, key, edge(up, right));
}
void reset(CMetasequoiaIME &ime) {
  ime = CMetasequoiaIME{};
  CMetasequoiaIME::_bareShiftHookOwner = &ime;
  for (auto &down : physical) down = false;
  posted = 0;
  extra_info = 0;
  FanyUtils::hotkeys = {};
}
void press(CMetasequoiaIME &ime) {
  physical[VK_SHIFT] = true;
  if (ime._bareShiftHook) hook(VK_SHIFT, false);
  ime._TrackModifierHotkeyArming(VK_SHIFT, edge(false), false);
}
bool release(CMetasequoiaIME &ime) {
  physical[VK_SHIFT] = false;
  if (ime._bareShiftHook) hook(VK_SHIFT, true);
  GUID action = 0;
  return ime._MatchModifierReleaseHotkey(VK_SHIFT, &action);
}
int main() {
  CMetasequoiaIME ime;
  GUID action = 0;
  reset(ime); press(ime);
  require(release(ime) && posted == 1, "a bare Shift must keep both TSF and hook routes available");
  require(!ime._MatchModifierReleaseHotkey(VK_SHIFT, &action), "a duplicate TSF release must not toggle twice");

  for (UINT key : {'T', '9', '0'}) {
    reset(ime); press(ime);
    // 应用没有把组合键的普通按下交给 TSF，但线程键盘钩子仍观察到真实按键。
    physical[key] = true; hook(key, false);
    physical[key] = false; hook(key, true);
    require(!release(ime) && posted == 0,
            "#9445: a chord observed by the hook must cancel the TSF Shift toggle");
  }

  reset(ime); press(ime);
  physical[static_cast<unsigned>('T')] = true; hook('T', false);
  ime._TrackModifierHotkeyArming('T', edge(false), false);
  physical[static_cast<unsigned>('T')] = false; hook('T', true);
  // 同一轮 Shift 的无重复位通知也不能把已经取消的轻点重新布防。
  ime._TrackModifierHotkeyArming(VK_SHIFT, edge(false), false);
  require(!release(ime), "a duplicate Shift notification must not rearm a chord");

  reset(ime); press(ime);
  require(!ime._MatchModifierReleaseHotkey(VK_SHIFT, &action),
          "a release callback while Shift is still physically held must not toggle");
  require(release(ime), "an early release callback must not consume the real bare release");

  reset(ime); physical[static_cast<unsigned>('T')] = true; press(ime);
  physical[static_cast<unsigned>('T')] = false;
  require(!release(ime), "pressing Shift after another held key is also a chord");

  reset(ime); ime._bareShiftHook = nullptr; physical[static_cast<unsigned>('T')] = true; press(ime);
  physical[static_cast<unsigned>('T')] = false;
  require(!release(ime), "the TSF-only route must reject a key already held before Shift");

  reset(ime); ime._bareShiftHook = nullptr; press(ime);
  require(release(ime) && posted == 0, "a bare tap must work when no hook is installed");

  reset(ime); physical[VK_SHIFT] = true;
  // 钩子虽然安装了，但这轮先从 TSF 进入，不能把未观察到按下误当作组合键已经取消。
  ime._TrackModifierHotkeyArming(VK_SHIFT, edge(false), false);
  physical[VK_SHIFT] = false;
  require(ime._MatchModifierReleaseHotkey(VK_SHIFT, &action),
          "an installed hook without an observed down must not disable a TSF bare tap");

  reset(ime); press(ime);
  hook(VK_RSHIFT, false, true);
  ime._TrackModifierHotkeyArming(VK_RSHIFT, edge(false, true), false);
  hook(VK_LSHIFT, true);
  require(!ime._MatchModifierReleaseHotkey(VK_LSHIFT, &action),
          "releasing one of two held Shift keys must not toggle");
  physical[VK_SHIFT] = false; hook(VK_RSHIFT, true, true);
  require(ime._MatchModifierReleaseHotkey(VK_RSHIFT, &action) && posted == 1,
          "releasing the final Shift must keep a bare tap available once");

  reset(ime); press(ime);
  extra_info = 5; hook('T', false); extra_info = 0;
  require(release(ime), "self-generated helper input must not cancel a bare Shift");

  reset(ime); press(ime);
  physical[VK_SHIFT] = false;
  ime._modifierHotkeyExpire = std::chrono::steady_clock::now() - std::chrono::milliseconds(1);
  require(!ime._MatchModifierReleaseHotkey(VK_SHIFT, &action), "a long hold must not toggle");
  reset(ime); press(ime); FanyUtils::hotkeys.shift = false;
  require(!release(ime), "the Shift preference must still be respected");
  reset(ime); press(ime);
  require(!ime._MatchModifierReleaseHotkey(VK_SHIFT, nullptr), "a null output must be rejected");

  reset(ime); physical[VK_CONTROL] = true;
  ime._TrackModifierHotkeyArming(VK_CONTROL, edge(false), false);
  physical[VK_CONTROL] = false;
  require(ime._MatchModifierReleaseHotkey(VK_CONTROL, &action) && action == 3,
          "the existing Ctrl toggle must keep working");
  std::cout << "Shift chords: cancellation, duplicate notifications, physical release and TSF-only taps passed\n";
}
'''


def method(source: str, signature: str) -> str:
    begin = source.index(signature)
    return source[begin:source.index("\n}", begin) + 2]


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if not compiler:
        print("skipped: no C++ compiler for the Shift chord regression")
        return 0
    source = (ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp").read_text(encoding="utf-8")
    signatures = ["bool IsShiftVk(", "bool IsControlVk(", "bool IsAltVk(",
                  "bool IsWinVk(", "bool IsOtherKeyboardKeyDown(",
                  "bool IsOnlyModifierPhysicallyDown(",
                  "LRESULT CALLBACK CMetasequoiaIME::_BareShiftKeyboardHookProc(",
                  "void CMetasequoiaIME::_TrackModifierHotkeyArming(",
                  "bool CMetasequoiaIME::_MatchModifierReleaseHotkey("]
    harness = HARNESS.replace("// 生产方法", "\n".join(method(source, s) for s in signatures))
    with tempfile.TemporaryDirectory(prefix="msime-shift-chord-") as temporary:
        directory = pathlib.Path(temporary)
        cpp, binary = directory / "shift.cpp", directory / "shift"
        cpp.write_text(harness, encoding="utf-8")
        built = subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror",
                                str(cpp), "-o", str(binary)], check=False)
        if built.returncode:
            return built.returncode
        return subprocess.run([str(binary)], check=False, timeout=30).returncode


if __name__ == "__main__":
    raise SystemExit(main())
