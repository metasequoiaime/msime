#pragma once

#include <windows.h>
#include <optional>

namespace msime::windows::compat {

// 读取内核版本，避免应用清单把新系统报告为旧版本；查询失败时不猜测版本。
inline const std::optional<OSVERSIONINFOW> &kernel_version() {
  static const auto version = []() -> std::optional<OSVERSIONINFOW> {
    using VersionQuery = LONG(WINAPI *)(OSVERSIONINFOW *);
    const auto module = GetModuleHandleW(L"ntdll.dll");
    const auto query = module ? reinterpret_cast<VersionQuery>(
        reinterpret_cast<void *>(GetProcAddress(module, "RtlGetVersion"))) : nullptr;
    OSVERSIONINFOW result{};
    result.dwOSVersionInfoSize = sizeof(result);
    if (!query || query(&result) != 0)
      return std::nullopt;
    return result;
  }();
  return version;
}

// Win7 会以 ERROR_INVALID_PARAMETER 拒绝这个 Win8 才引入的窗口样式。
// 新系统保留原样式，Win7 继续使用已有的分层窗口绘制。
inline DWORD no_redirection_bitmap_style() {
  // 旧 MinGW 在目标设为 Win7 时不声明此常量，数值仍按 Windows SDK 的样式契约固定。
  constexpr DWORD no_redirection_bitmap = 0x00200000;
  const auto &version = kernel_version();
  return version && (version->dwMajorVersion > 6 ||
                     (version->dwMajorVersion == 6 && version->dwMinorVersion >= 2))
             ? no_redirection_bitmap : 0;
}

// 只供不支持 TokenIsAppContainer 的错误分支使用；未知版本不能降低对端校验。
inline bool predates_appcontainers() {
  const auto &version = kernel_version();
  return version && (version->dwMajorVersion < 6 ||
                     (version->dwMajorVersion == 6 && version->dwMinorVersion < 2));
}

} // namespace msime::windows::compat
