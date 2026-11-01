#include "../CrashRecorder.h"
#include "../PosixCrashFile.h"
#include <cassert>
#include <csignal>
#include <cstdint>
#include <cstdio>
#include <dlfcn.h>
#include <filesystem>
#include <fstream>
#include <iterator>
#if defined(__linux__)
#include <link.h>
#endif
#include <string>
#include <sstream>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

namespace {
const char *header = "MSIME-CRASH/1 {\"component\":\"synthetic-test\"}\n";

std::string read(const std::filesystem::path &path) {
  std::ifstream file(path);
  return {std::istreambuf_iterator<char>(file), {}};
}

template<class Body> int child(Body body) {
  const auto pid = fork();
  assert(pid >= 0);
  if (!pid) { body(); _exit(0); }
  int status = 0;
  assert(waitpid(pid, &status, 0) == pid);
  return status;
}

void earlier_handler(int) { _exit(42); }
void one_shot_handler(int) {}
volatile sig_atomic_t recovered_signals = 0;
void *recoverable_page = nullptr;
int handled_ready = -1;
int handled_proceed = -1;

void returning_handler(int) {
  ++recovered_signals;
  if (recovered_signals == 3) _exit(43);
}

void restoring_default_handler(int number) {
  struct sigaction action{};
  action.sa_handler = SIG_DFL;
  sigemptyset(&action.sa_mask);
  sigaction(number, &action, nullptr);
}

void replacement_handler(int) { _exit(47); }
void replacing_handler(int number) {
  struct sigaction action{};
  action.sa_handler = replacement_handler;
  sigemptyset(&action.sa_mask);
  sigaction(number, &action, nullptr);
}

void recovering_handler(int, siginfo_t *, void *) {
  ++recovered_signals;
  if (mprotect(recoverable_page, 4096, PROT_READ | PROT_WRITE) != 0) _exit(44);
}

void blocked_handler(int) {
  if (write(handled_ready, "r", 1) != 1) _exit(45);
  char token;
  if (::read(handled_proceed, &token, 1) != 1) _exit(46);
}

void blocked_default_handler(int number) {
  blocked_handler(number);
  restoring_default_handler(number);
  raise(number);
}

// 收件进程已打开原 inode：恢复执行时清空，默认致命信号待送达时保留到退出。
void handled_file(const std::filesystem::path &root, bool pending_fatal) {
  const auto path = root / (pending_fatal ? "blocked-pending-fatal.crash" : "blocked-handled.crash");
  int ready[2], proceed[2];
  assert(pipe(ready) == 0 && pipe(proceed) == 0);
  const pid_t writer = fork();
  assert(writer >= 0);
  if (writer == 0) {
    close(ready[0]);
    close(proceed[1]);
    handled_ready = ready[1];
    handled_proceed = proceed[0];
    struct sigaction previous{};
    previous.sa_handler = pending_fatal ? blocked_default_handler : blocked_handler;
    sigemptyset(&previous.sa_mask);
    sigaction(SIGSEGV, &previous, nullptr);
    msime_native_crash_install();
    msime_native_crash_arm(path.c_str(), header);
    raise(SIGSEGV);
    assert(!std::filesystem::exists(path));
    _exit(0);
  }
  close(ready[1]);
  close(proceed[0]);
  char token;
  assert(::read(ready[0], &token, 1) == 1);
  const int reader = open(path.c_str(), O_RDONLY | O_CLOEXEC);
  assert(reader >= 0);
  assert(flock(reader, LOCK_EX | LOCK_NB) == -1 && (errno == EWOULDBLOCK || errno == EAGAIN));
  assert(write(proceed[1], "p", 1) == 1);
  int status;
  assert(waitpid(writer, &status, 0) == writer);
  assert(pending_fatal ? WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV
                       : WIFEXITED(status) && WEXITSTATUS(status) == 0);
  assert(flock(reader, LOCK_EX | LOCK_NB) == 0);
  // 默认致命信号仍待送达时，原 inode 必须保留，不能删掉后靠二次采集掩盖记录丢失。
  assert(::read(reader, &token, 1) == (pending_fatal ? 1 : 0));
  if (pending_fatal) assert(token == 'M');
  assert(std::filesystem::exists(path) == pending_fatal);
  close(reader);
  close(ready[0]);
  close(proceed[1]);
}

// 两个进程在摘要与栈之间交错：发布文件名时已经持锁，退出后才允许收件。
void concurrent_file(const std::filesystem::path &root, bool interrupted) {
  const auto path = root / (interrupted ? "interrupted.crash" : "concurrent.crash");
  const auto temporary = path.string() + ".writing";
  int ready[2], proceed[2];
  assert(pipe(ready) == 0 && pipe(proceed) == 0);
  const pid_t writer = fork();
  assert(writer >= 0);
  if (writer == 0) {
    close(ready[0]);
    close(proceed[1]);
    const int file = msime_crash_file_create(path.c_str(), temporary.c_str());
    assert(file >= 0);
    assert(write(file, "Synthetic fault\n", 16) == 16);
    assert(write(ready[1], "r", 1) == 1);
    char token;
    assert(::read(proceed[0], &token, 1) == 1);
    assert(write(file, "synthetic-module+0x1\n", 21) == 21);
    close(file);
    _exit(0);
  }
  close(ready[1]);
  close(proceed[0]);
  char token;
  assert(::read(ready[0], &token, 1) == 1);
  const int reader = open(path.c_str(), O_RDONLY | O_CLOEXEC);
  assert(reader >= 0);
  assert(flock(reader, LOCK_EX | LOCK_NB) == -1 && (errno == EWOULDBLOCK || errno == EAGAIN));
  assert(!std::filesystem::exists(temporary));
  if (interrupted) assert(kill(writer, SIGKILL) == 0);
  else assert(write(proceed[1], "p", 1) == 1);
  int status;
  assert(waitpid(writer, &status, 0) == writer);
  assert(interrupted ? WIFSIGNALED(status) && WTERMSIG(status) == SIGKILL : WIFEXITED(status) && WEXITSTATUS(status) == 0);
  assert(flock(reader, LOCK_EX | LOCK_NB) == 0);
  assert(read(path) == (interrupted ? "Synthetic fault\n" : "Synthetic fault\nsynthetic-module+0x1\n"));
  close(reader);
  close(ready[0]);
  close(proceed[1]);
  // 原有首份记录不能被后续信号替换，也不能残留本次创建的暂存文件。
  assert(msime_crash_file_create(path.c_str(), temporary.c_str()) == -1 && errno == EEXIST);
  assert(!std::filesystem::exists(temporary));
}

// 真实内存故障的第一帧必须来自故障函数，不能来自信号处理器或展开器。
__attribute__((noinline)) void synthetic_fault(void *page) {
  *static_cast<volatile char *>(page) = 7;
}
}

int main() {
  const auto root = std::filesystem::temp_directory_path() / ("msime-crash-recorder-" + std::to_string(getpid()));
  assert(std::filesystem::create_directory(root));
  concurrent_file(root, false);
  concurrent_file(root, true);
  const auto fatal = root / "fatal.crash";
  int status = child([&] {
    msime_native_crash_install();
    msime_native_crash_arm(fatal.c_str(), header);
    assert(msime::crash::is_armed());
    raise(SIGSEGV);
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV);
  const auto signal = read(fatal);
  assert(signal.rfind(header, 0) == 0);
  assert(signal.find("SIGSEGV: segmentation fault") != std::string::npos);
  assert(signal.size() > std::string(header).size() + 40);
#if defined(__linux__)
  assert(signal.find("+0x") != std::string::npos);
  assert(signal.find(" [id 0x") != std::string::npos);
#endif

  const auto instruction = root / "instruction.crash";
  void *page = mmap(nullptr, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
  assert(page != MAP_FAILED);
  status = child([&] {
    msime_native_crash_install();
    msime_native_crash_arm(instruction.c_str(), header);
    synthetic_fault(page);
  });
  assert(WIFSIGNALED(status) && (WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS));
  assert(munmap(page, 4096) == 0);
  std::istringstream recorded(read(instruction));
  std::string first_frame;
  assert(static_cast<bool>(std::getline(recorded, first_frame)));
  assert(static_cast<bool>(std::getline(recorded, first_frame)));
  assert(static_cast<bool>(std::getline(recorded, first_frame)));
  const auto offset = first_frame.find("+0x");
  assert(offset != std::string::npos);
  const auto pc = std::stoull(first_frame.substr(offset + 1), nullptr, 16);
  Dl_info image{};
  assert(dladdr(reinterpret_cast<void *>(&synthetic_fault), &image));
  auto base = reinterpret_cast<uintptr_t>(image.dli_fbase);
#if defined(__linux__)
  // ELF 帧减去加载偏移；非 PIE 的 ET_EXEC 偏移为零，不能减去 dladdr 返回的映射起点。
  const auto *elf = static_cast<const ElfW(Ehdr) *>(image.dli_fbase);
  assert(elf->e_type == ET_EXEC || elf->e_type == ET_DYN);
  if (elf->e_type == ET_EXEC) base = 0;
#endif
  const auto function = reinterpret_cast<uintptr_t>(&synthetic_fault) - base;
  assert(pc >= function && pc < function + 128);

  const auto first = root / "first.crash";
  status = child([&] {
    msime_native_crash_install();
    msime_native_crash_arm(first.c_str(), header);
    msime_native_crash_record("Synthetic panic");
    raise(SIGABRT);
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
  assert(read(first).find("Synthetic panic\n") != std::string::npos);
  assert(read(first).find("SIGABRT") == std::string::npos);

  const auto disabled = root / "disabled.crash";
  status = child([&] {
    msime_native_crash_install();
    msime_native_crash_arm(disabled.c_str(), header);
    msime_native_crash_disarm();
    raise(SIGSEGV);
  });
  assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV);
  assert(!std::filesystem::exists(disabled));

  const auto chained = root / "chained.crash";
  status = child([&] {
    struct sigaction previous{};
    previous.sa_handler = earlier_handler;
    sigemptyset(&previous.sa_mask);
    sigaction(SIGSEGV, &previous, nullptr);
    msime_native_crash_install();
    msime_native_crash_arm(chained.c_str(), header);
    raise(SIGSEGV);
  });
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 42);
  assert(read(chained).rfind(header, 0) == 0);

  // 既有处理器返回表示信号已被接管，不能留下可补报的崩溃；后续致命信号仍要记录。
  const auto handled = root / "handled.crash";
  status = child([&] {
    struct sigaction previous{};
    previous.sa_handler = returning_handler;
    sigemptyset(&previous.sa_mask);
    sigaction(SIGSEGV, &previous, nullptr);
    msime_native_crash_install();
    msime_native_crash_arm(handled.c_str(), header);
    for (int attempt = 0; attempt < 2; ++attempt) {
      raise(SIGSEGV);
      assert(!std::filesystem::exists(handled));
      assert(msime::crash::is_armed());
    }
    raise(SIGSEGV);
  });
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 43);
  assert(read(handled).rfind(header, 0) == 0);

  // 直接调用前任仍须遵循一次性回调；第二次信号交给默认处置并保留崩溃。
  const auto one_shot = root / "one-shot.crash";
  status = child([&] {
    struct sigaction previous{};
    previous.sa_handler = one_shot_handler;
    previous.sa_flags = SA_RESETHAND;
    sigemptyset(&previous.sa_mask);
    sigaction(SIGSEGV, &previous, nullptr);
    struct sigaction observed{};
    assert(sigaction(SIGSEGV, nullptr, &observed) == 0);
    if (!(observed.sa_flags & SA_RESETHAND)) _exit(77);
    msime_native_crash_install();
    msime_native_crash_arm(one_shot.c_str(), header);
    raise(SIGSEGV);
    assert(!std::filesystem::exists(one_shot));
    raise(SIGSEGV);
  });
  if (WIFEXITED(status) && WEXITSTATUS(status) == 77) {
    std::fprintf(stderr, "SKIP one-shot: sigaction does not expose SA_RESETHAND\n");
  } else {
    assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGSEGV);
    assert(read(one_shot).rfind(header, 0) == 0);
  }

  // 前任显式更换回调后，下一次必须委托给新回调，不能继续调用旧回调或漏掉记录。
  const auto replaced = root / "replaced-handler.crash";
  status = child([&] {
    struct sigaction previous{};
    previous.sa_handler = replacing_handler;
    sigemptyset(&previous.sa_mask);
    sigaction(SIGSEGV, &previous, nullptr);
    msime_native_crash_install();
    msime_native_crash_arm(replaced.c_str(), header);
    raise(SIGSEGV);
    assert(!std::filesystem::exists(replaced));
    raise(SIGSEGV);
  });
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 47);
  assert(read(replaced).rfind(header, 0) == 0);

  // Rust 的前任会改回默认处置并返回；外部信号不算崩溃，之后的真实故障仍必须采集。
  for (const bool synthetic_first : {false, true}) {
    const auto reset = root / (synthetic_first ? "reset-after-synthetic.crash" : "reset-after-fault.crash");
    status = child([&] {
      struct sigaction previous{};
      previous.sa_handler = restoring_default_handler;
      sigemptyset(&previous.sa_mask);
      sigaction(SIGSEGV, &previous, nullptr);
      sigaction(SIGBUS, &previous, nullptr);
      msime_native_crash_install();
      msime_native_crash_arm(reset.c_str(), header);
      if (synthetic_first) {
        raise(SIGSEGV);
        assert(!std::filesystem::exists(reset));
      }
      void *page = mmap(nullptr, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
      assert(page != MAP_FAILED);
      synthetic_fault(page);
    });
    assert(WIFSIGNALED(status) && (WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS));
    assert(read(reset).rfind(header, 0) == 0);
  }

  // 真实保护页故障由原处理器修复后会重试原指令，两次恢复都不应被当作进程崩溃。
  const auto recovered = root / "recovered.crash";
  status = child([&] {
    recoverable_page = mmap(nullptr, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(recoverable_page != MAP_FAILED);
    struct sigaction previous{};
    previous.sa_sigaction = recovering_handler;
    previous.sa_flags = SA_SIGINFO;
    sigemptyset(&previous.sa_mask);
    sigaction(SIGSEGV, &previous, nullptr);
    sigaction(SIGBUS, &previous, nullptr);
    msime_native_crash_install();
    msime_native_crash_arm(recovered.c_str(), header);
    for (int attempt = 0; attempt < 2; ++attempt) {
      synthetic_fault(recoverable_page);
      assert(*static_cast<volatile char *>(recoverable_page) == 7);
      assert(!std::filesystem::exists(recovered));
      assert(mprotect(recoverable_page, 4096, PROT_NONE) == 0);
    }
    assert(recovered_signals == 2);
    assert(munmap(recoverable_page, 4096) == 0);
  });
  if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
    std::fprintf(stderr, "recoverable signal child status: %d\n", status);
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
  handled_file(root, false);
  handled_file(root, true);
  std::filesystem::remove_all(root);
}
