#pragma once
#include <optional>
#include <string>
#include <windows.h>

namespace msime::windows {
// 进程可执行文件的基名（如 `code.exe`），UTF-8。应用例外按它查规则，和 TIP 用 GetModuleFileNameW 取的 `current_process_name` 是同一个名字。只要 PROCESS_QUERY_LIMITED_INFORMATION，提升权限的进程也能问到；受保护的进程问不到时返回空，规则就不命中。
inline std::optional<std::string> process_image_base_name(DWORD pid) {
  if (pid == 0)
    return std::nullopt;
  const HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
  if (!process)
    return std::nullopt;
  std::wstring path(32768, L'\0');
  DWORD length = static_cast<DWORD>(path.size());
  const BOOL found = QueryFullProcessImageNameW(process, 0, path.data(), &length);
  CloseHandle(process);
  if (!found || length == 0)
    return std::nullopt;
  path.resize(length);
  const auto separator = path.find_last_of(L"\\/");
  const std::wstring base = separator == std::wstring::npos ? path : path.substr(separator + 1);
  if (base.empty())
    return std::nullopt;
  const int wide = static_cast<int>(base.size());
  const int size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, base.data(), wide, nullptr, 0, nullptr, nullptr);
  if (size <= 0)
    return std::nullopt;
  std::string name(static_cast<size_t>(size), '\0');
  if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, base.data(), wide, name.data(), size, nullptr, nullptr) != size)
    return std::nullopt;
  return name;
}
} // namespace msime::windows
