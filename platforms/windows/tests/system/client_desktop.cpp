#include "ClientDesktop.h"
#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <thread>
#include <vector>

using namespace msime::windows;
namespace {
struct Handle {
  HANDLE value = nullptr;
  ~Handle() { if (value) CloseHandle(value); }
};
struct Desktop {
  HDESK value = nullptr;
  ~Desktop() { if (value) assert(CloseDesktop(value)); }
};
PipeTicket ticket(DWORD pid, DWORD tid) {
  return {(uint64_t{pid} << 32) | tid, {1, 1, 1}, 0};
}
// 子进程只创建合成隐藏窗口并等待停止信号，不操作输入桌面或真实输入法。
int child(const wchar_t *name) {
  const HDESK original = GetThreadDesktop(GetCurrentThreadId());
  Desktop desktop{OpenDesktopW(name, 0, FALSE,
      DESKTOP_CREATEWINDOW | DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS | DESKTOP_ENUMERATE)};
  if (!desktop.value || !SetThreadDesktop(desktop.value)) return 2;
  HWND window = CreateWindowExW(0, L"STATIC", L"", WS_POPUP, 0, 0, 1, 1,
                                nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
  if (!window) return 3;
  std::printf("%lu:%lu\n", GetCurrentProcessId(), GetCurrentThreadId());
  std::fflush(stdout);
  char text[32]{};
  const bool stopped = std::fgets(text, sizeof(text), stdin) && std::string(text) == "stop\n";
  DestroyWindow(window);
  if (!SetThreadDesktop(original)) return 4;
  return stopped ? 0 : 5;
}
void cross_process(const std::wstring &token, HDESK desktop) {
  SECURITY_ATTRIBUTES security{sizeof(security), nullptr, TRUE};
  Handle input_read, input_write, output_read, output_write;
  assert(CreatePipe(&input_read.value, &input_write.value, &security, 0));
  assert(CreatePipe(&output_read.value, &output_write.value, &security, 0));
  assert(SetHandleInformation(input_write.value, HANDLE_FLAG_INHERIT, 0));
  assert(SetHandleInformation(output_read.value, HANDLE_FLAG_INHERIT, 0));
  std::vector<wchar_t> executable(32768);
  assert(GetModuleFileNameW(nullptr, executable.data(), static_cast<DWORD>(executable.size())));
  std::wstring command = L"\"" + std::wstring(executable.data()) + L"\" " + token;
  STARTUPINFOW startup{};
  startup.cb = sizeof(startup);
  startup.dwFlags = STARTF_USESTDHANDLES;
  startup.hStdInput = input_read.value;
  startup.hStdOutput = output_write.value;
  startup.hStdError = output_write.value;
  PROCESS_INFORMATION process{};
  assert(CreateProcessW(executable.data(), command.data(), nullptr, nullptr, TRUE, CREATE_NO_WINDOW,
                        nullptr, nullptr, &startup, &process));
  Handle process_handle{process.hProcess};
  Handle process_thread{process.hThread};
  // 读子进程的合成身份有超时，失败时不能永远卡住原生测试。
  DWORD available = 0;
  const auto deadline = GetTickCount64() + 10000;
  while (!available && GetTickCount64() < deadline) {
    assert(PeekNamedPipe(output_read.value, nullptr, 0, nullptr, &available, nullptr));
    if (!available) Sleep(10);
  }
  assert(available);
  char line[128]{};
  DWORD bytes = 0;
  assert(ReadFile(output_read.value, line, sizeof(line) - 1, &bytes, nullptr));
  char *end = nullptr;
  const DWORD pid = std::strtoul(line, &end, 10);
  assert(end && *end == ':' && pid == process.dwProcessId);
  const DWORD tid = std::strtoul(end + 1, &end, 10);
  assert(end && (*end == '\r' || *end == '\n'));
  auto resolved = ClientDesktop::resolve(ticket(pid, tid));
  assert(resolved && resolved->current());
  assert(same_desktop_object(resolved->handle(), desktop));
  assert(!ClientDesktop::resolve(ticket(GetCurrentProcessId(), tid)));
  assert(WriteFile(input_write.value, "stop\n", 5, &bytes, nullptr) && bytes == 5);
  assert(WaitForSingleObject(process_handle.value, 10000) == WAIT_OBJECT_0);
  DWORD code = 1;
  assert(GetExitCodeProcess(process_handle.value, &code) && code == 0);
  // 仍持有旧线程对象时退出的客户端不能继续给候选窗提供归属。
  assert(!resolved->current());
}
void same_process(HDESK first, HDESK second) {
  Handle ready{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
  Handle stop{CreateEventW(nullptr, TRUE, FALSE, nullptr)};
  assert(ready.value && stop.value);
  DWORD tid = 0;
  std::thread owner([&] {
    assert(SetThreadDesktop(first));
    HWND window = CreateWindowExW(0, L"STATIC", L"", WS_POPUP, 0, 0, 1, 1,
                                  nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    assert(window);
    tid = GetCurrentThreadId();
    assert(SetEvent(ready.value));
    assert(WaitForSingleObject(stop.value, 10000) == WAIT_OBJECT_0);
    assert(DestroyWindow(window));
  });
  assert(WaitForSingleObject(ready.value, 10000) == WAIT_OBJECT_0);
  auto resolved = ClientDesktop::resolve(ticket(GetCurrentProcessId(), tid));
  assert(resolved && resolved->current());
  assert(same_desktop_object(resolved->handle(), first));
  assert(!same_desktop_object(resolved->handle(), second));
  // 使用产品解析出的拥有句柄绑定新的 UI 线程，不能借用客户端返回的句柄。
  std::thread ui([&] {
    assert(SetThreadDesktop(resolved->handle()));
    assert(same_desktop_object(GetThreadDesktop(GetCurrentThreadId()), first));
  });
  ui.join();
  assert(SetEvent(stop.value));
  owner.join();
  assert(!resolved->current());
  assert(!ClientDesktop::resolve(ticket(GetCurrentProcessId(), tid)));
}
} // namespace
int wmain(int argc, wchar_t **argv) {
  const bool require_station = argc == 2 && !lstrcmpW(argv[1], L"--require-window-station");
  if (argc == 2 && !require_station) return child(argv[1]);
  const auto token = L"MSIME9439Native" + std::to_wstring(GetCurrentProcessId()) +
                     L"x" + std::to_wstring(GetTickCount64());
  Desktop first{CreateDesktopW(token.c_str(), nullptr, nullptr, 0, 0xC3, nullptr)};
  Desktop second{CreateDesktopW((token + L"Other").c_str(), nullptr, nullptr, 0, 0xC3, nullptr)};
  assert(first.value && second.value);
  // 同名桌面位于另一个窗口站时，Win7 回退也必须拒绝；只在建窗前临时切换进程窗口站。
  const HWINSTA original_station = GetProcessWindowStation();
  const HWINSTA other_station = CreateWindowStationW((token + L"Station").c_str(), 0,
      WINSTA_CREATEDESKTOP | WINSTA_ENUMDESKTOPS | WINSTA_READATTRIBUTES, nullptr);
  if (other_station) {
    assert(original_station && SetProcessWindowStation(other_station));
    Desktop same_name{CreateDesktopW(token.c_str(), nullptr, nullptr, 0, 0xC3, nullptr)};
    assert(same_name.value && SetProcessWindowStation(original_station));
    assert(!desktop_identity::same_by_binding(first.value, same_name.value));
    assert(!same_desktop_object(first.value, same_name.value));
    assert(CloseDesktop(same_name.value));
    same_name.value = nullptr;
    assert(CloseWindowStation(other_station));
    std::puts("cross-window-station desktop identity passed");
  } else {
    // Win7 的普通令牌不能创建窗口站；提权专项用参数强制覆盖，不能伪报此项已通过。
    assert(!require_station && GetLastError() == ERROR_ACCESS_DENIED);
    std::puts("cross-window-station creation denied: run elevated with --require-window-station to verify this boundary");
  }
  assert(desktop_identity::same_by_binding(first.value, first.value));
  assert(!desktop_identity::same_by_binding(first.value, second.value));
  assert(!desktop_identity::same_by_binding(first.value, nullptr));
  assert(!ClientDesktop::resolve({}));
  assert(!ClientDesktop::resolve(ticket(GetCurrentProcessId(), 0)));
  assert(!ClientDesktop::resolve(ticket(0, GetCurrentThreadId())));
  cross_process(token, first.value);
  same_process(first.value, second.value);
  // 多个候选线程共享探针时，每次结果仍绑定本次请求，线程退出也不能等待加载锁。
  std::vector<std::thread> callers;
  for (unsigned index = 0; index < 4; ++index) {
    callers.emplace_back([&] {
      for (unsigned iteration = 0; iteration < 16; ++iteration) {
        assert(desktop_identity::same_by_binding(first.value, first.value));
        assert(!desktop_identity::same_by_binding(first.value, second.value));
      }
    });
  }
  for (auto &caller : callers) caller.join();
  std::puts("client desktop: ownership, isolation, binding, process mismatch and exited thread passed");
  return 0;
}
