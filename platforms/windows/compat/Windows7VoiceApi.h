#pragma once

#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <pathcch.h>
#include <dxgi1_3.h>
#include <string_view>

namespace msime::windows::compat {

// 只用于 Win7 语音运行库；现代包仍使用上游未经修改的运行库。
struct VoiceApi {
  using Create = HANDLE(WINAPI*)(LPCWSTR, DWORD, DWORD, DWORD, LPCREATEFILE2_EXTENDED_PARAMETERS);
  using PreciseTime = VOID(WINAPI*)(LPFILETIME);
  using Mitigation = BOOL(WINAPI*)(HANDLE, PROCESS_MITIGATION_POLICY, PVOID, SIZE_T);
  using Path = HRESULT(WINAPI*)(PWSTR, size_t);
  using Factory = HRESULT(WINAPI*)(UINT, REFIID, void**);
  Create create = nullptr;
  PreciseTime precise_time = nullptr;
  Mitigation mitigation = nullptr;
  Path remove_backslash = nullptr;
  Path remove_file_spec = nullptr;
  Factory factory2 = nullptr;

  HRESULT create_factory(UINT flags, REFIID iid, void** output) const {
    if (factory2) return factory2(flags, iid, output);
    if (!output) return E_POINTER;
    *output = nullptr;
    // Win7 的 DXGI 1.1 仍提供真实工厂；不伪造新接口或忽略无法兑现的调试标志。
    if (flags) return DXGI_ERROR_UNSUPPORTED;
    return CreateDXGIFactory1(iid, output);
  }

  HANDLE create_file(LPCWSTR name, DWORD access, DWORD share, DWORD disposition,
                     LPCREATEFILE2_EXTENDED_PARAMETERS parameters) const {
    if (create) return create(name, access, share, disposition, parameters);
    if (parameters && parameters->dwSize != sizeof(CREATEFILE2_EXTENDED_PARAMETERS)) {
      SetLastError(ERROR_INVALID_PARAMETER);
      return INVALID_HANDLE_VALUE;
    }
    // CreateFile2 只是把这些 CreateFileW 参数拆进结构体，不能丢掉共享、目录或安全选项。
    return CreateFileW(name, access, share, parameters ? parameters->lpSecurityAttributes : nullptr,
        disposition, parameters ? parameters->dwFileAttributes | parameters->dwFileFlags |
            parameters->dwSecurityQosFlags : FILE_ATTRIBUTE_NORMAL,
        parameters ? parameters->hTemplateFile : nullptr);
  }

  void system_time(LPFILETIME output) const {
    if (precise_time) precise_time(output);
    else GetSystemTimeAsFileTime(output);
  }

  BOOL process_mitigation(HANDLE process, PROCESS_MITIGATION_POLICY policy, PVOID output, SIZE_T size) const {
    if (mitigation) return mitigation(process, policy, output, size);
    // Win7 没有这项查询：保留真实失败，不伪造一份“已成功读取”的策略。
    SetLastError(ERROR_CALL_NOT_IMPLEMENTED);
    return FALSE;
  }

  static HRESULT path_view(PWSTR path, size_t capacity, std::wstring_view& view) {
    if (!path || !capacity || capacity > PATHCCH_MAX_CCH) return E_INVALIDARG;
    size_t length = 0;
    while (length < capacity && path[length]) ++length;
    if (length == capacity) return E_INVALIDARG;
    view = std::wstring_view(path, length);
    return S_OK;
  }

  static size_t root_length(std::wstring_view path) {
    size_t start = 0;
    if (path.substr(0, 4) == L"\\\\?\\") {
      start = 4;
      if (path.substr(start, 4) == L"UNC\\" || path.substr(start, 4) == L"unc\\") start = 8;
      else if (path.size() >= 6 && path[5] == L':')
        return path.size() >= 7 && path[6] == L'\\' ? 7 : 6;
      else {
        const auto separator = path.find(L'\\', start);
        return separator == path.npos ? path.size() : separator + 1;
      }
    } else if (path.size() >= 2 && path[1] == L':') {
      return path.size() >= 3 && path[2] == L'\\' ? 3 : 2;
    } else if (path.substr(0, 2) == L"\\\\") start = 2;
    else return !path.empty() && path[0] == L'\\' ? 1 : 0;
    const auto server = path.find(L'\\', start);
    if (server == path.npos) return path.size();
    const auto share = path.find(L'\\', server + 1);
    return share == path.npos ? path.size() : share;
  }

  HRESULT path_remove_backslash(PWSTR path, size_t capacity) const {
    if (remove_backslash) return remove_backslash(path, capacity);
    std::wstring_view view;
    const HRESULT valid = path_view(path, capacity, view);
    if (FAILED(valid)) return valid;
    if (view.size() <= root_length(view) || view.back() != L'\\') return S_FALSE;
    path[view.size() - 1] = L'\0';
    return S_OK;
  }

  HRESULT path_remove_file_spec(PWSTR path, size_t capacity) const {
    if (remove_file_spec) return remove_file_spec(path, capacity);
    std::wstring_view view;
    const HRESULT valid = path_view(path, capacity, view);
    if (FAILED(valid)) return valid;
    const size_t root = root_length(view);
    if (view.size() <= root) return S_FALSE;
    const auto separator = view.find_last_of(L'\\');
    path[separator == view.npos || separator < root ? root : separator] = L'\0';
    return S_OK;
  }
};

template <class Function>
inline Function voice_function(HMODULE module, const char* name) {
  return module ? reinterpret_cast<Function>(reinterpret_cast<void*>(GetProcAddress(module, name))) : nullptr;
}

inline const VoiceApi& voice_api() {
  static const VoiceApi api = [] {
    VoiceApi loaded;
    const HMODULE kernel = GetModuleHandleW(L"kernel32.dll");
    loaded.create = voice_function<VoiceApi::Create>(kernel, "CreateFile2");
    loaded.precise_time = voice_function<VoiceApi::PreciseTime>(kernel, "GetSystemTimePreciseAsFileTime");
    loaded.mitigation = voice_function<VoiceApi::Mitigation>(kernel, "GetProcessMitigationPolicy");
    loaded.factory2 = voice_function<VoiceApi::Factory>(GetModuleHandleW(L"dxgi.dll"), "CreateDXGIFactory2");
    // 旧系统没有 PathCch；只从系统目录加载，模块在进程生命周期内保留。
    wchar_t location[MAX_PATH + 16]{};
    const UINT length = GetSystemDirectoryW(location, MAX_PATH);
    HMODULE paths = nullptr;
    if (length && length < MAX_PATH) {
      constexpr wchar_t suffix[] = L"\\pathcch.dll";
      for (size_t index = 0; index < sizeof(suffix) / sizeof(wchar_t); ++index)
        location[length + index] = suffix[index];
      paths = LoadLibraryExW(location, nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
    }
    loaded.remove_backslash = voice_function<VoiceApi::Path>(paths, "PathCchRemoveBackslash");
    loaded.remove_file_spec = voice_function<VoiceApi::Path>(paths, "PathCchRemoveFileSpec");
    return loaded;
  }();
  return api;
}

} // namespace msime::windows::compat
