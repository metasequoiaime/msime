// 线程优先级守卫的真机测试：守卫启用后读回 Above normal，析构恢复进入前的值。宿主
// 策略拒绝修改线程优先级时以 77 跳过，避免把环境限制判成实现错误。
#include "ThreadPriority.h"
#include <windows.h>
#include <cstdio>

namespace {
constexpr int skip_exit_code = 77;

// job 或策略可能收下调用却不真正生效；每一项都写进去再读回来，读不到就跳过。
bool set_and_verify(HANDLE thread, int priority, int original_priority,
                    const char *name) {
  if (!SetThreadPriority(thread, priority)) {
    std::printf("skip: cannot set thread priority to %s (err=%lu)\n", name,
                GetLastError());
    (void)SetThreadPriority(thread, original_priority);
    return false;
  }
  const int actual = GetThreadPriority(thread);
  if (actual != priority) {
    std::printf("skip: thread priority did not become %s (got %d)\n", name,
                actual);
    (void)SetThreadPriority(thread, original_priority);
    return false;
  }
  return true;
}
} // namespace

int main() {
  const HANDLE thread = GetCurrentThread();
  const int original_priority = GetThreadPriority(thread);
  if (original_priority == THREAD_PRIORITY_ERROR_RETURN) {
    std::printf("skip: cannot read current thread priority (err=%lu)\n",
                GetLastError());
    return skip_exit_code;
  }
  // 守卫恢复的是构造时捕获的值，所以从 Normal 起步，恢复结果没有歧义。
  if (!set_and_verify(thread, THREAD_PRIORITY_NORMAL, original_priority,
                      "normal"))
    return skip_exit_code;
  // 先证明这台主机接受提升后的优先级，再让守卫去用它。
  if (!set_and_verify(thread, THREAD_PRIORITY_ABOVE_NORMAL, original_priority,
                      "above normal"))
    return skip_exit_code;
  if (!set_and_verify(thread, THREAD_PRIORITY_NORMAL, original_priority,
                      "normal"))
    return skip_exit_code;

  bool ok = true;
  {
    msime::windows::ScopedThreadPriorityBoost disabled(
        THREAD_PRIORITY_ABOVE_NORMAL, false);
    if (disabled.active() || GetThreadPriority(thread) != THREAD_PRIORITY_NORMAL) {
      std::printf("disabled guard active=%d priority=%d\n",
                  disabled.active() ? 1 : 0, GetThreadPriority(thread));
      ok = false;
    }
  }
  {
    msime::windows::ScopedThreadPriorityBoost boost(THREAD_PRIORITY_ABOVE_NORMAL);
    const int boosted = GetThreadPriority(thread);
    ok = ok && boost.active() && boosted == THREAD_PRIORITY_ABOVE_NORMAL;
    std::printf("boost active=%d priority=%d\n", boost.active() ? 1 : 0, boosted);
  }
  if (GetThreadPriority(thread) != THREAD_PRIORITY_NORMAL) {
    std::printf("expected %d after guard destruction, got %d\n",
                THREAD_PRIORITY_NORMAL, GetThreadPriority(thread));
    ok = false;
  }
  // 硬编码恢复成 Normal 的守卫也能通过上面两项；从 Below normal 进入再提升一次，
  // 证明恢复的是捕获值。
  if (!ok) {
    (void)SetThreadPriority(thread, original_priority);
    return 1;
  }
  if (!set_and_verify(thread, THREAD_PRIORITY_BELOW_NORMAL, original_priority,
                      "below normal"))
    return skip_exit_code;
  {
    msime::windows::ScopedThreadPriorityBoost boost(THREAD_PRIORITY_ABOVE_NORMAL);
    ok = boost.active() &&
         GetThreadPriority(thread) == THREAD_PRIORITY_ABOVE_NORMAL;
  }
  if (GetThreadPriority(thread) != THREAD_PRIORITY_BELOW_NORMAL) {
    std::printf("expected %d after guard destruction, got %d\n",
                THREAD_PRIORITY_BELOW_NORMAL, GetThreadPriority(thread));
    ok = false;
  }

  (void)SetThreadPriority(thread, original_priority);
  return ok ? 0 : 1;
}
