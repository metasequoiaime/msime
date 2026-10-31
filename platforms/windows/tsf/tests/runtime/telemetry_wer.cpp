#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <werapi.h>
#include "../../CrashCaptureContext.h"
#include "../../WerApiCompatibility.h"
#include <cstdio>
#include <cstring>
#include <cwchar>
#include <filesystem>
#include <fstream>
#include <stdexcept>
#include <string>

namespace {
using msime::tsf::wer::Context;
void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}
template <class Function> Function load(HMODULE module, const char *name) {
  const auto raw = GetProcAddress(module, name);
  require(raw != nullptr, name);
  Function result = nullptr;
  static_assert(sizeof(raw) == sizeof(result));
  std::memcpy(&result, &raw, sizeof(result));
  return result;
}
std::string bytes(const std::filesystem::path &file) {
  std::ifstream stream(file, std::ios::binary);
  return {std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>()};
}
struct Fixture {
  std::filesystem::path directory;
  std::filesystem::path path;
  std::filesystem::path temporary;
  HMODULE module = nullptr;
  PROCESS_INFORMATION child{};
  void *remote = nullptr;
#if defined(_M_IX86) || defined(__i386__)
  void *remote_frames = nullptr;
#endif
  msime::tsf::wer::ExceptionEvent event = nullptr;
  Context context{};
  WER_RUNTIME_EXCEPTION_INFORMATION information{};

  void initialize() {
    wchar_t buffer[1024]{};
    const DWORD length = GetTempPathW(1024, buffer);
    require(length && length < 1024, "temporary directory unavailable");
    directory = std::filesystem::path(buffer) /
        (L"msime-wer-synthetic-" + std::to_wstring(GetCurrentProcessId()) + L"-" + std::to_wstring(GetTickCount64()));
    require(std::filesystem::create_directory(directory), "temporary directory collision");
    path = directory / L"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.crash";
    temporary = path.wstring() + L".werpartial";
    const DWORD size = GetModuleFileNameW(nullptr, buffer, 1024);
    require(size && size < 1024, "test executable path unavailable");
    module = LoadLibraryW((std::filesystem::path(buffer).parent_path() / L"MetasequoiaImeCrashWer.dll").c_str());
    require(module != nullptr, "WER helper load failed");
    event = load<msime::tsf::wer::ExceptionEvent>(module, "OutOfProcessExceptionEventCallback");
    std::wstring command = L"\"" + std::wstring(buffer) + L"\" --wait";
    STARTUPINFOW startup{};
    startup.cb = sizeof(startup);
    require(CreateProcessW(nullptr, command.data(), nullptr, nullptr, FALSE, CREATE_NO_WINDOW,
                            nullptr, nullptr, &startup, &child) != FALSE, "synthetic target start failed");
    remote = VirtualAllocEx(child.hProcess, nullptr, sizeof(Context), MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    require(remote != nullptr, "synthetic remote context allocation failed");
#if defined(_M_IX86) || defined(__i386__)
    remote_frames = VirtualAllocEx(child.hProcess, nullptr, 3 * sizeof(DWORD) * 2,
        MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    require(remote_frames != nullptr, "synthetic remote frame allocation failed");
#endif
    context.sequence = 2;
    context.armed = 1;
    std::memcpy(context.path, path.c_str(), (path.wstring().size() + 1) * sizeof(wchar_t));
    std::strcpy(context.header, "MSIME-CRASH/1 {\"component\":\"tsf\",\"version\":\"0.0.0\"}\n");
    context.modules[0].base = 0x10000;
    context.modules[0].size = 0x1000;
    std::strcpy(context.modules[0].name, "MetasequoiaImeTsf.dll");
    context.modules[1].base = 0x20000;
    context.modules[1].size = 0x1000;
    std::strcpy(context.modules[1].name, "msime_host_api_wubi.dll");
    information.dwSize = sizeof(information);
    information.hProcess = child.hProcess;
    information.hThread = child.hThread;
    information.bIsFatal = TRUE;
    information.exceptionRecord.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
    information.exceptionRecord.ExceptionAddress = reinterpret_cast<void *>(0x10123);
  }
  ~Fixture() {
    if (child.hProcess) {
      TerminateProcess(child.hProcess, 0);
      WaitForSingleObject(child.hProcess, 5000);
      CloseHandle(child.hProcess);
      CloseHandle(child.hThread);
    }
    if (module) FreeLibrary(module);
    std::error_code error;
    std::filesystem::remove_all(directory, error);
  }
  void publish() {
    SIZE_T written = 0;
    require(WriteProcessMemory(child.hProcess, remote, &context, sizeof(context), &written) &&
            written == sizeof(context), "synthetic remote context write failed");
  }
#if defined(_M_IX86) || defined(__i386__)
  void publish_frames() {
    struct Link { DWORD previous; DWORD address; } links[3]{};
    const auto base = reinterpret_cast<std::uintptr_t>(remote_frames);
    links[0] = {static_cast<DWORD>(base + sizeof(Link)), 0x10150};
    links[1] = {static_cast<DWORD>(base + 2 * sizeof(Link)), 0x30000};
    links[2] = {0, 0x20010};
    SIZE_T written = 0;
    require(WriteProcessMemory(child.hProcess, remote_frames, links, sizeof(links), &written) &&
            written == sizeof(links), "synthetic remote frame write failed");
    context.sequence = 2;
    context.armed = 1;
    context.size = sizeof(Context);
    context.magic = msime::tsf::wer::context_magic;
    context.modules[0].base = 0x10000;
    context.modules[0].size = 0x1000;
    std::strcpy(context.modules[0].name, "MetasequoiaImeTsf.dll");
    context.modules[1].base = 0x20000;
    context.modules[1].size = 0x1000;
    std::strcpy(context.modules[1].name, "msime_host_api_wubi.dll");
    information.context.Ebp = static_cast<DWORD>(base);
  }
#endif
  void invoke(void *address = nullptr) {
    publish();
    BOOL ownership = TRUE;
    wchar_t name[] = L"preserve-host-event";
    DWORD size = 23;
    DWORD count = 7;
    require(event(address ? address : remote, &information, &ownership, name, &size, &count) == S_OK,
            "callback result changed host behavior");
    require(ownership == FALSE, "callback claimed native WER event");
    require(std::wcscmp(name, L"preserve-host-event") == 0, "callback changed native WER event name");
    require(size == 23, "callback changed native WER event name capacity");
    require(count == 7, "callback changed native WER signature count");
  }
  void rejects(const char *message) {
    invoke();
    require(!std::filesystem::exists(path) && !std::filesystem::exists(temporary), message);
  }
};
void tests() {
  Fixture f;
  f.initialize();
  const Context original = f.context;
  f.information.bIsFatal = FALSE; f.rejects("nonfatal exception recorded"); f.information.bIsFatal = TRUE;
  f.information.dwSize = 0; f.rejects("truncated event recorded"); f.information.dwSize = sizeof(f.information);
  f.context.armed = 0; f.rejects("disarmed context recorded"); f.context = original;
  f.context.sequence = 3; f.rejects("inconsistent context recorded"); f.context = original;
  f.context.sequence = 0; f.rejects("unprepared context recorded"); f.context = original;
  f.context.magic = 0; f.rejects("unknown context recorded"); f.context = original;
  f.context.size = 0; f.rejects("wrong context layout recorded"); f.context = original;
  f.information.exceptionRecord.ExceptionAddress = reinterpret_cast<void *>(0x11000);
  f.rejects("module upper bound recorded");
  f.information.exceptionRecord.ExceptionAddress = reinterpret_cast<void *>(0x10123);
  std::strcpy(f.context.modules[0].name, "host-application.exe");
  f.rejects("external module recorded"); f.context = original;
  std::memset(f.context.modules[0].name, 'x', sizeof(f.context.modules[0].name));
  f.rejects("unterminated module recorded"); f.context = original;
  std::memset(f.context.header, 'x', sizeof(f.context.header));
  f.rejects("unterminated header recorded"); f.context = original;
  std::strcat(f.context.header, "injected\n"); f.rejects("multiline header recorded"); f.context = original;
  f.context.path[1] = L'\\'; f.rejects("relative path recorded"); f.context = original;
  f.context.path[3] = L':'; f.rejects("alternate stream recorded"); f.context = original;
  const auto redirected = f.directory / L"redirected";
  const auto link = f.directory / L"link";
  std::filesystem::create_directory(redirected);
  // Win7 不支持免特权标志；两次都因权限被拒时明确报告这项未执行。
  if (CreateSymbolicLinkW(link.c_str(), redirected.c_str(), SYMBOLIC_LINK_FLAG_DIRECTORY | 2) ||
      CreateSymbolicLinkW(link.c_str(), redirected.c_str(), SYMBOLIC_LINK_FLAG_DIRECTORY)) {
    const auto linked = link / f.path.filename();
    require(linked.wstring().size() < msime::tsf::wer::path_capacity, "synthetic linked path too long");
    std::memset(f.context.path, 0, sizeof(f.context.path));
    std::memcpy(f.context.path, linked.c_str(), linked.wstring().size() * sizeof(wchar_t));
    f.rejects("reparse directory accepted");
    require(std::filesystem::is_empty(redirected), "helper followed reparse parent");
    std::filesystem::remove(link);
  } else {
    require(GetLastError() == ERROR_PRIVILEGE_NOT_HELD || GetLastError() == ERROR_ACCESS_DENIED,
            "synthetic symlink creation failed unexpectedly");
    std::puts("SKIP reparse-parent fixture requires symlink privilege");
  }
  f.context = original;
  // 同进程装了进程内记录器时普通访问冲突交给它记，绕过过滤器的快速失败仍由 helper 记。
  f.context.in_process_recorder = 1;
  f.rejects("in-process recorder fault recorded twice");
  f.information.exceptionRecord.ExceptionCode = 0xC0000409;
  f.invoke();
  require(bytes(f.path) == std::string(original.header) + "EXCEPTION (0xc0000409)\nMetasequoiaImeTsf.dll+0x123\n",
          "fail-fast beside in-process recorder not captured");
  std::filesystem::remove(f.path);
  f.information.exceptionRecord.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
  f.context = original;
  f.invoke(reinterpret_cast<void *>(1));
  require(!std::filesystem::exists(f.path), "unreadable remote context recorded");
  // 目标句柄没有读取内存权限，不能退回把远端地址当成本进程指针。
  const HANDLE target = f.information.hProcess;
  f.information.hProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, f.child.dwProcessId);
  require(f.information.hProcess != nullptr, "restricted target handle unavailable");
  f.rejects("target without memory rights recorded");
  CloseHandle(f.information.hProcess);
  f.information.hProcess = target;

  require(f.event(nullptr, nullptr, nullptr, nullptr, nullptr, nullptr) == E_POINTER,
          "null ownership output accepted");
  BOOL ownership = TRUE;
  require(f.event(nullptr, nullptr, &ownership, nullptr, nullptr, nullptr) == S_OK && !ownership,
          "missing event changed host behavior");
  const auto signature = load<msime::tsf::wer::ExceptionSignature>(f.module,
      "OutOfProcessExceptionEventSignatureCallback");
  const auto debugger = load<msime::tsf::wer::DebuggerLaunch>(f.module,
      "OutOfProcessExceptionEventDebuggerLaunchCallback");
  wchar_t name[] = L"keep";
  DWORD size = 17;
  BOOL custom = TRUE, automatic = TRUE;
  require(signature(nullptr, nullptr, 0, name, &size, name, &size) == E_NOTIMPL &&
          std::wcscmp(name, L"keep") == 0 && size == 17, "signature output modified");
  require(debugger(nullptr, nullptr, &custom, name, &size, &automatic) == E_NOTIMPL &&
          custom && automatic && std::wcscmp(name, L"keep") == 0 && size == 17, "debugger output modified");

  // 已存在临时文件也不能被改写或删除；只清理由本次 CREATE_NEW 建出的文件。
  { std::ofstream stream(f.temporary); stream << "keep-first-partial"; }
  f.invoke();
  require(!std::filesystem::exists(f.path) && bytes(f.temporary) == "keep-first-partial",
          "existing partial changed");
  std::filesystem::remove(f.temporary);
  f.invoke();
  const auto expected = std::string(original.header) +
      "EXCEPTION_ACCESS_VIOLATION (0xc0000005)\nMetasequoiaImeTsf.dll+0x123\n";
  require(bytes(f.path) == expected, "own fatal event not captured with original header");
  f.information.exceptionRecord.ExceptionCode = EXCEPTION_STACK_OVERFLOW;
  f.invoke();
  require(bytes(f.path) == expected && !std::filesystem::exists(f.temporary), "first complete record overwritten");
  std::filesystem::remove(f.path);
  f.information.exceptionRecord.ExceptionAddress = reinterpret_cast<void *>(0x20007);
  f.invoke();
  require(bytes(f.path) == std::string(original.header) +
      "EXCEPTION_STACK_OVERFLOW (0xc00000fd)\nmsime_host_api_wubi.dll+0x7\n", "edition host module not captured");
#if defined(_M_IX86) || defined(__i386__)
  std::filesystem::remove(f.path);
  f.information.exceptionRecord.ExceptionAddress = reinterpret_cast<void *>(0x10123);
  f.information.exceptionRecord.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
  f.publish_frames();
  f.invoke();
  require(bytes(f.path) == std::string(original.header) +
      "EXCEPTION_ACCESS_VIOLATION (0xc0000005)\nMetasequoiaImeTsf.dll+0x123\n"
      "MetasequoiaImeTsf.dll+0x150\nmsime_host_api_wubi.dll+0x10\n",
      "x86 EBP chain was not captured with external frames filtered");
  std::filesystem::remove(f.path);
  f.publish_frames();
  struct Link { DWORD previous; DWORD address; } loop{};
  loop.previous = static_cast<DWORD>(reinterpret_cast<std::uintptr_t>(f.remote_frames));
  loop.address = 0x10150;
  SIZE_T written = 0;
  require(WriteProcessMemory(f.child.hProcess, f.remote_frames, &loop, sizeof(loop), &written) &&
          written == sizeof(loop), "synthetic EBP loop write failed");
  f.invoke();
  require(bytes(f.path) == std::string(original.header) +
      "EXCEPTION_ACCESS_VIOLATION (0xc0000005)\nMetasequoiaImeTsf.dll+0x123\n"
      "MetasequoiaImeTsf.dll+0x150\n", "x86 EBP loop was not bounded");
#endif
}
}
int main(int argc, char **) {
  if (argc > 1) { Sleep(60000); return 0; }
  try { tests(); std::puts("PASS Windows WER remote-context callback boundaries"); return 0; }
  catch (const std::exception &error) { std::fprintf(stderr, "FAIL %s (%lu)\n", error.what(), GetLastError()); return 1; }
}
