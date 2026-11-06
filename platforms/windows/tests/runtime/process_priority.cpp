// 进程优先级恢复的真机测试：先把当前进程降到登录任务默认的降级状态，再调用恢复逻辑
// 读回结果。环境策略（job 对象等）不接受这些设置时以 77 跳过，避免假通过。
#include "ProcessPriority.h"
#include <windows.h>
#include <cstdio>

namespace {
constexpr int skip_exit_code = 77;

using ProcessInformationFunction = BOOL(WINAPI *)(HANDLE,
                                                 PROCESS_INFORMATION_CLASS,
                                                 LPVOID, DWORD);

template <typename Function> Function load_kernel32(const char *name) {
  const HMODULE module = GetModuleHandleW(L"kernel32.dll");
  return module ? reinterpret_cast<Function>(
                      reinterpret_cast<void *>(GetProcAddress(module, name)))
                : nullptr;
}

// MEMORY_PRIORITY_LOWEST 是合法的 0，不能用 0 当失败标志。
bool read_memory_priority(DWORD &priority) {
  static const auto get_information =
      load_kernel32<ProcessInformationFunction>("GetProcessInformation");
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
  const auto set_information =
      load_kernel32<ProcessInformationFunction>("SetProcessInformation");
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
  // 内存优先级只有 Windows 8 起才有。Wine 之类只实现一半 API 的环境（读得到、
  // 写 LOW 成功、写 NORMAL 失败）按不支持处理：先证明两种值都写得进去，否则不
  // 做这一半的断言，免得把宿主缺失判成恢复逻辑的错。
  bool memory_supported = read_memory_priority(original_memory);
  DWORD probe = 0;
  if (memory_supported) {
    memory_supported =
        write_memory_priority(MEMORY_PRIORITY_LOW) &&
        read_memory_priority(probe) && probe == MEMORY_PRIORITY_LOW &&
        write_memory_priority(MEMORY_PRIORITY_NORMAL) &&
        read_memory_priority(probe) && probe == MEMORY_PRIORITY_NORMAL;
    if (!memory_supported)
      (void)write_memory_priority(original_memory);
  }

  if (!SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) ||
      GetPriorityClass(GetCurrentProcess()) != BELOW_NORMAL_PRIORITY_CLASS) {
    std::printf("skip: cannot lower process priority class (err=%lu)\n",
                GetLastError());
    (void)SetPriorityClass(GetCurrentProcess(), original_class);
    return skip_exit_code;
  }
  const bool lowered_memory =
      memory_supported && write_memory_priority(MEMORY_PRIORITY_LOW) &&
      read_memory_priority(probe) && probe == MEMORY_PRIORITY_LOW;
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
  const bool ok = result.cpu_ok &&
                  GetPriorityClass(GetCurrentProcess()) == NORMAL_PRIORITY_CLASS &&
                  (!lowered_memory ||
                   (result.memory_ok && restored_memory_read &&
                    restored_memory == MEMORY_PRIORITY_NORMAL));

  std::printf("restore cpu_ok=%d cpu_error=%lu memory_ok=%d memory_error=%lu "
              "class=0x%lx memory=%lu lowered_memory=%d\n",
              result.cpu_ok ? 1 : 0, result.cpu_error, result.memory_ok ? 1 : 0,
              result.memory_error, GetPriorityClass(GetCurrentProcess()),
              restored_memory, lowered_memory ? 1 : 0);

  (void)SetPriorityClass(GetCurrentProcess(), original_class);
  (void)write_memory_priority(original_memory);
  return ok ? 0 : 1;
}
