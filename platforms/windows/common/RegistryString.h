#pragma once

#include <windows.h>
#include <string>

namespace msime::windows {
inline std::wstring read_registry_string_64(HKEY root, const wchar_t* subkey,
                                          const wchar_t* name) {
  HKEY opened = nullptr;
  // Win7 的 RegGetValueW 不支持用 RRF_SUBKEY_WOW6464KEY 选择视图，必须先打开键。
  if (RegOpenKeyExW(root, subkey, 0, KEY_QUERY_VALUE | KEY_WOW64_64KEY,
                    &opened) != ERROR_SUCCESS)
    return {};
  struct CloseKey {
    HKEY key;
    ~CloseKey() { RegCloseKey(key); }
  } close{opened};
  DWORD bytes = 0;
  if (RegGetValueW(opened, nullptr, name, RRF_RT_REG_SZ, nullptr, nullptr,
                   &bytes) != ERROR_SUCCESS || bytes < sizeof(wchar_t) ||
      bytes % sizeof(wchar_t) != 0 || bytes / sizeof(wchar_t) > 32768)
    return {};
  std::wstring value(bytes / sizeof(wchar_t), L'\0');
  if (RegGetValueW(opened, nullptr, name, RRF_RT_REG_SZ, nullptr, value.data(),
                   &bytes) != ERROR_SUCCESS || bytes % sizeof(wchar_t) != 0 ||
      bytes / sizeof(wchar_t) > value.size())
    return {};
  value.resize(bytes / sizeof(wchar_t));
  // RRF_RT_REG_SZ 补齐终止符；只取第一个终止符之前的内容，与 host-windows 的 installed_data_directory 一致，安装器多写的终止符不能进路径。
  const auto end = value.find(L'\0');
  if (end == std::wstring::npos)
    return {};
  value.resize(end);
  return value;
}
} // namespace msime::windows
