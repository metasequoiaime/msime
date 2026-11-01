#include "../CrashRecorder.h"
#include <cassert>
#include <csignal>
#include <cstdint>
#include <dlfcn.h>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>
#include <vector>

int main(int argc, char **argv) {
  assert(argc == 2);
  const std::filesystem::path root(argv[1]);
  std::vector<void *> handles;
  for (int index = 0; index < 620; ++index) {
    const auto path = root / ("synthetic-" + std::to_string(index) + ".so");
    auto *handle = dlopen(path.c_str(), RTLD_NOW | RTLD_LOCAL);
    assert(handle);
    handles.push_back(handle);
  }
  const auto function = reinterpret_cast<void (*)(void *)>(dlsym(handles.back(), "synthetic_library_fault"));
  assert(function);
  const auto record = root / "many-modules.crash";
  const auto pid = fork();
  assert(pid >= 0);
  if (!pid) {
    msime_native_crash_install();
    msime_native_crash_arm(record.c_str(), "MSIME-CRASH/1 {}\n");
    assert(msime::crash::is_armed());
    void *page = mmap(nullptr, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
    assert(page != MAP_FAILED);
    function(page);
    _exit(1);
  }
  int status = 0;
  assert(waitpid(pid, &status, 0) == pid);
  assert(WIFSIGNALED(status) && (WTERMSIG(status) == SIGSEGV || WTERMSIG(status) == SIGBUS));
  std::ifstream stream(record);
  std::string line;
  assert(static_cast<bool>(std::getline(stream, line)));
  assert(static_cast<bool>(std::getline(stream, line)));
  assert(static_cast<bool>(std::getline(stream, line)));
  assert(line.rfind("synthetic-619.so+0x", 0) == 0);
  assert(line.find(" [id 0x") != std::string::npos);
  Dl_info image{};
  assert(dladdr(reinterpret_cast<void *>(function), &image));
  const auto offset = std::stoull(line.substr(line.find('+') + 1), nullptr, 16);
  const auto start = reinterpret_cast<uintptr_t>(function) - reinterpret_cast<uintptr_t>(image.dli_fbase);
  assert(offset >= start && offset < start + 128);

  // 武装之后才加载的库：快照不含它时只能写 <unknown>，refresh_modules 之后才对得上模块名和 build-id。
  for (const bool refresh : {false, true}) {
    const auto late = root / (refresh ? "late-refreshed.crash" : "late-stale.crash");
    const auto child = fork();
    assert(child >= 0);
    if (!child) {
      msime_native_crash_install();
      msime_native_crash_arm(late.c_str(), "MSIME-CRASH/1 {}\n");
      assert(msime::crash::is_armed());
      auto *loaded = dlopen((root / "late.so").c_str(), RTLD_NOW | RTLD_LOCAL);
      assert(loaded);
      if (refresh) msime_native_crash_refresh_modules();
      assert(msime::crash::is_armed());
      const auto fault = reinterpret_cast<void (*)(void *)>(dlsym(loaded, "synthetic_library_fault"));
      assert(fault);
      void *page = mmap(nullptr, 4096, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
      assert(page != MAP_FAILED);
      fault(page);
      _exit(1);
    }
    int late_status = 0;
    assert(waitpid(child, &late_status, 0) == child);
    assert(WIFSIGNALED(late_status) && (WTERMSIG(late_status) == SIGSEGV || WTERMSIG(late_status) == SIGBUS));
    std::ifstream late_stream(late);
    std::string frame;
    for (int skip = 0; skip < 3; ++skip) assert(static_cast<bool>(std::getline(late_stream, frame)));
    if (refresh) {
      assert(frame.rfind("late.so+0x", 0) == 0);
      assert(frame.find(" [id 0x") != std::string::npos);
    } else {
      assert(frame.rfind("<unknown>+0x", 0) == 0);
    }
  }
  for (auto *handle : handles) assert(dlclose(handle) == 0);
}
