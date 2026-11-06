#include "ThreadPriority.h"
#include <windows.h>

namespace msime::windows {
ScopedThreadPriorityBoost::ScopedThreadPriorityBoost(int priority, bool enabled) {
  if (!enabled)
    return;
  const HANDLE thread = GetCurrentThread();
  const int previous = GetThreadPriority(thread);
  if (previous == THREAD_PRIORITY_ERROR_RETURN)
    return;
  if (SetThreadPriority(thread, priority)) {
    previous_ = previous;
    active_ = true;
  }
}

ScopedThreadPriorityBoost::~ScopedThreadPriorityBoost() {
  // 恢复尽力而为：任务已经按提升后的优先级跑完，这一步失败不该掩盖那件事。
  if (active_)
    (void)SetThreadPriority(GetCurrentThread(), previous_);
}
} // namespace msime::windows
