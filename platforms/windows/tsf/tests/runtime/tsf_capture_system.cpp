#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <werapi.h>
#include "../../CrashCaptureContext.h"
#include "../../../../../shared/contracts/msime_edition.h"
#include <nlohmann/json.hpp>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>

namespace {
using msime::tsf::wer::Context;
void require(bool value, const char *message) { if (!value) throw std::runtime_error(message); }
template<class Function> Function load(HMODULE module, const char *name) {
  const auto raw = GetProcAddress(module, name);
  require(raw != nullptr, name);
  Function function = nullptr;
  static_assert(sizeof(function) == sizeof(raw));
  std::memcpy(&function, &raw, sizeof(function));
  return function;
}
template<class Predicate> void until(Predicate predicate, const char *message) {
  const auto end = GetTickCount64() + 8000;
  do { if (predicate()) return; Sleep(10); } while (GetTickCount64() < end);
  throw std::runtime_error(message);
}
__declspec(noinline) void external_crash() {
  volatile std::uintptr_t address = 1;
  *reinterpret_cast<volatile unsigned *>(address) = 42;
}
bool caught(void (*crash)()) {
#if defined(_MSC_VER)
  __try { crash(); }
  __except (GetExceptionCode() == EXCEPTION_ACCESS_VIOLATION ? EXCEPTION_EXECUTE_HANDLER : EXCEPTION_CONTINUE_SEARCH) {
    return true;
  }
#else
  static_cast<void>(crash);
#endif
  return false;
}
void run(const std::string &mode, const std::filesystem::path &root) {
  require(mode == "tip" || mode == "host" || mode == "external" || mode == "withdrawn" ||
          mode == "reopened" || mode == "released" || mode == "caught", "unknown system WER case");
  require(root.is_absolute() && std::filesystem::is_directory(root), "synthetic output directory missing");
  require(SetEnvironmentVariableW(L"LOCALAPPDATA", root.c_str()) &&
          SetEnvironmentVariableW(L"MSIME_TSF_TEST_PREFERENCES", root.c_str()) &&
          SetEnvironmentVariableW(L"MSIME_TSF_TEST_SYSTEM_WER", L"1"), "synthetic environment failed");
  // 自动化宿主可能继承禁用 WER 的错误模式；只在本合成子进程清除此位，不能改加载 TIP 的真实宿主。
  const auto inherited_error_mode = GetErrorMode();
  SetErrorMode(inherited_error_mode & ~SEM_NOGPFAULTERRORBOX);
  require((GetErrorMode() & SEM_NOGPFAULTERRORBOX) == 0, "synthetic process still suppresses WER");
  // 关闭弹窗用 WER 自己的标志，不能用 SEM_NOGPFAULTERRORBOX 把整个系统投递路径也关掉。
  require(SUCCEEDED(WerSetFlags(WER_FAULT_REPORTING_FLAG_QUEUE | WER_FAULT_REPORTING_FLAG_NOHEAP |
                              WER_FAULT_REPORTING_NO_UI)), "WER flags failed");
  wchar_t buffer[32768]{};
  const auto length = GetModuleFileNameW(nullptr, buffer, 32768);
  require(length && length < 32768, "test executable path unavailable");
  const auto directory = std::filesystem::path(buffer).parent_path();
  const HMODULE host = LoadLibraryW((directory / MSIME_EDITION_HOST_DLL).c_str());
  require(host != nullptr, "synthetic Host API load failed");
  const auto initialize = load<void (*)(const char *)>(host, "msime_test_host_initialize");
  const auto finish = load<void (*)()>(host, "msime_test_host_finish");
  const auto host_mode = load<void (*)(int)>(host, "msime_test_host_mode");
  const auto host_crash = load<void (*)()>(host, "msime_test_host_crash");
  const auto tip_path = directory / L"MetasequoiaImeTsfLeaseTest.dll";
  const HMODULE tip = LoadLibraryW(tip_path.c_str());
  require(tip != nullptr, "synthetic TIP load failed");
  const auto acquire = load<void *(*)()>(tip, "msime_test_tsf_acquire");
  const auto release = load<void (*)(void *)>(tip, "msime_test_tsf_release");
  const auto snapshot = load<bool (*)(Context *)>(tip, "msime_test_tsf_snapshot");
  const auto tip_crash = load<void (*)()>(tip, "msime_test_tsf_crash");
  initialize(root.u8string().c_str());
  void *lease = acquire();
  Context first{};
  until([&] { return snapshot(&first) && first.armed == 1; }, "real WER registration did not arm TIP");
  Context current = first;
  if (mode == "withdrawn" || mode == "reopened") {
    host_mode(0);
    until([&] { return snapshot(&current) && current.armed == 0; }, "withdrawal did not disarm TIP");
    if (mode == "reopened") {
      host_mode(1);
      until([&] { return snapshot(&current) && current.armed == 1 &&
          std::strcmp(current.header, first.header) != 0; }, "reopen reused withdrawn capture");
    }
  }
  const bool record_expected = mode == "tip" || mode == "host" || mode == "reopened";
  // 致命故障前关闭夹具文件；预期头来自生产控制器发布的快照。
  {
    nlohmann::json expected{{"mode", mode}, {"record_expected", record_expected},
      {"inherited_error_mode", inherited_error_mode}, {"effective_error_mode", GetErrorMode()},
      {"path", std::filesystem::path(std::u16string(current.path)).u8string()},
      {"initial_path", std::filesystem::path(std::u16string(first.path)).u8string()},
      {"header", current.header}, {"module", mode == "host" ? current.modules[1].name : current.modules[0].name}};
    std::ofstream stream(root / "expected.json", std::ios::binary);
    stream << expected.dump(2);
    stream.close();
    require(!stream.fail(), "expected snapshot could not be saved");
  }
  if (mode == "caught") require(caught(tip_crash), "synthetic exception was not caught by host");
  if (mode == "released" || mode == "caught") {
    release(lease);
    require(FreeLibrary(tip) != FALSE, "TIP unload failed");
    until([] { return GetModuleHandleW(L"MetasequoiaImeTsfLeaseTest.dll") == nullptr; }, "TIP did not unregister and unload");
    finish();
    require(FreeLibrary(host) != FALSE, "Host API unload failed");
    if (mode == "caught") return;
    external_crash();
  } else if (mode == "external") external_crash();
  else if (mode == "host") host_crash();
  else tip_crash();
  throw std::runtime_error("fatal fault unexpectedly returned");
}
}
int wmain(int argc, wchar_t **argv) {
  try {
    require(argc == 3, "usage: system-test MODE ABSOLUTE-SYNTHETIC-DIRECTORY");
    run(std::filesystem::path(argv[1]).u8string(), std::filesystem::path(argv[2]));
    return 0;
  } catch (const std::exception &error) {
    std::fprintf(stderr, "FAIL: %s\n", error.what());
    return 1;
  }
}
