#pragma once
#include "SecureFieldPolicy.h"
#include <chrono>
#include <condition_variable>
#include <iterator>
#include <memory>
#include <mutex>
#include <optional>
#include <thread>
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <objbase.h>
#include <uiautomationclient.h>

namespace msime::windows {
// 前台窗口所在线程的焦点控件是不是带 ES_PASSWORD 的 Win32 编辑控件。只读窗口样式，不向那个线程发消息，挂起的程序也不会拖住调用方。
inline bool focused_win32_password_edit() {
  const HWND foreground = GetForegroundWindow();
  if (!foreground)
    return false;
  GUITHREADINFO info{};
  info.cbSize = sizeof(info);
  if (!GetGUIThreadInfo(GetWindowThreadProcessId(foreground, nullptr), &info) ||
      !info.hwndFocus)
    return false;
  wchar_t name[96]{};
  if (!GetClassNameW(info.hwndFocus, name, static_cast<int>(std::size(name))))
    return false;
  return win32_password_edit(
      name, static_cast<unsigned long>(GetWindowLongPtrW(info.hwndFocus, GWL_STYLE)));
}
// UI Automation 报告的焦点元素是不是密码框。跨进程的 UI Automation 调用要等焦点所在的程序回答，所以放在自己的 MTA 线程上做，调用方最多等 `budget`；超时返回空，线程做完后自行结束，结果没人再读。CLSID 和 IID 在这里写出，不依赖哪个导入库带着它们。
inline std::optional<bool>
automation_focused_password(std::chrono::milliseconds budget) {
  static const CLSID automation_class = {
      0xff48dba4, 0x60ef, 0x4201, {0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e}};
  static const IID automation_interface = {
      0x30cbe57d, 0xd9d0, 0x452a, {0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee}};
  struct Answer {
    std::mutex mutex;
    std::condition_variable ready;
    bool done = false;
    std::optional<bool> password;
  };
  auto answer = std::make_shared<Answer>();
  try {
    std::thread([answer] {
      std::optional<bool> password;
      if (SUCCEEDED(CoInitializeEx(nullptr, COINIT_MULTITHREADED))) {
        IUIAutomation *automation = nullptr;
        if (SUCCEEDED(CoCreateInstance(automation_class, nullptr,
                                       CLSCTX_INPROC_SERVER, automation_interface,
                                       reinterpret_cast<void **>(&automation))) &&
            automation) {
          IUIAutomationElement *element = nullptr;
          if (SUCCEEDED(automation->GetFocusedElement(&element)) && element) {
            BOOL value = FALSE;
            if (SUCCEEDED(element->get_CurrentIsPassword(&value)))
              password = value != FALSE;
            element->Release();
          }
          automation->Release();
        }
        CoUninitialize();
      }
      {
        std::lock_guard<std::mutex> lock(answer->mutex);
        answer->password = password;
        answer->done = true;
      }
      answer->ready.notify_all();
    }).detach();
  } catch (...) {
    // 起不了线程只是少了这一路证据，按没有回答处理。
    return std::nullopt;
  }
  std::unique_lock<std::mutex> lock(answer->mutex);
  if (!answer->ready.wait_for(lock, budget, [&] { return answer->done; }))
    return std::nullopt;
  return answer->password;
}
// 焦点控件的类别：先看 Win32 样式位，不是再问 UI Automation。
inline FocusedFieldKind focused_field_kind(std::chrono::milliseconds budget) {
  if (focused_win32_password_edit())
    return FocusedFieldKind::Password;
  return focused_field_kind(false, automation_focused_password(budget));
}
} // namespace msime::windows
