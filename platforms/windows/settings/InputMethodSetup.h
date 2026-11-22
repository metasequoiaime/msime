#pragma once
#include <windows.h>
#include <algorithm>
#include <cstring>
#include <cwchar>
#include <string>
#include <vector>
#include "../../../shared/contracts/msime_edition.h"

namespace msime::settings::input_method {


// 系统无法查询时保留未知，两个设置宿主共用实际的启用和默认状态。
enum class InputMethodState { unknown, missing, not_default, ready };

// 本版本的文本服务和语言 profile，与 TSF 的 Globals.cpp 和看门狗读同一份 shared/contracts/msime_edition.h。
constexpr GUID input_method_clsid = MSIME_EDITION_CLSID;
constexpr GUID input_method_profile = MSIME_EDITION_PROFILE_GUID;
constexpr const wchar_t *input_method_id =
    MSIME_EDITION_LANGID_STRING L":" MSIME_EDITION_CLSID_STRING
    MSIME_EDITION_PROFILE_GUID_STRING;

struct LayoutOrTipProfile {
  DWORD profile_type;
  LANGID language;
  GUID clsid;
  GUID profile;
  GUID category;
  DWORD substitute_layout;
  DWORD flags;
  WCHAR id[MAX_PATH];
};
constexpr DWORD layout_or_tip_default = 0x0001;
constexpr DWORD layout_or_tip_disabled = 0x0002;
constexpr int input_method_repair_attempts = 3;
constexpr DWORD input_method_repair_delay_ms = 250;

constexpr bool input_method_repair_succeeded(InputMethodState state) {
  return state == InputMethodState::ready ||
         state == InputMethodState::not_default ||
         state == InputMethodState::unknown;
}

template <typename Function> inline Function input_dll_function(const char *name) {
  static const HMODULE module = []() -> HMODULE {
    // 使用系统目录绝对路径，兼容未安装安全加载扩展补丁的 Windows 7。
    wchar_t directory[MAX_PATH]{};
    const UINT length = GetSystemDirectoryW(directory, MAX_PATH);
    if (length == 0 || length >= MAX_PATH) return nullptr;
    return LoadLibraryW((std::wstring(directory, length) + L"\\input.dll").c_str());
  }();
  if (!module)
    return nullptr;
  return reinterpret_cast<Function>(
      reinterpret_cast<void *>(GetProcAddress(module, name)));
}

inline bool same_guid(GUID const &a, GUID const &b) {
  return std::memcmp(&a, &b, sizeof(GUID)) == 0;
}

inline InputMethodState input_method_state() {
  using Enumerate = UINT(WINAPI *)(LPCWSTR, LPCWSTR, LPCWSTR,
                                   LayoutOrTipProfile *, UINT);
  const auto enumerate = input_dll_function<Enumerate>("EnumEnabledLayoutOrTip");
  if (!enumerate)
    return InputMethodState::unknown;
  const UINT count = enumerate(nullptr, nullptr, nullptr, nullptr, 0);
  if (count == 0)
    return InputMethodState::unknown;
  std::vector<LayoutOrTipProfile> profiles(count);
  const UINT copied =
      std::min(enumerate(nullptr, nullptr, nullptr, profiles.data(), count), count);
  if (copied == 0)
    return InputMethodState::unknown;
  for (UINT i = 0; i < copied; ++i) {
    const auto &item = profiles[i];
    if (!same_guid(item.clsid, input_method_clsid) ||
        !same_guid(item.profile, input_method_profile))
      continue;
    if (item.flags & layout_or_tip_disabled)
      return InputMethodState::missing;
    return (item.flags & layout_or_tip_default) != 0
               ? InputMethodState::ready
               : InputMethodState::not_default;
  }
  return InputMethodState::missing;
}

inline bool add_input_method() {
  using Install = BOOL(WINAPI *)(LPCWSTR, DWORD);
  const auto install = input_dll_function<Install>("InstallLayoutOrTip");
  return install && install(input_method_id, 0) != FALSE;
}

inline bool repair_input_method() {
  for (int attempt = 0; attempt < input_method_repair_attempts; ++attempt) {
    if (add_input_method()) {
      if (input_method_repair_succeeded(input_method_state()))
        return true;
    }
    if (attempt + 1 < input_method_repair_attempts)
      Sleep(input_method_repair_delay_ms);
  }
  return false;
}

// 安装器以管理员身份运行，不能直接替当前用户写键盘列表；安装器改为以
// 原用户身份启动设置宿主，再由这里调用 input.dll。参数必须单独处理，不能
// 经过设置路由，否则重装后 profile 仍可能只是已注册但未启用。
inline int run_if_requested(const wchar_t *command) {
  if (!command || std::wcscmp(command, L"--repair-input-method") != 0)
    return -1;
  return repair_input_method() ? 0 : 1;
}

inline bool make_input_method_default() {
  using SetDefault = BOOL(WINAPI *)(LPCWSTR, DWORD);
  const auto set_default = input_dll_function<SetDefault>("SetDefaultLayoutOrTip");
  return set_default && set_default(input_method_id, 0) != FALSE;
}

}
