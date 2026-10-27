#include "../CrashRecorder.h"
#include <cassert>
#include <csignal>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
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
