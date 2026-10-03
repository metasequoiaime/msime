// Supervises the preview Server: starts it, restarts it when it stops
// unexpectedly, and leaves when the Server reports a requested stop. Ported
// from the shipped watchdog; the restart rules live in WatchdogPolicy.h.
#define NOMINMAX
#include "WatchdogPolicy.h"
#include "WatchdogProtocol.h"
#include <msctf.h>
#include <shellapi.h>
#include <string>
#include <tlhelp32.h>
#include <vector>
#include <windows.h>

#include "../../../../shared/contracts/msime_edition.h"

namespace {
constexpr wchar_t server_file_name[] = L"MetasequoiaImeServer.exe";
// 互斥量带版本后缀，CLSID、profile 和语言按版本取（shared/contracts/msime_edition.h，与 TSF 的 Globals.cpp 同源）：每个版本的看门狗只看护、只等待自己版本的 TIP 和 Server。
constexpr wchar_t watchdog_mutex[] = L"Local\\MSIMEClientWatchdog.SingleInstance" MSIME_EDITION_NAME_SUFFIX;
constexpr DWORD profile_ready_timeout_milliseconds = 30'000;
constexpr DWORD profile_ready_retry_milliseconds = 1'000;
constexpr CLSID client_clsid = MSIME_EDITION_CLSID;
constexpr GUID client_profile = MSIME_EDITION_PROFILE_GUID;
constexpr LANGID client_language = MSIME_EDITION_LANGID;

struct Apartment {
  bool owned = false;
  Apartment() {
    const HRESULT entered = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
    owned = SUCCEEDED(entered);
  }
  ~Apartment() {
    if (owned)
      CoUninitialize();
  }
  Apartment(const Apartment &) = delete;
  Apartment &operator=(const Apartment &) = delete;
};

// Fail closed: without a confirmed profile in this user's keyboard list there
// is nothing to supervise, and an unnecessary background process is worse than
// none.
bool profile_enabled_for_current_user() {
  Apartment apartment;
  if (!apartment.owned)
    return false;
  ITfInputProcessorProfiles *profiles = nullptr;
  const HRESULT created = CoCreateInstance(
      CLSID_TF_InputProcessorProfiles, nullptr, CLSCTX_INPROC_SERVER,
      IID_ITfInputProcessorProfiles, reinterpret_cast<void **>(&profiles));
  BOOL enabled = FALSE;
  const HRESULT queried =
      SUCCEEDED(created) ? profiles->IsEnabledLanguageProfile(
                               client_clsid, client_language, client_profile,
                               &enabled)
                         : created;
  if (profiles)
    profiles->Release();
  return queried == S_OK && enabled != FALSE;
}

// At logon the scheduler can start this before TSF has published the user's
// profile list, so a first "not installed" answer is retried rather than
// treated as final.
bool wait_for_profile_enabled() {
  const ULONGLONG deadline =
      GetTickCount64() + profile_ready_timeout_milliseconds;
  for (;;) {
    if (profile_enabled_for_current_user())
      return true;
    if (GetTickCount64() >= deadline)
      return false;
    Sleep(profile_ready_retry_milliseconds);
  }
}

std::wstring executable_directory() {
  std::vector<wchar_t> path(32768);
  const DWORD length =
      GetModuleFileNameW(nullptr, path.data(), static_cast<DWORD>(path.size()));
  if (!length || length == path.size())
    return {};
  std::wstring full(path.data(), length);
  const auto separator = full.find_last_of(L'\\');
  return separator == std::wstring::npos ? std::wstring{}
                                         : full.substr(0, separator);
}

// Only adopt a Server started from the same installation; another copy on this
// machine is not ours to supervise.
HANDLE find_running_server(const std::wstring &expected_path) {
  HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
  if (snapshot == INVALID_HANDLE_VALUE)
    return nullptr;
  PROCESSENTRY32W entry{};
  entry.dwSize = sizeof(entry);
  HANDLE found = nullptr;
  if (Process32FirstW(snapshot, &entry)) {
    do {
      if (_wcsicmp(entry.szExeFile, server_file_name) != 0)
        continue;
      HANDLE process =
          OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE,
                      entry.th32ProcessID);
      if (!process)
        continue;
      std::vector<wchar_t> path(32768);
      DWORD length = static_cast<DWORD>(path.size());
      if (QueryFullProcessImageNameW(process, 0, path.data(), &length) &&
          _wcsicmp(std::wstring(path.data(), length).c_str(),
                   expected_path.c_str()) == 0) {
        found = process;
        break;
      }
      CloseHandle(process);
    } while (Process32NextW(snapshot, &entry));
  }
  CloseHandle(snapshot);
  return found;
}

HANDLE start_server(const std::wstring &server_path,
                    const std::wstring &working_directory) {
  // The Server runs with uiAccess, which CreateProcess refuses; the shell verb
  // is the same path an Explorer launch takes.
  SHELLEXECUTEINFOW execute{};
  execute.cbSize = sizeof(execute);
  execute.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI;
  execute.lpVerb = L"open";
  execute.lpFile = server_path.c_str();
  execute.lpParameters = msime::windows::watchdog_protocol::managed_argument;
  execute.lpDirectory = working_directory.c_str();
  execute.nShow = SW_SHOWNOACTIVATE;
  if (!ShellExecuteExW(&execute) || !execute.hProcess)
    return nullptr;
  return execute.hProcess;
}
} // namespace

int WINAPI wWinMain(HINSTANCE, HINSTANCE, PWSTR command_line, int) {
  if (command_line && std::wstring(command_line) == L"--help")
    return 0;
  HANDLE mutex = CreateMutexW(nullptr, FALSE, watchdog_mutex);
  if (!mutex)
    return 1;
  if (GetLastError() == ERROR_ALREADY_EXISTS) {
    CloseHandle(mutex);
    return 0;
  }
  struct MutexGuard {
    HANDLE handle;
    ~MutexGuard() { CloseHandle(handle); }
  } guard{mutex};
  if (!wait_for_profile_enabled())
    return 0;
  const std::wstring directory = executable_directory();
  if (directory.empty())
    return 1;
  const std::wstring server_path = directory + L"\\" + server_file_name;
  uint32_t delay = msime::windows::watchdog::initial_restart_delay_milliseconds;
  for (;;) {
    HANDLE server = find_running_server(server_path);
    if (!server)
      server = start_server(server_path, directory);
    if (!server) {
      const auto decision = msime::windows::watchdog_after_failed_start(delay);
      delay = decision.delay_milliseconds;
      Sleep(delay);
      continue;
    }
    const ULONGLONG started_at = GetTickCount64();
    WaitForSingleObject(server, INFINITE);
    DWORD exit_code = 0;
    GetExitCodeProcess(server, &exit_code);
    CloseHandle(server);
    const auto decision = msime::windows::watchdog_after_exit(
        exit_code, GetTickCount64() - started_at, delay);
    if (!decision.keep_running)
      return 0;
    delay = decision.delay_milliseconds;
    Sleep(delay);
  }
}
