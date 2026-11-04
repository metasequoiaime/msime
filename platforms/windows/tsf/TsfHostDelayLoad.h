#pragma once

#include <string>

namespace msime::windows {

// 从 TSF 模块路径计算同目录宿主 DLL 的绝对路径。
inline std::wstring host_library_path(const std::wstring &module_path,
                                      const std::wstring &host_name) {
  const auto separator = module_path.find_last_of(L"\\/");
  if (separator == std::wstring::npos) return host_name;
  return module_path.substr(0, separator + 1) + host_name;
}

}  // 命名空间 msime::windows
