// 键盘用于使用情况报告的致命信号处理函数。它只做异步信号安全的事：打开 client-services 的 `msime_client_telemetry_begin` 返回的崩溃记录路径（`O_EXCL`，所以未捕获异常处理函数已经写下的记录会保留），写入一行摘要和各帧，再把信号交还给之前接管它的处理函数。`backtrace_symbols_fd` 写出 "index binary address symbol + offset" 时不分配内存，二进制只以文件名标出。
#include <execinfo.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <string.h>
#include <unistd.h>

static char msime_crash_paths[2][PATH_MAX];
static _Atomic(const char *) msime_crash_path = NULL;
static char msime_record_header[1024];
static size_t msime_record_header_size;
static atomic_flag msime_configuration = ATOMIC_FLAG_INIT;
static const int msime_crash_signals[] = {SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT, SIGTRAP};
static struct sigaction msime_previous_actions[NSIG];
static char msime_alternate_stack[64 * 1024];

static const char *msime_signal_name(int signal) {
  switch (signal) {
  case SIGSEGV: return "SIGSEGV";
  case SIGBUS: return "SIGBUS";
  case SIGILL: return "SIGILL";
  case SIGFPE: return "SIGFPE";
  case SIGABRT: return "SIGABRT";
  case SIGTRAP: return "SIGTRAP";
  default: return "signal";
  }
}

static void msime_crash_signal(int signal, siginfo_t *info, void *context) {
  (void)info; (void)context;
  const bool owns = !atomic_flag_test_and_set(&msime_configuration);
  const char *path = owns ? atomic_load(&msime_crash_path) : NULL;
  if (path != NULL) {
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0600);
    if (fd >= 0) {
      (void)!write(fd, msime_record_header, msime_record_header_size);
      const char *prefix = "Fatal signal ";
      const char *name = msime_signal_name(signal);
      (void)!write(fd, prefix, strlen(prefix));
      (void)!write(fd, name, strlen(name));
      (void)!write(fd, "\n", 1);
      void *frames[64];
      int count = backtrace(frames, 64);
      backtrace_symbols_fd(frames, count, fd);
      (void)fsync(fd);
      close(fd);
    }
  }
  if (owns) atomic_flag_clear(&msime_configuration);
  sigaction(signal, &msime_previous_actions[signal], NULL);
  raise(signal);
}

// 正常执行路径固定路径与构建头，信号处理器只尝试一次配置锁，不能等待。
void msime_ios_crash_record_set(const char *path, const char *header) {
  atomic_store(&msime_crash_path, NULL);
  if (atomic_flag_test_and_set(&msime_configuration)) return;
  if (header == NULL || strlen(header) >= sizeof(msime_record_header)) {
    atomic_flag_clear(&msime_configuration);
    return;
  }
  strlcpy(msime_record_header, header, sizeof(msime_record_header));
  msime_record_header_size = strlen(header);
  if (path == NULL || strlen(path) >= PATH_MAX) {
    atomic_store(&msime_crash_path, NULL);
    atomic_flag_clear(&msime_configuration);
    return;
  }
  // 配置锁保护两个字段成对发布。
  char *slot = atomic_load(&msime_crash_path) == msime_crash_paths[0] ? msime_crash_paths[1] : msime_crash_paths[0];
  strlcpy(slot, path, PATH_MAX);
  atomic_store(&msime_crash_path, slot);
  atomic_flag_clear(&msime_configuration);
}

// 主线程安装一次，为主线程栈溢出保留备用栈。
void msime_ios_crash_handlers_install(void) {
  static atomic_bool installed = false;
  if (atomic_exchange(&installed, true)) return;
  // 首次展开可能加载运行库，提前在正常路径预热。
  void *warm[1];
  (void)backtrace(warm, 1);
  stack_t alternate = {0};
  alternate.ss_sp = msime_alternate_stack;
  alternate.ss_size = sizeof(msime_alternate_stack);
  sigaltstack(&alternate, NULL);
  struct sigaction action;
  memset(&action, 0, sizeof(action));
  action.sa_sigaction = msime_crash_signal;
  action.sa_flags = SA_SIGINFO | SA_ONSTACK;
  sigemptyset(&action.sa_mask);
  for (size_t index = 0; index < sizeof(msime_crash_signals) / sizeof(msime_crash_signals[0]); index++) {
    sigaction(msime_crash_signals[index], &action, &msime_previous_actions[msime_crash_signals[index]]);
  }
}
