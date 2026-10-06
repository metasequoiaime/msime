// The keyboard's fatal-signal handler for usage reporting. It does only async-signal-safe work: open the crash record path client-core's msime_client_telemetry_begin returned (O_EXCL, so a record the uncaught-exception handler already wrote is kept), write a summary line and the frames, and hand the signal back to whoever had it before. backtrace_symbols_fd writes "index binary address symbol + offset" without allocating and names a binary by its file name only.
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
  const char *path = atomic_load(&msime_crash_path);
  if (path != NULL) {
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0600);
    if (fd >= 0) {
      const char *prefix = "Fatal signal ";
      const char *name = msime_signal_name(signal);
      (void)!write(fd, prefix, strlen(prefix));
      (void)!write(fd, name, strlen(name));
      (void)!write(fd, "\n", 1);
      void *frames[64];
      int count = backtrace(frames, 64);
      backtrace_symbols_fd(frames, count, fd);
      close(fd);
    }
  }
  sigaction(signal, &msime_previous_actions[signal], NULL);
  raise(signal);
}

/// Sets the record the running session's crash is written to; NULL when no session runs. Called from one serial queue only.
void msime_ios_crash_record_path_set(const char *path) {
  if (path == NULL || strlen(path) >= PATH_MAX) {
    atomic_store(&msime_crash_path, NULL);
    return;
  }
  // Write the slot the handler is not reading, then publish it.
  char *slot = atomic_load(&msime_crash_path) == msime_crash_paths[0] ? msime_crash_paths[1] : msime_crash_paths[0];
  strlcpy(slot, path, PATH_MAX);
  atomic_store(&msime_crash_path, slot);
}

/// Installs the handlers once, from the main thread so a stack overflow there still has a stack to report on.
void msime_ios_crash_handlers_install(void) {
  static atomic_bool installed = false;
  if (atomic_exchange(&installed, true)) return;
  // The first backtrace() loads the unwinder, which allocates; do it here rather than inside a handler.
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
