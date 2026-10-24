#include "LocalAsr.h"
#include <windows.h>
#include <cassert>
#include <cstdio>
#include <filesystem>
#include <string>

int wmain(int count, wchar_t** arguments) {
  assert(count == 2);
  namespace fs = std::filesystem;
  const auto runtime = fs::absolute(arguments[1]);
  // 真实公共入口接收前端或 CMake 常用的正斜杠路径，不能换成裸 LoadLibrary 来验收。
  msime::voice::set_sherpa_library_path(runtime.generic_u8string());
  if (!msime::voice::sherpa_runtime_available()) {
    std::fputs(msime::voice::sherpa_runtime_error().c_str(), stderr);
    return 1;
  }
  const HMODULE dependency = GetModuleHandleW(L"onnxruntime.dll");
  assert(dependency);
  wchar_t loaded[32768]{};
  const DWORD length = GetModuleFileNameW(dependency, loaded, 32768);
  assert(length && length < 32768);
  assert(fs::equivalent(fs::path(loaded), runtime.parent_path() / L"onnxruntime.dll"));
  std::puts("Shared ASR Windows runtime: generic path loads the actual adjacent ONNX dependency");
}
