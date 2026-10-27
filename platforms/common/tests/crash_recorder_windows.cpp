#include "../CrashRecorder.h"
#include <windows.h>
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>

namespace {
constexpr char header[] = "MSIME-CRASH/1 {\"component\":\"synthetic-test\"}\n";

std::string read(const std::filesystem::path &path) {
  std::ifstream file(path);
  return {std::istreambuf_iterator<char>(file), {}};
}

DWORD child(const std::wstring &exe, const std::wstring &mode, const std::filesystem::path &record) {
  auto command = L"\"" + exe + L"\" " + mode + L" \"" + record.wstring() + L"\"";
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  PROCESS_INFORMATION process{};
  assert(CreateProcessW(exe.c_str(), command.data(), nullptr, nullptr, FALSE, 0, nullptr, nullptr, &startup, &process));
  assert(WaitForSingleObject(process.hProcess, 30000) == WAIT_OBJECT_0);
  DWORD code = 0;
  assert(GetExitCodeProcess(process.hProcess, &code));
  CloseHandle(process.hThread);
  CloseHandle(process.hProcess);
  return code;
}
}

int wmain(int argc, wchar_t **argv) {
  SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
  if (argc == 3) {
    const auto path = std::filesystem::path(argv[2]).u8string();
    msime_native_crash_install();
    msime_native_crash_arm(path.c_str(), header);
    if (std::wstring(argv[1]) == L"disabled") msime_native_crash_disarm();
    if (std::wstring(argv[1]) == L"first") msime_native_crash_record("Synthetic terminate");
    RaiseException(EXCEPTION_ACCESS_VIOLATION, EXCEPTION_NONCONTINUABLE, 0, nullptr);
    return 1;
  }
  wchar_t exe[32768]{};
  assert(GetModuleFileNameW(nullptr, exe, static_cast<DWORD>(std::size(exe))) > 0);
  const auto root = std::filesystem::temp_directory_path() / ("msime-crash-recorder-" + std::to_string(GetCurrentProcessId()));
  assert(std::filesystem::create_directory(root));
  const auto fatal = root / "fatal.crash";
  assert(child(exe, L"fatal", fatal) == EXCEPTION_ACCESS_VIOLATION);
  const auto contents = read(fatal);
  assert(contents.rfind(header, 0) == 0);
  assert(contents.find("EXCEPTION_ACCESS_VIOLATION (0xc0000005)") != std::string::npos);
  assert(contents.find(std::filesystem::path(exe).filename().string() + "+0x") != std::string::npos);
  const auto disabled = root / "disabled.crash";
  assert(child(exe, L"disabled", disabled) == EXCEPTION_ACCESS_VIOLATION);
  assert(!std::filesystem::exists(disabled));
  const auto first = root / "first.crash";
  assert(child(exe, L"first", first) == EXCEPTION_ACCESS_VIOLATION);
  assert(read(first).find("Synthetic terminate\n") != std::string::npos);
  assert(read(first).find("EXCEPTION_ACCESS_VIOLATION") == std::string::npos);
  std::filesystem::remove_all(root);
  return 0;
}
