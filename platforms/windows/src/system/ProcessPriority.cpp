#include "ProcessPriority.h"
#include <windows.h>

namespace msime::windows {
namespace {
using SetProcessInformationFunction = BOOL(WINAPI *)(HANDLE,
                                                     PROCESS_INFORMATION_CLASS,
                                                     LPVOID, DWORD);
} // namespace

ProcessPriorityResult restore_interactive_process_priority() {
  ProcessPriorityResult result;

  // SetPriorityClass 只改 CPU 调度类。登录任务的 Priority 7 同时把内存优先级压到
  // Low，而 CreateProcess 只继承调度类，所以内存优先级必须单独恢复。
  if (SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS))
    result.cpu_ok = true;
  else
    result.cpu_error = GetLastError();

  // 静态导入会在 Windows 7 上直接让程序起不来：那套系统没有这个导出。
  static const auto set_information = [] {
    const HMODULE module = GetModuleHandleW(L"kernel32.dll");
    return module ? reinterpret_cast<SetProcessInformationFunction>(
                        reinterpret_cast<void *>(
                            GetProcAddress(module, "SetProcessInformation")))
                  : nullptr;
  }();
  if (!set_information) {
    result.memory_ok = true;
    return result;
  }
  MEMORY_PRIORITY_INFORMATION memory_priority{};
  memory_priority.MemoryPriority = MEMORY_PRIORITY_NORMAL;
  if (set_information(GetCurrentProcess(), ProcessMemoryPriority,
                      &memory_priority, sizeof(memory_priority)))
    result.memory_ok = true;
  else
    result.memory_error = GetLastError();
  return result;
}
} // namespace msime::windows
