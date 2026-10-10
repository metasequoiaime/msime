#pragma once
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <shellapi.h>

namespace msime::windows {
// 系统自带的表情面板（Win+.）。共享应用不在时表情按钮和托盘的表情行退回到它，工具栏右键菜单的「表情与符号…」总是打开它，与 macOS 退回系统字符面板一致。按键注入给前台窗口：工具栏和菜单都不抢焦点，所以面板跟着用户正在打字的输入框出现。Windows 没有给 Win32 进程开这个面板的公开接口，合成这个组合键是唯一的办法。
inline bool open_system_emoji_panel() {
  INPUT inputs[4]{};
  for (auto &input : inputs)
    input.type = INPUT_KEYBOARD;
  inputs[0].ki.wVk = VK_LWIN;
  inputs[1].ki.wVk = VK_OEM_PERIOD;
  inputs[2].ki.wVk = VK_OEM_PERIOD;
  inputs[2].ki.dwFlags = KEYEVENTF_KEYUP;
  inputs[3].ki.wVk = VK_LWIN;
  inputs[3].ki.dwFlags = KEYEVENTF_KEYUP;
  return SendInput(4, inputs, sizeof(INPUT)) == 4;
}

// 系统的屏幕键盘 osk.exe。共享应用不在时屏幕键盘按钮和托盘的键盘行退回到它，与 macOS 退回原生屏幕键盘面板对应。
inline bool open_system_screen_keyboard() {
  return reinterpret_cast<INT_PTR>(ShellExecuteW(nullptr, L"open", L"osk.exe", nullptr,
                                                 nullptr, SW_SHOWNORMAL)) > 32;
}

// 用默认浏览器打开一个网址。
inline bool open_web_page(const wchar_t *url) {
  return reinterpret_cast<INT_PTR>(
             ShellExecuteW(nullptr, L"open", url, nullptr, nullptr, SW_SHOWNORMAL)) > 32;
}
} // namespace msime::windows
