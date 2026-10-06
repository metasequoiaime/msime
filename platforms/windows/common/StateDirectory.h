#pragma once

#include <windows.h>
#include <shlobj.h>
#include <filesystem>
#include <string>
#include <vector>

#include "../../../shared/contracts/msime_edition.h"

namespace msime::windows {
// 32 位 TSF DLL 和 64 位 Server 各在自己的进程里解析状态根，两边必须落到同一个目录：偏好、词库和运行时租约都在那里。crates/host-windows 的 `server_state_directory` 是同一顺序的 Rust 副本，scripts/test-windows-state-dir-parity.py 核对两边都从版本表取这些名字。
// 环境变量名、注册表键和目录名都按版本取（shared/contracts/msime_edition.h）：几个版本同时安装时各有各的状态根，一个版本的覆盖设置不会把另一个版本也带过去。full 的环境变量和注册表键也是自己的（不是 msime-windows 用的 METASEQUOIA_IME_DATA_DIR 和 Software\Metasequoia\MetasequoiaIME）。
inline constexpr wchar_t state_directory_environment_variable[] = MSIME_EDITION_DATA_DIR_ENVIRONMENT_VARIABLE;
inline constexpr wchar_t state_directory_registry_key[] = MSIME_EDITION_REGISTRY_KEY;
inline constexpr wchar_t state_directory_registry_value[] = L"DataDir";
inline constexpr wchar_t state_directory_folder_name[] = MSIME_EDITION_STATE_DIRECTORY;

// 先是绝对路径的本版本数据目录环境变量（full 是 METASEQUOIA_IME_FULL_DATA_DIR），再是安装器写在本版本 HKLM 键下的 DataDir，最后是 %LOCALAPPDATA%\<本版本的状态目录名>（full 是 MSIME-Client）。只有 known folder 查询失败时返回空。
inline std::filesystem::path resolve_state_directory() {
  // Keep the Windows host relocatable like the upstream installer. The installer/enterprise launcher can provide one absolute data directory; all preferences, dictionaries and runtime leases then follow it instead of silently splitting state between the redirected path and LocalAppData. Read through the process environment block rather than the CRT's copy: the TSF DLL is loaded into arbitrary host processes whose CRT environment may be a stale snapshot, or belong to a different CRT than the one the DLL links.
  {
    std::vector<wchar_t> configured(32768);
    const DWORD length = GetEnvironmentVariableW(
        state_directory_environment_variable, configured.data(),
        static_cast<DWORD>(configured.size()));
    if (length && length < configured.size()) {
      const std::filesystem::path value(std::wstring(configured.data(), length));
      if (value.is_absolute())
        return value;
    }
  }
  // The installer stores its user-selected directory in the 64-bit machine view so the 32-bit TSF DLL and the 64-bit Server resolve the same root. Keep the registry lookup after the environment override for enterprise launches that deliberately inject a temporary profile.
  {
    DWORD bytes = 0;
    if (RegGetValueW(HKEY_LOCAL_MACHINE, state_directory_registry_key,
                     state_directory_registry_value,
                     RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY, nullptr, nullptr,
                     &bytes) == ERROR_SUCCESS &&
        bytes >= sizeof(wchar_t)) {
      std::wstring value(bytes / sizeof(wchar_t), L'\0');
      if (RegGetValueW(HKEY_LOCAL_MACHINE, state_directory_registry_key,
                       state_directory_registry_value,
                       RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY, nullptr,
                       value.data(), &bytes) == ERROR_SUCCESS) {
        value.resize((bytes / sizeof(wchar_t)) - 1);
        const std::filesystem::path path(value);
        if (path.is_absolute())
          return path;
      }
    }
  }
  PWSTR app_data = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr,
                                  &app_data)))
    return {};
  const std::filesystem::path state =
      std::filesystem::path(app_data) / state_directory_folder_name;
  CoTaskMemFree(app_data);
  return state;
}
} // namespace msime::windows
