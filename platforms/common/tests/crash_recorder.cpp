#include "../CrashRecorder.h"
#include "../PosixCrashFile.h"
#include <cassert>
#include <csignal>
#include <cstdint>
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
  std::filesystem::remove_all(root);
}
