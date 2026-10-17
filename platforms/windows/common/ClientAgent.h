#pragma once

// 宿主自报的 User-Agent（#8201 的 P11）里 Windows 要交出的两段：设备型号和系统版本。拼装与清洗都在 Rust（msime_client_client_agent），这里只读系统；Server（nlohmann）和设置窗口（WinRT JSON）各自用自己的 JSON 库拼请求，所以这个头文件不带 JSON。服务端「我的设备」按注册、登录时的 User-Agent 显示平台、设备和版本。

#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

#include <string>

namespace msime::windows {

// 「Windows <产品版本> <内部版本号>」，例如 Windows 11 26100。Windows 11 的主版本号仍报 10，内部版本号从 22000 起才是 11。
inline std::string windows_system_name(unsigned long major, unsigned long build) {
  const auto product = major == 10 && build >= 22000 ? std::string("11") : std::to_string(major);
  return "Windows " + product + " " + std::to_string(build);
}

// 当前系统的 windows_system_name，取自 ntdll 的 RtlGetVersion：GetVersionEx 按应用清单声明的兼容系统作答，可能报低。读不到时为空，Rust 写成 unknown。
inline std::string windows_system_description() {
  using RtlGetVersion = LONG(WINAPI *)(PRTL_OSVERSIONINFOW);
  const auto ntdll = GetModuleHandleW(L"ntdll.dll");
  const auto get = ntdll ? reinterpret_cast<RtlGetVersion>(
                               reinterpret_cast<void *>(GetProcAddress(ntdll, "RtlGetVersion")))
                         : nullptr;
  RTL_OSVERSIONINFOW info{};
  info.dwOSVersionInfoSize = sizeof(info);
  if (!get || get(&info) != 0)
    return {};
  return windows_system_name(info.dwMajorVersion, info.dwBuildNumber);
}

// 设备型号：固件报给系统的产品名（HKLM\HARDWARE\DESCRIPTION\System\BIOS 的 SystemProductName，例如 Surface Pro），UTF-8。读不到时为空，Rust 写成 unknown；不认识的字符由 Rust 去掉。
inline std::string windows_device_model() {
  wchar_t value[256]{};
  DWORD bytes = sizeof(value);
  if (RegGetValueW(HKEY_LOCAL_MACHINE, L"HARDWARE\\DESCRIPTION\\System\\BIOS", L"SystemProductName",
                   RRF_RT_REG_SZ, nullptr, value, &bytes) != ERROR_SUCCESS)
    return {};
  const int length = WideCharToMultiByte(CP_UTF8, 0, value, -1, nullptr, 0, nullptr, nullptr);
  if (length <= 1)
    return {};
  std::string text(static_cast<size_t>(length), '\0');
  WideCharToMultiByte(CP_UTF8, 0, value, -1, text.data(), length, nullptr, nullptr);
  text.resize(static_cast<size_t>(length - 1));
  return text;
}

} // namespace msime::windows
