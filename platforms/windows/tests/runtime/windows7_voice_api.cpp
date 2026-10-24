#include "../../compat/Windows7VoiceApi.h"

#include <cassert>
#include <cstdio>
#include <string>
#include <vector>

namespace {
UINT factory_flags = 0;
HRESULT WINAPI synthetic_factory(UINT flags, REFIID iid, void** output) {
  assert(iid == __uuidof(IDXGIFactory1)); factory_flags = flags;
  *output = reinterpret_cast<void*>(1); return S_FALSE;
}
}

int main() {
  using msime::windows::compat::VoiceApi;
  const VoiceApi fallback;
  const auto& native = msime::windows::compat::voice_api();
  IDXGIFactory1* factory = nullptr; IDXGIFactory1* expected_factory = nullptr;
  const auto actual_factory = fallback.create_factory(0, __uuidof(IDXGIFactory1), reinterpret_cast<void**>(&factory));
  const auto expected_result = CreateDXGIFactory1(__uuidof(IDXGIFactory1), reinterpret_cast<void**>(&expected_factory));
  assert(actual_factory == expected_result && (factory != nullptr) == (expected_factory != nullptr));
  if (factory) factory->Release(); if (expected_factory) expected_factory->Release();
  void* output = reinterpret_cast<void*>(1);
  assert(fallback.create_factory(DXGI_CREATE_FACTORY_DEBUG, __uuidof(IDXGIFactory1), &output) == DXGI_ERROR_UNSUPPORTED && output == nullptr);
  assert(fallback.create_factory(0, __uuidof(IDXGIFactory1), nullptr) == E_POINTER);
  VoiceApi forwarding; forwarding.factory2 = synthetic_factory;
  assert(forwarding.create_factory(123, __uuidof(IDXGIFactory1), &output) == S_FALSE && factory_flags == 123 && output == reinterpret_cast<void*>(1));
  assert(reinterpret_cast<FARPROC>(native.factory2) == GetProcAddress(GetModuleHandleW(L"dxgi.dll"), "CreateDXGIFactory2"));
  const std::vector<std::wstring> paths{
      L"", L"synthetic", L"synthetic\\", L"synthetic\\child", L"C:", L"C:synthetic",
      L"C:\\", L"C:\\synthetic", L"C:\\synthetic\\", L"C:\\synthetic\\child",
      L"\\", L"\\synthetic", L"\\\\synthetic\\share", L"\\\\synthetic\\share\\",
      L"\\\\synthetic\\share\\child", L"\\\\?\\C:\\", L"\\\\?\\C:\\synthetic",
      L"\\\\?\\UNC\\synthetic\\share", L"\\\\?\\UNC\\synthetic\\share\\child",
      L"C:\\" + std::wstring(400, L'x') + L"\\synthetic"};
  // 缺失接口的路径行为与当前 Windows 系统接口逐项对照，覆盖根、UNC 和长路径。
  for (const auto& original : paths) {
    for (bool file_spec : {false, true}) {
      std::vector<wchar_t> actual(original.begin(), original.end());
      actual.push_back(L'\0');
      auto expected = actual;
      const HRESULT got = file_spec ? fallback.path_remove_file_spec(actual.data(), actual.size()) :
          fallback.path_remove_backslash(actual.data(), actual.size());
      const auto function = file_spec ? native.remove_file_spec : native.remove_backslash;
      if (function) {
        const HRESULT wanted = function(expected.data(), expected.size());
        if (got != wanted || std::wstring(actual.data()) != std::wstring(expected.data())) {
          std::fprintf(stderr, "Path compatibility mismatch: sample %zu, operation %d, HRESULT %lx / %lx\n",
              static_cast<size_t>(&original - paths.data()), file_spec, static_cast<unsigned long>(got), static_cast<unsigned long>(wanted));
          return 1;
        }
      }
    }
  }
  assert(fallback.path_remove_file_spec(nullptr, 0) == E_INVALIDARG);
  wchar_t unterminated[]{L'x', L'y'};
  assert(FAILED(fallback.path_remove_backslash(unterminated, 2)));
  assert(unterminated[0] == L'x' && unterminated[1] == L'y');

  FILETIME before{}, now{}, after{};
  GetSystemTimeAsFileTime(&before);
  fallback.system_time(&now);
  GetSystemTimeAsFileTime(&after);
  const auto ticks = [](FILETIME value) { return (static_cast<ULONGLONG>(value.dwHighDateTime) << 32) | value.dwLowDateTime; };
  assert(ticks(before) <= ticks(now) && ticks(now) <= ticks(after));
  DWORD policy = 0xdeadbeef;
  assert(!fallback.process_mitigation(GetCurrentProcess(), ProcessSystemCallDisablePolicy, &policy, sizeof(policy)));
  assert(GetLastError() == ERROR_CALL_NOT_IMPLEMENTED && policy == 0xdeadbeef);

  wchar_t root[MAX_PATH]{};
  wchar_t file[MAX_PATH]{};
  const DWORD length = GetTempPathW(MAX_PATH, root);
  assert(length && length < MAX_PATH);
  assert(GetTempFileNameW(root, L"m7v", 0, file));
  CREATEFILE2_EXTENDED_PARAMETERS parameters{};
  parameters.dwSize = sizeof(parameters);
  parameters.dwFileFlags = FILE_FLAG_SEQUENTIAL_SCAN;
  HANDLE handle = fallback.create_file(file, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ, OPEN_EXISTING, &parameters);
  assert(handle != INVALID_HANDLE_VALUE);
  const char payload[] = "synthetic-runtime-fixture";
  DWORD written = 0;
  assert(WriteFile(handle, payload, sizeof(payload), &written, nullptr) && written == sizeof(payload));
  HANDLE denied = fallback.create_file(file, GENERIC_WRITE, FILE_SHARE_READ, OPEN_EXISTING, nullptr);
  assert(denied == INVALID_HANDLE_VALUE && GetLastError() == ERROR_SHARING_VIOLATION);
  assert(CloseHandle(handle));
  assert(DeleteFileW(file));
  parameters.dwFileFlags = FILE_FLAG_BACKUP_SEMANTICS;
  handle = fallback.create_file(root, FILE_READ_ATTRIBUTES, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
      OPEN_EXISTING, &parameters);
  assert(handle != INVALID_HANDLE_VALUE && CloseHandle(handle));
  parameters.dwSize = 0;
  assert(fallback.create_file(root, GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING, &parameters) == INVALID_HANDLE_VALUE);
  assert(GetLastError() == ERROR_INVALID_PARAMETER);
  std::puts("Win7 voice API: DXGI factory forwarding/fallback, native path comparison, real file handles, clock and absent policy passed");
}
