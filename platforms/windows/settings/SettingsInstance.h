#pragma once

#include <windows.h>
#include <sddl.h>
#include <functional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>
#include "../../../shared/contracts/msime_edition.h"

namespace msime::settings {

// 两个原生设置宿主共用进程入口守卫；锁覆盖初始化和整个窗口生命周期。
class SettingsInstance final {
public:
  enum class Start { primary, forwarded, failed };
  using Activate = std::function<bool(std::string_view)>;

  explicit SettingsInstance(std::wstring suffix = MSIME_EDITION_NAME_SUFFIX)
      : name_(L"Local\\MSIME.Settings" + suffix + user_suffix()) {}
  SettingsInstance(const SettingsInstance &) = delete;
  SettingsInstance &operator=(const SettingsInstance &) = delete;
  ~SettingsInstance() {
    if (endpoint_) DestroyWindow(endpoint_);
    if (owned_) ReleaseMutex(mutex_);
    if (mutex_) CloseHandle(mutex_);
  }

  Start start(std::string_view route, DWORD timeout = 10000) {
    if (mutex_ || !valid_route(route)) return Start::failed;
    mutex_ = CreateMutexW(nullptr, TRUE, name_.c_str());
    if (!mutex_) return Start::failed;
    if (GetLastError() != ERROR_ALREADY_EXISTS) {
      owned_ = true;
      return Start::primary;
    }
    const ULONGLONG deadline = GetTickCount64() + timeout;
    for (;;) {
      // 原实例已经退出或在初始化中崩溃时接管，不依赖容易过期的窗口句柄。
      const DWORD acquired = WaitForSingleObject(mutex_, 0);
      if (acquired == WAIT_OBJECT_0 || acquired == WAIT_ABANDONED) {
        owned_ = true;
        return Start::primary;
      }
      if (acquired != WAIT_TIMEOUT) return Start::failed;
      if (const HWND endpoint = FindWindowW(endpoint_class, name_.c_str())) {
        DWORD pid = 0;
        GetWindowThreadProcessId(endpoint, &pid);
        AllowSetForegroundWindow(pid);
        // 只转发至多 64 字节的设置路由，不接受路径、环境变量或任意命令。
        const std::string payload(route);
        COPYDATASTRUCT data{activation_id, static_cast<DWORD>(payload.size() + 1),
                            const_cast<char *>(payload.c_str())};
        DWORD_PTR accepted = 0;
        if (SendMessageTimeoutW(endpoint, WM_COPYDATA, 0,
                                reinterpret_cast<LPARAM>(&data),
                                SMTO_ABORTIFHUNG | SMTO_BLOCK, 2000, &accepted) && accepted)
          return Start::forwarded;
        return Start::failed;
      }
      // 首个窗口还没建好时等待其发布入口；超时也不能绕过锁另开窗口。
      if (GetTickCount64() >= deadline) return Start::failed;
      Sleep(20);
    }
  }

  bool listen(HWND window, Activate activate) {
    if (!owned_ || endpoint_ || !IsWindow(window)) return false;
    window_ = window;
    activate_ = std::move(activate);
    WNDCLASSW descriptor{};
    descriptor.lpfnWndProc = receive;
    descriptor.hInstance = GetModuleHandleW(nullptr);
    descriptor.lpszClassName = endpoint_class;
    if (!RegisterClassW(&descriptor) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
      return false;
    // 隐藏顶层窗口可由 FindWindow 找到，并在原生 UI 线程处理转发。
    endpoint_ = CreateWindowExW(0, endpoint_class, name_.c_str(), WS_POPUP,
                                0, 0, 0, 0, nullptr, nullptr, descriptor.hInstance, this);
    return endpoint_ != nullptr;
  }

private:
  static constexpr const wchar_t *endpoint_class = L"MSIME.Settings.Activation";
  static constexpr ULONG_PTR activation_id = 0x4d53494d;
  static bool valid_route(std::string_view route) {
    if (route != "settings" && route.substr(0, 9) != "settings:") return false;
    if (route == "settings:") return false;
    return route.size() <= 64 && route.find(':', 9) == std::string_view::npos && route.find_first_not_of(
        "abcdefghijklmnopqrstuvwxyz0123456789-:") == std::string_view::npos;
  }
  static std::wstring user_suffix() {
    HANDLE token = nullptr;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token))
      throw std::runtime_error("Settings identity unavailable");
    DWORD size = 0;
    GetTokenInformation(token, TokenUser, nullptr, 0, &size);
    std::vector<unsigned char> buffer(size);
    const BOOL loaded = GetTokenInformation(token, TokenUser, buffer.data(), size, &size);
    CloseHandle(token);
    LPWSTR sid = nullptr;
    if (!loaded || !ConvertSidToStringSidW(
                       reinterpret_cast<TOKEN_USER *>(buffer.data())->User.Sid, &sid))
      throw std::runtime_error("Settings identity unavailable");
    const std::wstring result = L"." + std::wstring(sid);
    LocalFree(sid);
    return result;
  }
  static LRESULT CALLBACK receive(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    if (message == WM_NCCREATE) {
      const auto create = reinterpret_cast<CREATESTRUCTW *>(lparam);
      SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(create->lpCreateParams));
    }
    const auto self = reinterpret_cast<SettingsInstance *>(GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_COPYDATA && self) {
      const auto data = reinterpret_cast<const COPYDATASTRUCT *>(lparam);
      if (!data || data->dwData != activation_id || !data->lpData ||
          data->cbData < 2 || data->cbData > 65 || !IsWindow(self->window_)) return FALSE;
      const auto bytes = static_cast<const char *>(data->lpData);
      if (bytes[data->cbData - 1] != '\0') return FALSE;
      const std::string_view route(bytes, data->cbData - 1);
      if (!valid_route(route)) return FALSE;
      try {
        if (!self->activate_(route)) return FALSE;
        ShowWindow(self->window_, IsIconic(self->window_) ? SW_RESTORE : SW_SHOW);
        SetForegroundWindow(self->window_);
        return TRUE;
      } catch (...) {
        return FALSE;
      }
    }
    return DefWindowProcW(window, message, wparam, lparam);
  }
  std::wstring name_;
  HANDLE mutex_ = nullptr;
  bool owned_ = false;
  HWND endpoint_ = nullptr;
  HWND window_ = nullptr;
  Activate activate_;
};

} // namespace msime::settings
