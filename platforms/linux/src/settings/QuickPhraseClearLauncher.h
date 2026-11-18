#pragma once

#include "LinuxEdition.h"

#include <cerrno>
#include <spawn.h>
#include <string>
#include <sys/wait.h>
#include <thread>

extern "C" char **environ;

namespace msime::linux_host {

inline bool launch_quick_phrase_clear() {
  char program[] = MSIME_EDITION_PACKAGE "-quick-phrase-clear";
  char *arguments[] = {program, nullptr};
  pid_t child = 0;
  if (posix_spawnp(&child, program, nullptr, nullptr, arguments, ::environ) != 0) return false;
  // 原生窗口独立运行；回收子进程避免菜单宿主留下僵尸进程。
  std::thread([child] {
    while (waitpid(child, nullptr, 0) < 0 && errno == EINTR) {}
  }).detach();
  return true;
}

}  // namespace msime::linux_host
