// 合成进程用真实内存故障验证业务首帧，不读取输入或真实应用状态。
#include <assert.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/file.h>
#include <sys/ucontext.h>
#include <sys/wait.h>
#include <unistd.h>

bool msime_ios_crash_record_set(const char *, const char *);
bool msime_ios_crash_modules_refresh(void);
void msime_ios_crash_handlers_install(void);

__attribute__((noinline)) void synthetic_fault(void *page) {
  *(volatile char *)page = 7;
}

static volatile sig_atomic_t handled_count;
static int handled_ready = -1;
static int handled_proceed = -1;
static void *recoverable_page;

static void returning_handler(int signal) {
  (void)signal;
  if (++handled_count == 3) _exit(43);
}

static void one_shot_handler(int signal) { (void)signal; }

static void restoring_default_handler(int signal) {
  struct sigaction action = {0};
  action.sa_handler = SIG_DFL;
  sigemptyset(&action.sa_mask);
  sigaction(signal, &action, NULL);
}

static void recovering_handler(int signal, siginfo_t *info, void *context) {
  (void)signal;
  assert(info != NULL && context != NULL);
  assert(info->si_addr == recoverable_page);
  const ucontext_t *machine = context;
#if defined(__aarch64__)
  const uintptr_t pc = __darwin_arm_thread_state64_get_pc(machine->uc_mcontext->__ss);
#elif defined(__x86_64__)
  const uintptr_t pc = machine->uc_mcontext->__ss.__rip;
#endif
  assert(pc >= (uintptr_t)synthetic_fault && pc < (uintptr_t)synthetic_fault + 128);
  ++handled_count;
  if (mprotect(recoverable_page, 4096, PROT_READ | PROT_WRITE) != 0) _exit(44);
}

static void blocked_handler(int signal) {
  (void)signal;
  if (write(handled_ready, "r", 1) != 1) _exit(45);
  char token;
  if (read(handled_proceed, &token, 1) != 1) _exit(46);
}

static void blocked_default_handler(int signal) {
  blocked_handler(signal);
  restoring_default_handler(signal);
  raise(signal);
}

static void previous_handler(void (*handler)(int)) {
  struct sigaction action = {0};
  action.sa_handler = handler;
  sigemptyset(&action.sa_mask);
  sigaction(SIGSEGV, &action, NULL);
  sigaction(SIGBUS, &action, NULL);
}

static int child_status(pid_t pid) {
  int status = 0;
  assert(waitpid(pid, &status, 0) == pid);
  return status;
}

static void assert_record(const char *path) {
  FILE *file = fopen(path, "r");
  assert(file != NULL);
  char header[100];
  assert(fgets(header, sizeof(header), file) != NULL);
  assert(strcmp(header, "MSIME-CRASH/1 {}\n") == 0);
  fclose(file);
  assert(unlink(path) == 0);
}

static void handled_signals(const char *path) {
  pid_t pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    previous_handler(returning_handler);
    msime_ios_crash_handlers_install();
    msime_ios_crash_record_set(path, "MSIME-CRASH/1 {}\n");
    for (int attempt = 0; attempt < 2; ++attempt) {
      raise(SIGSEGV);
      assert(access(path, F_OK) != 0 && errno == ENOENT);
    }
    raise(SIGSEGV);
    _exit(1);
  }
  int status = child_status(pid);
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 43);
  assert_record(path);

  // 直接委托仍须保留前任的一次性处置，第二次信号应交给系统结束进程。
  pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    struct sigaction action = {0};
    action.sa_handler = one_shot_handler;
    action.sa_flags = SA_RESETHAND;
    sigemptyset(&action.sa_mask);
    sigaction(SIGSEGV, &action, NULL);
    struct sigaction observed = {0};
    assert(sigaction(SIGSEGV, NULL, &observed) == 0);
    if (!(observed.sa_flags & SA_RESETHAND)) _exit(77);
    msime_ios_crash_handlers_install();
    msime_ios_crash_record_set(path, "MSIME-CRASH/1 {}\n");
    raise(SIGSEGV);
    assert(access(path, F_OK) != 0 && errno == ENOENT);
    raise(SIGSEGV);
    _exit(1);
  }
  status = child_status(pid);
  if (WIFEXITED(status) && WEXITSTATUS(status) == 77) {
    fprintf(stderr, "SKIP one-shot: sigaction does not expose SA_RESETHAND\n");
  } else {
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV);
    assert_record(path);
  }

  // 前任恢复系统默认处置后等待指令重试，不能把后续真实崩溃的记录一起删掉。
  for (int synthetic_first = 0; synthetic_first < 2; ++synthetic_first) {
    pid = fork();
    assert(pid >= 0);
    if (pid == 0) {
      previous_handler(restoring_default_handler);
      msime_ios_crash_handlers_install();
      msime_ios_crash_record_set(path, "MSIME-CRASH/1 {}\n");
      if (synthetic_first) {
        raise(SIGSEGV);
        assert(access(path, F_OK) != 0 && errno == ENOENT);
      }
      void *page = mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
      assert(page != MAP_FAILED);
      synthetic_fault(page);
      _exit(1);
    }
    status = child_status(pid);
    assert(WIFSIGNALED(status) && (WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS));
    assert_record(path);
  }

  // 修复真正保护页故障时必须收到原始上下文，两次恢复均不产生崩溃记录。
  pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    recoverable_page = mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(recoverable_page != MAP_FAILED);
    struct sigaction action = {0};
    action.sa_sigaction = recovering_handler;
    action.sa_flags = SA_SIGINFO;
    sigemptyset(&action.sa_mask);
    sigaction(SIGSEGV, &action, NULL);
    sigaction(SIGBUS, &action, NULL);
    msime_ios_crash_handlers_install();
    msime_ios_crash_record_set(path, "MSIME-CRASH/1 {}\n");
    for (int attempt = 0; attempt < 2; ++attempt) {
      synthetic_fault(recoverable_page);
      assert(access(path, F_OK) != 0 && errno == ENOENT);
      assert(mprotect(recoverable_page, 4096, PROT_NONE) == 0);
    }
    assert(handled_count == 2);
    _exit(0);
  }
  status = child_status(pid);
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);

  for (int pending_fatal = 0; pending_fatal < 2; ++pending_fatal) {
    // 收件进程已经打开 inode 时，恢复执行也必须撤销旧描述符中的内容。
    int ready[2], proceed[2];
    assert(pipe(ready) == 0 && pipe(proceed) == 0);
    pid = fork();
    assert(pid >= 0);
    if (pid == 0) {
      close(ready[0]); close(proceed[1]);
      handled_ready = ready[1]; handled_proceed = proceed[0];
      previous_handler(pending_fatal ? blocked_default_handler : blocked_handler);
      msime_ios_crash_handlers_install();
      msime_ios_crash_record_set(path, "MSIME-CRASH/1 {}\n");
      raise(SIGSEGV);
      assert(access(path, F_OK) != 0 && errno == ENOENT);
      _exit(0);
    }
    close(ready[1]); close(proceed[0]);
    char token;
    assert(read(ready[0], &token, 1) == 1);
    const int reader = open(path, O_RDONLY | O_CLOEXEC);
    assert(reader >= 0);
    assert(flock(reader, LOCK_EX | LOCK_NB) == -1 && (errno == EWOULDBLOCK || errno == EAGAIN));
    assert(write(proceed[1], "p", 1) == 1);
    status = child_status(pid);
    assert(pending_fatal ? WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV
                         : WIFEXITED(status) && WEXITSTATUS(status) == 0);
    assert(flock(reader, LOCK_EX | LOCK_NB) == 0);
    assert(read(reader, &token, 1) == (pending_fatal ? 1 : 0));
    if (pending_fatal) assert(token == 'M');
    if (pending_fatal) assert_record(path);
    else assert(access(path, F_OK) != 0 && errno == ENOENT);
    close(reader); close(ready[0]); close(proceed[1]);
  }
}

int main(int argc, char **argv) {
  assert(argc == 2 || argc == 3);
  handled_signals(argv[1]);
  // 记录路径先于动态库发布：库里的帧只能靠之后的映像补登记归到模块，发布时的遍历看不到它。
  assert(msime_ios_crash_record_set(argv[1], "MSIME-CRASH/1 {}\n"));
  void (*fault)(void *) = synthetic_fault;
  if (argc == 3) {
    void *library = dlopen(argv[2], RTLD_NOW | RTLD_LOCAL);
    assert(library != NULL);
    fault = (void (*)(void *))dlsym(library, "synthetic_library_fault");
    assert(fault != NULL);
  }
  assert(msime_ios_crash_modules_refresh());
  void *page = mmap(NULL, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
  assert(page != MAP_FAILED);
  const pid_t pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    msime_ios_crash_handlers_install();
    fault(page);
    _exit(0);
  }
  int status = 0;
  assert(waitpid(pid, &status, 0) == pid);
  assert(WIFSIGNALED(status) && (WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS));
  assert(munmap(page, 4096) == 0);
  FILE *record = fopen(argv[1], "r");
  assert(record != NULL);
  char line[2048];
  assert(fgets(line, sizeof(line), record));
  assert(fgets(line, sizeof(line), record));
  assert(fgets(line, sizeof(line), record));
  fclose(record);
  const char *address = strstr(line, "0x");
  assert(address != NULL);
  const uintptr_t pc = (uintptr_t)strtoull(address, NULL, 16);
  Dl_info image;
  assert(dladdr((const void *)fault, &image) != 0);
  const uintptr_t function = (uintptr_t)fault - (uintptr_t)image.dli_fbase;
  assert(pc >= function && pc < function + 128);
  assert(strstr(line, " [id 0x") != NULL);
  assert(strchr(line, '/') == NULL);
  const char *module = argc == 3 ? argv[2] : argv[0];
  const char *name = strrchr(module, '/');
  name = name == NULL ? module : name + 1;
  assert(strncmp(line, name, strlen(name)) == 0 && line[strlen(name)] == '+');
  return 0;
}
