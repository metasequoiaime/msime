#include "../CrashRecorder.h"
#include <cassert>
#include <csignal>
#include <cstdint>
#include <dlfcn.h>
#include <filesystem>
#include <fstream>
#include <iterator>
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

// 真实内存故障的第一帧必须来自故障函数，不能来自信号处理器或展开器。
__attribute__((noinline)) void synthetic_fault(void *page) {
  *static_cast<volatile char *>(page) = 7;
}
}

int main() {
  const auto root = std::filesystem::temp_directory_path() / ("msime-crash-recorder-" + std::to_string(getpid()));
  assert(std::filesystem::create_directory(root));
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
  const auto function = reinterpret_cast<uintptr_t>(&synthetic_fault) - reinterpret_cast<uintptr_t>(image.dli_fbase);
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
