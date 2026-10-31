// App 与键盘的致命信号记录器：先写摘要和系统上下文里的故障指令，再尽力展开，最后交还系统处置。O_EXCL 保留异常回调已写下的首份记录；展开器面对损坏栈仍可能失败，不能把它当作完全异步信号安全的路径。
#include <execinfo.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <sys/ucontext.h>
#include <unistd.h>
#include "../../../common/PosixCrashFile.h"

static char msime_crash_paths[2][PATH_MAX];
static char msime_temporary_path[PATH_MAX];
static _Atomic(const char *) msime_crash_path = NULL;
static char msime_record_header[1024];
static size_t msime_record_header_size;
static atomic_flag msime_configuration = ATOMIC_FLAG_INIT;
static const int msime_crash_signals[] = {SIGSEGV, SIGBUS, SIGILL, SIGFPE, SIGABRT, SIGTRAP};
static struct sigaction msime_previous_actions[NSIG];
static char msime_alternate_stack[64 * 1024];

// 在正常配置路径固定模块身份，信号里不查询动态加载器或输出设备目录。
struct msime_crash_module {
  uintptr_t begin;
  uintptr_t end;
  uintptr_t base;
  char name[128];
  char uuid[33];
};
static struct msime_crash_module msime_modules[512];
static size_t msime_module_count;
// 上次遍历开始时加载器报告的映像数；发布之后才加载的映像靠它发现，不必撤下记录路径重新发布。
static _Atomic uint32_t msime_module_image_count;

static void msime_snapshot_modules(void) {
  const uint32_t images = _dyld_image_count();
  msime_module_count = 0;
  for (uint32_t index = 0; index < _dyld_image_count() &&
       msime_module_count < sizeof(msime_modules) / sizeof(msime_modules[0]); ++index) {
    const struct mach_header *image = _dyld_get_image_header(index);
    if (image == NULL || image->magic != MH_MAGIC_64) continue;
    const struct mach_header_64 *header = (const struct mach_header_64 *)image;
    struct msime_crash_module *module = &msime_modules[msime_module_count];
    memset(module, 0, sizeof(*module));
    module->base = (uintptr_t)image;
    const char *name = _dyld_get_image_name(index);
    if (name == NULL) name = "<executable>";
    for (const char *cursor = name; *cursor; ++cursor) if (*cursor == '/') name = cursor + 1;
    strlcpy(module->name, name, sizeof(module->name));
    const intptr_t slide = _dyld_get_image_vmaddr_slide(index);
    const unsigned char *cursor = (const unsigned char *)(header + 1);
    size_t remaining = header->sizeofcmds;
    for (uint32_t command = 0; command < header->ncmds && remaining >= sizeof(struct load_command); ++command) {
      const struct load_command *load = (const struct load_command *)cursor;
      if (load->cmdsize < sizeof(*load) || load->cmdsize > remaining) break;
      if (load->cmd == LC_SEGMENT_64 && load->cmdsize >= sizeof(struct segment_command_64)) {
        const struct segment_command_64 *segment = (const struct segment_command_64 *)load;
        if (segment->initprot & 4) {
          const uintptr_t begin = (uintptr_t)(segment->vmaddr + slide);
          if (module->begin == 0 || begin < module->begin) module->begin = begin;
          if (begin + segment->vmsize > module->end) module->end = begin + segment->vmsize;
        }
      }
      if (load->cmd == LC_UUID && load->cmdsize >= sizeof(struct uuid_command)) {
        const struct uuid_command *uuid = (const struct uuid_command *)load;
        for (size_t byte = 0; byte < sizeof(uuid->uuid); ++byte) {
          module->uuid[byte * 2] = "0123456789abcdef"[uuid->uuid[byte] >> 4];
          module->uuid[byte * 2 + 1] = "0123456789abcdef"[uuid->uuid[byte] & 15];
        }
      }
      cursor += load->cmdsize;
      remaining -= load->cmdsize;
    }
    if (module->begin < module->end) ++msime_module_count;
  }
  atomic_store(&msime_module_image_count, images);
}

static void msime_write_all(int fd, const char *value, size_t length) {
  while (length != 0) {
    const ssize_t written = write(fd, value, length);
    if (written < 0 && errno == EINTR) continue;
    if (written <= 0) return;
    value += written;
    length -= (size_t)written;
  }
}

static void msime_write_text(int fd, const char *value) {
  msime_write_all(fd, value, strlen(value));
}

static void msime_write_hex(int fd, uintptr_t value) {
  char digits[2 + sizeof(value) * 2];
  size_t begin = sizeof(digits);
  do {
    digits[--begin] = "0123456789abcdef"[value & 15];
    value >>= 4;
  } while (value != 0);
  digits[--begin] = 'x';
  digits[--begin] = '0';
  msime_write_all(fd, digits + begin, sizeof(digits) - begin);
}

static void msime_write_frame(int fd, uintptr_t address) {
  for (size_t index = 0; index < msime_module_count; ++index) {
    const struct msime_crash_module *module = &msime_modules[index];
    if (address < module->begin || address >= module->end) continue;
    msime_write_text(fd, module->name);
    msime_write_text(fd, "+");
    msime_write_hex(fd, address - module->base);
    if (module->uuid[0] != '\0') {
      msime_write_text(fd, " [id 0x");
      msime_write_text(fd, module->uuid);
      msime_write_text(fd, "]");
    }
    msime_write_text(fd, "\n");
    return;
  }
  msime_write_text(fd, "<unknown>+");
  msime_write_hex(fd, address);
  msime_write_text(fd, "\n");
}

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

static bool msime_same_signal_action(const struct sigaction *left, const struct sigaction *right) {
  if (left->sa_flags != right->sa_flags) return false;
  if (left->sa_flags & SA_SIGINFO) {
    if (left->sa_sigaction != right->sa_sigaction) return false;
  } else if (left->sa_handler != right->sa_handler) return false;
  for (int signal = 1; signal < NSIG; ++signal) {
    if (sigismember(&left->sa_mask, signal) != sigismember(&right->sa_mask, signal)) return false;
  }
  return true;
}

static void msime_crash_signal(int signal, siginfo_t *info, void *context) {
  const int saved_errno = errno;
  const bool owns = !atomic_flag_test_and_set(&msime_configuration);
  const char *path = owns ? atomic_load(&msime_crash_path) : NULL;
  int fd = -1;
  if (path != NULL) {
    fd = msime_crash_file_create(path, msime_temporary_path);
    if (fd >= 0) {
      (void)!write(fd, msime_record_header, msime_record_header_size);
      const char *prefix = "Fatal signal ";
      const char *name = msime_signal_name(signal);
      (void)!write(fd, prefix, strlen(prefix));
      (void)!write(fd, name, strlen(name));
      (void)!write(fd, "\n", 1);
      // 先写系统上下文里的故障指令，处理器自身的展开栈不能占据业务首帧。
      const ucontext_t *machine = context;
      uintptr_t pc = 0;
      if (machine != NULL && machine->uc_mcontext != NULL) {
#if defined(__aarch64__)
        pc = __darwin_arm_thread_state64_get_pc(machine->uc_mcontext->__ss);
#elif defined(__x86_64__)
        pc = machine->uc_mcontext->__ss.__rip;
#endif
      }
      if (pc != 0) msime_write_frame(fd, pc);
      void *frames[64];
      int count = backtrace(frames, 64);
      for (int index = 1; index < count; ++index) msime_write_frame(fd, (uintptr_t)frames[index]);
      (void)fsync(fd);
    }
  }
  const struct sigaction previous = msime_previous_actions[signal];
  struct sigaction capture_action = {0};
  const bool has_capture_action = sigaction(signal, NULL, &capture_action) == 0 &&
      (capture_action.sa_flags & SA_SIGINFO) && capture_action.sa_sigaction == msime_crash_signal;
  struct sigaction disposition = previous;
  if (previous.sa_flags & SA_RESETHAND) {
    // 直接委托时内核不会替我们重置一次性回调。
    disposition = (struct sigaction){0};
    disposition.sa_handler = SIG_DFL;
    sigemptyset(&disposition.sa_mask);
  }
  sigaction(signal, &disposition, NULL);
  if (previous.sa_handler == SIG_DFL) {
    // 当前信号被屏蔽，默认处置在回调返回后生效；文件锁交给内核在进程退出时释放。
    raise(signal);
    if (owns) atomic_flag_clear(&msime_configuration);
    errno = saved_errno;
    return;
  }
  if (previous.sa_handler != SIG_IGN && previous.sa_handler != NULL) {
    // 前任直接收到原始故障上下文，才能修复保护页或调整返回指令。
    if (previous.sa_flags & SA_SIGINFO) previous.sa_sigaction(signal, info, context);
    else previous.sa_handler(signal);
  }
  sigset_t pending;
  struct sigaction actual = {0};
  if (sigpending(&pending) == 0 && sigismember(&pending, signal) == 1 &&
      sigaction(signal, NULL, &actual) == 0 && actual.sa_handler == SIG_DFL) {
    // 前任重新排队默认致命信号后返回，原记录与文件锁仍须保留到进程结束。
    if (owns) atomic_flag_clear(&msime_configuration);
    errno = saved_errno;
    return;
  }
  if (fd >= 0) {
    // 已打开旧 inode 的收件进程也必须看不到恢复执行的误报，清空和删名均在持锁时完成。
    (void)!ftruncate(fd, 0);
    (void)unlink(path);
    close(fd);
  }
  struct sigaction after = {0};
  if (has_capture_action && sigaction(signal, NULL, &after) == 0 &&
      !msime_same_signal_action(&capture_action, &after)) {
    // 前任更换回调或恢复默认处置时，下一次委托新处置，保留真正重发故障的采集。
    msime_previous_actions[signal] = after;
    sigaction(signal, &capture_action, NULL);
  }
  if (owns) atomic_flag_clear(&msime_configuration);
  errno = saved_errno;
}

// 正常执行路径固定路径与构建头，信号处理器只尝试一次配置锁，不能等待。返回请求的状态是否已经生效：撤下总能生效，配置锁被占用时没有发布，调用方下次重试。
bool msime_ios_crash_record_set(const char *path, const char *header) {
  atomic_store(&msime_crash_path, NULL);
  if (atomic_flag_test_and_set(&msime_configuration)) return path == NULL;
  if (header == NULL || strlen(header) >= sizeof(msime_record_header)) {
    atomic_flag_clear(&msime_configuration);
    return path == NULL;
  }
  strlcpy(msime_record_header, header, sizeof(msime_record_header));
  msime_record_header_size = strlen(header);
  const char temporary_suffix[] = ".writing";
  if (path == NULL || strlen(path) + sizeof(temporary_suffix) > PATH_MAX) {
    atomic_store(&msime_crash_path, NULL);
    atomic_flag_clear(&msime_configuration);
    return path == NULL;
  }
  // 配置锁保护两个字段成对发布。
  char *slot = atomic_load(&msime_crash_path) == msime_crash_paths[0] ? msime_crash_paths[1] : msime_crash_paths[0];
  strlcpy(slot, path, PATH_MAX);
  strlcpy(msime_temporary_path, path, sizeof(msime_temporary_path));
  strlcat(msime_temporary_path, temporary_suffix, sizeof(msime_temporary_path));
  msime_snapshot_modules();
  atomic_store(&msime_crash_path, slot);
  atomic_flag_clear(&msime_configuration);
  return true;
}

// 生命周期每拍调用：映像数变了才在配置锁里重新遍历，已发布的记录路径保持不动，之后加载的映像里的帧也能归到模块。映像数没变时不取锁；锁被占用时返回 false，下一拍重试。
bool msime_ios_crash_modules_refresh(void) {
  if (_dyld_image_count() == atomic_load(&msime_module_image_count)) return true;
  if (atomic_flag_test_and_set(&msime_configuration)) return false;
  msime_snapshot_modules();
  atomic_flag_clear(&msime_configuration);
  return true;
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
