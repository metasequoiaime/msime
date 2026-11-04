#pragma once

#include <string>

namespace msime::windows {

// 从 TSF 模块路径计算同目录宿主 DLL 的绝对路径。
inline std::wstring host_library_path(const std::wstring &module_path,
                                      const std::wstring &host_name) {
  const bool drive_absolute = module_path.size() >= 3 &&
      ((module_path[0] >= L'A' && module_path[0] <= L'Z') ||
       (module_path[0] >= L'a' && module_path[0] <= L'z')) &&
      module_path[1] == L':' &&
      (module_path[2] == L'\\' || module_path[2] == L'/');
  const bool unc_absolute = module_path.size() > 2 &&
      module_path[0] == L'\\' && module_path[1] == L'\\';
  if ((!drive_absolute && !unc_absolute) || host_name.empty() ||
      host_name.find_first_of(L"\\/:") != std::wstring::npos) return {};
  const auto separator = module_path.find_last_of(L"\\/");
  if (separator == std::wstring::npos) return {};
  return module_path.substr(0, separator + 1) + host_name;
}

}  // 命名空间 msime::windows
