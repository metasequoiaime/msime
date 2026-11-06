#pragma once

namespace msime::windows {
// 进程的交互优先级由两项组成：CPU 调度类和内存优先级。任务计划程序的 Priority 7
// 同时压低两者，CreateProcess 只让子进程继承前者，所以恢复时要分开设置。
struct ProcessPriorityResult {
  bool cpu_ok = false;
  unsigned long cpu_error = 0;
  bool memory_ok = false;
  unsigned long memory_error = 0;
};

/// 把当前进程恢复成交互优先级：NORMAL_PRIORITY_CLASS + MEMORY_PRIORITY_NORMAL。
/// 两项各自独立尝试，调用方按结果分别记录，不因此中断启动。
/// 内存优先级从 Windows 8 起才有；更早的系统上没有可恢复的状态，按成功处理。
ProcessPriorityResult restore_interactive_process_priority();
} // namespace msime::windows
