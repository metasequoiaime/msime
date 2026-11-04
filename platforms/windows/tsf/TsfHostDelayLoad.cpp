#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include "TsfHostDelayLoad.h"
#include "ModulePath.h"

#include <windows.h>
#include <delayimp.h>

#include <cstring>
#include <string>

#include "msime_edition.h"

#ifndef MSIME_TSF_HOST_DLL_NAME
#define MSIME_TSF_HOST_DLL_NAME MSIME_EDITION_HOST_DLL
#endif

namespace {

std::wstring module_path(HMODULE module) noexcept {
  return msime::tsf::ReadModulePath([module](wchar_t *buffer, unsigned capacity) {
    return GetModuleFileNameW(module, buffer, capacity);
  });
}

HMODULE load_host_library() noexcept {
  HMODULE self = nullptr;
  if (!GetModuleHandleExW(
          GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
              GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
          reinterpret_cast<LPCWSTR>(&load_host_library), &self)) {
    return nullptr;
  }
  const auto path = msime::windows::host_library_path(
      module_path(self), MSIME_TSF_HOST_DLL_NAME);
  if (path.empty()) return nullptr;
  return LoadLibraryExW(path.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
}

}  // 匿名命名空间

// 沉浸式系统进程不会把 TSF 所在目录加入普通 DLL 搜索路径；宿主库必须
// 从与 TSF 相同的安装目录加载，不能退回当前目录或 PATH。
extern "C" FARPROC WINAPI msime_tsf_host_delay_notify(
    unsigned event, PDelayLoadInfo info) noexcept {
  if (event != dliNotePreLoadLibrary || info == nullptr ||
      info->szDll == nullptr) {
    return nullptr;
  }
  const auto *expected = MSIME_TSF_HOST_DLL_NAME;
  std::string expected_narrow;
  for (const wchar_t *cursor = expected; *cursor != L'\0'; ++cursor) {
    if (*cursor > 0x7f) return nullptr;
    expected_narrow.push_back(static_cast<char>(*cursor));
  }
  if (_stricmp(info->szDll, expected_narrow.c_str()) != 0) return nullptr;
  const HMODULE module = load_host_library();
  if (module == nullptr) {
    // 绝对路径加载失败时不能回退到当前目录或 PATH，避免把错误版本的
    // 同名 DLL 注入 TSF 进程。
    RaiseException(VcppException(ERROR_SEVERITY_ERROR, ERROR_MOD_NOT_FOUND),
                   0, 0, nullptr);
  }
  return reinterpret_cast<FARPROC>(module);
}

extern "C" {
#ifdef _MSC_VER
extern const PfnDliHook __pfnDliNotifyHook2 = msime_tsf_host_delay_notify;
#else
PfnDliHook __pfnDliNotifyHook2 = msime_tsf_host_delay_notify;
#endif
}
