#!/usr/bin/env python3
"""编译执行实际 Shift 观察方法，覆盖 TSF 与线程钩子只见到部分按键。"""
import shutil
import pathlib
import subprocess
import tempfile
root = pathlib.Path(__file__).resolve().parent.parent
compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
if compiler is None:
    print("skipped: C++ compiler unavailable for Windows Shift observation regression")
    raise SystemExit(0)
sink = (root / 'platforms/windows/tsf/Key/KeyEventSink.cpp').read_text()
def method(start, end):
    pos = sink.index(start)
    return sink[pos:sink.index(end, pos)]
unit = r'''
#include <chrono>
#include <cstdint>
#include <initializer_list>
#include <cstdio>
using UINT=unsigned; using BYTE=unsigned char; using WPARAM=std::uintptr_t;
using LPARAM=std::intptr_t; using LRESULT=std::intptr_t; using ULONG_PTR=std::uintptr_t;
using ULONGLONG=unsigned long long; using GUID=int;
#ifndef _Out_
#define _Out_
#endif
namespace Global { constexpr int MetasequoiaIMEGuidImeModePreserveKey=1, MetasequoiaIMEGuidImeModePreserveKey03=2; }
namespace FanyUtils {
 struct Hotkeys { bool shift=true, ctrl=true; };
 Hotkeys ReadConfiguredSwitchLanguageHotkeys() { return {}; }
}
#define CALLBACK
constexpr int HC_ACTION=0;
constexpr UINT VK_SHIFT=0x10, VK_LSHIFT=0xa0, VK_RSHIFT=0xa1, VK_CONTROL=0x11,
 VK_LCONTROL=0xa2, VK_RCONTROL=0xa3, VK_MENU=0x12, VK_LMENU=0xa4, VK_RMENU=0xa5,
 VK_LWIN=0x5b, VK_RWIN=0x5c, VK_CAPITAL=0x14, WM_BareShiftRelease=20;
constexpr auto kModifierHotkeyToggleLimit=std::chrono::milliseconds(500);
UINT LOWORD(WPARAM value) { return value&0xffff; }
// 生产修饰键分类
bool IsOtherKeyboardKeyDown() { return false; }
bool IsOnlyModifierPhysicallyDown(UINT) { return true; }
bool IsSelfGeneratedSendInputExtraInfo(ULONG_PTR) { return false; }
ULONG_PTR GetMessageExtraInfo() { return 0; }
ULONGLONG GetTickCount64() { return 1; }
short GetAsyncKeyState(UINT) { return 0; }
int posted=0;
bool PostMessage(void*,UINT,WPARAM,LPARAM) { ++posted; return true; }
LRESULT CallNextHookEx(void*,int,WPARAM,LPARAM) { return 0; }
struct CMetasequoiaIME {
 static CMetasequoiaIME* _bareShiftHookOwner;
 void* _bareShiftHook=nullptr; void* _msgWndHandle=this;
 BYTE _bareShiftDownMask=0; bool _bareShiftArmed=false;
 UINT _bareShiftSequence=0; std::uint64_t _bareShiftFocusGeneration=0,
 _deferredKeyFocusGeneration=1; ULONGLONG _bareShiftExpireTick=0;
 bool _shiftHotkeyArmed=false, _ctrlHotkeyArmed=false;
 std::chrono::steady_clock::time_point _modifierHotkeyExpire;
 struct { void interrupt() {} } _capsLockTap;
 static LRESULT CALLBACK _BareShiftKeyboardHookProc(int,WPARAM,LPARAM);
 void _TrackModifierHotkeyArming(WPARAM,LPARAM,bool);
 bool _MatchModifierReleaseHotkey(WPARAM,GUID*);
};
CMetasequoiaIME* CMetasequoiaIME::_bareShiftHookOwner=nullptr;
'''
unit = unit.replace('// 生产修饰键分类', method('bool IsShiftVk(', 'bool IsOtherKeyboardKeyDown('))
unit += method('LRESULT CALLBACK CMetasequoiaIME::_BareShiftKeyboardHookProc(', '\nvoid CMetasequoiaIME::_MarkBareShiftHandled(')
unit += method('void CMetasequoiaIME::_TrackModifierHotkeyArming(', '\nbool CMetasequoiaIME::_MatchChordInputHotkey(')
unit += method('bool CMetasequoiaIME::_MatchModifierReleaseHotkey(', '\nvoid CMetasequoiaIME::_ArmCapsLockTap(')
unit += r'''
int checks=0, failures=0;
void check(bool passed, const char* message) {
 ++checks;
 if (!passed) { ++failures; std::fprintf(stderr,"FAIL: %s\n",message); }
}
struct Fixture {
 CMetasequoiaIME ime;
 static LPARAM flags(UINT key,bool up=false) {
  const std::uint32_t scan=key==VK_RSHIFT?0x36u:0x2au;
  return static_cast<LPARAM>((scan<<16)|1u|(up?0xc0000000u:0u));
 }
 Fixture() { CMetasequoiaIME::_bareShiftHookOwner=&ime; posted=0; }
 void hook(UINT key,bool up=false) {
  ime._BareShiftKeyboardHookProc(HC_ACTION,key,flags(key,up));
 }
 void tsf(UINT key) { ime._TrackModifierHotkeyArming(key,flags(key),false); }
 void press() { hook(VK_SHIFT); tsf(VK_SHIFT); }
};
int main() {
 for (UINT letter : {UINT('A'),UINT('9'),UINT(0xba),VK_CONTROL,VK_MENU,VK_LWIN}) {
  {
   Fixture f; f.press(); f.tsf(letter); f.hook(VK_SHIFT,true);
   check(posted==0,"TSF-only chord must cancel hook Shift toggle");
  }
  {
   Fixture f; f.press(); f.hook(letter); GUID guid=0;
   check(!f.ime._MatchModifierReleaseHotkey(VK_SHIFT,&guid),
         "hook-only chord must cancel TSF Shift toggle");
  }
  {
   Fixture f; f.press(); f.tsf(letter); f.hook(letter); f.hook(VK_SHIFT,true); GUID guid=0;
   check(posted==0 && !f.ime._MatchModifierReleaseHotkey(VK_SHIFT,&guid),
         "both observers keep chords from toggling");
  }
 }
 for (UINT shift : {VK_SHIFT,VK_LSHIFT,VK_RSHIFT}) {
  {
   Fixture f; f.hook(shift); f.tsf(shift); f.hook(shift,true);
   check(posted==1,"bare Shift still posts one hook fallback");
   f.hook(shift,true); check(posted==1,"duplicate release must not repost");
  }
  {
   Fixture f; f.tsf(shift); GUID guid=0;
   check(f.ime._MatchModifierReleaseHotkey(shift,&guid),"bare Shift still matches TSF release");
   check(!f.ime._MatchModifierReleaseHotkey(shift,&guid),"duplicate TSF release must not toggle");
  }
  {
   Fixture f; f.hook(shift); f.tsf(shift); f.tsf('A');
   f.ime._TrackModifierHotkeyArming(shift,Fixture::flags(shift)|0x40000000,false);
   f.hook(shift,true); GUID guid=0;
   check(posted==0 && !f.ime._MatchModifierReleaseHotkey(shift,&guid),
         "Shift repeat cannot rearm a chord");
  }
 }
 std::printf("Shift chord observation: %d checks, %d failures\n",checks,failures);
 return failures?1:0;
}
'''
with tempfile.TemporaryDirectory(prefix="msime-shift-observation-") as tmp:
    source=pathlib.Path(tmp)/'probe.cpp'; binary=pathlib.Path(tmp)/'probe'
    source.write_text(unit)
    subprocess.run([compiler,'-std=c++17','-Wall','-Wextra','-Werror',str(source),'-o',str(binary)],check=True)
    raise SystemExit(subprocess.run([str(binary)]).returncode)
