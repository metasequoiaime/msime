// 进程优先级恢复的真机测试：先把当前进程降到登录任务默认的降级状态，再调用恢复逻辑
// 读回结果。环境策略（job 对象等）不接受这些设置时以 77 跳过，避免假通过。
#include "ProcessPriority.h"
#include <windows.h>
#include <cstdio>

namespace {
constexpr int skip_exit_code = 77;

using GetProcessInformationFunction = BOOL(WINAPI *)(HANDLE,
                                                     PROCESS_INFORMATION_CLASS,
                                                     LPVOID, DWORD);
using SetProcessInformationFunction = BOOL(WINAPI *)(HANDLE,
                                                     PROCESS_INFORMATION_CLASS,
                                                     LPVOID, DWORD);

GetProcessInformationFunction get_process_information() {
  static const GetProcessInformationFunction function = [] {
    const HMODULE module = GetModuleHandleW(L"kernel32.dll");
    return module ? reinterpret_cast<GetProcessInformationFunction>(
                        reinterpret_cast<void *>(
                            GetProcAddress(module, "GetProcessInformation")))
                  : nullptr;
  }();
  return function;
}

// MEMORY_PRIORITY_LOWEST 是合法的 0，不能用 0 当失败标志。
bool read_memory_priority(DWORD &priority) {
  const auto get_information = get_process_information();
  if (!get_information)
    return false;
  MEMORY_PRIORITY_INFORMATION info{};
  if (!get_information(GetCurrentProcess(), ProcessMemoryPriority, &info,
                       sizeof(info)))
    return false;
  priority = info.MemoryPriority;
  return true;
}

bool write_memory_priority(DWORD priority) {
  const HMODULE module = GetModuleHandleW(L"kernel32.dll");
  if (!module)
    return false;
  const auto set_information = reinterpret_cast<SetProcessInformationFunction>(
      reinterpret_cast<void *>(GetProcAddress(module, "SetProcessInformation")));
  if (!set_information)
    return false;
  MEMORY_PRIORITY_INFORMATION info{};
  info.MemoryPriority = priority;
  return set_information(GetCurrentProcess(), ProcessMemoryPriority, &info,
                         sizeof(info)) != FALSE;
}
} // namespace

int main() {
  const DWORD original_class = GetPriorityClass(GetCurrentProcess());
  if (original_class == 0) {
    std::printf("skip: cannot read current process priority (err=%lu)\n",
                GetLastError());
    return skip_exit_code;
  }
  DWORD original_memory = 0;
  const bool memory_supported = read_memory_priority(original_memory);

  if (!SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) ||
      GetPriorityClass(GetCurrentProcess()) != BELOW_NORMAL_PRIORITY_CLASS) {
    std::printf("skip: cannot lower process priority class (err=%lu)\n",
                GetLastError());
    (void)SetPriorityClass(GetCurrentProcess(), original_class);
    return skip_exit_code;
  }
  // 内存优先级只有 Windows 8 起才有；这项 API 不在时只验 CPU 调度类。
  const bool lowered_memory =
      memory_supported &&
      write_memory_priority(MEMORY_PRIORITY_LOW) &&
      [&] {
        DWORD lowered = 0;
        return read_memory_priority(lowered) &&
               lowered == MEMORY_PRIORITY_LOW;
      }();
  if (memory_supported && !lowered_memory) {
    std::printf("skip: cannot lower memory priority (err=%lu)\n",
                GetLastError());
    (void)SetPriorityClass(GetCurrentProcess(), original_class);
    (void)write_memory_priority(original_memory);
    return skip_exit_code;
  }

  const auto result = msime::windows::restore_interactive_process_priority();
  DWORD restored_memory = 0;
  const bool restored_memory_read = read_memory_priority(restored_memory);
  const bool ok = result.cpu_ok && result.memory_ok &&
                  GetPriorityClass(GetCurrentProcess()) == NORMAL_PRIORITY_CLASS &&
                  (!lowered_memory ||
                   (restored_memory_read &&
                    restored_memory == MEMORY_PRIORITY_NORMAL));

  std::printf("restore cpu_ok=%d cpu_error=%lu memory_ok=%d memory_error=%lu "
              "class=0x%lx memory=%lu\n",
              result.cpu_ok ? 1 : 0, result.cpu_error, result.memory_ok ? 1 : 0,
              result.memory_error, GetPriorityClass(GetCurrentProcess()),
              restored_memory);

  (void)SetPriorityClass(GetCurrentProcess(), original_class);
  (void)write_memory_priority(original_memory);
  return ok ? 0 : 1;
}
