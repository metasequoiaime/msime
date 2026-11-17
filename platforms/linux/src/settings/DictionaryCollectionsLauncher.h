#pragma once
#include "LinuxEdition.h"
#include <cerrno>
#include <filesystem>
#include <spawn.h>
#include <string>
#include <sys/wait.h>
#include <thread>
extern "C" char **environ;

namespace msime::linux_host {
inline bool launch_dictionary_collections(std::string directory) {
  if (!std::filesystem::path(directory).is_absolute()) return false;
  char program[] = MSIME_EDITION_PACKAGE "-dictionary-groups";
  char option[] = "--state-root";
  char *arguments[] = {program, option, directory.data(), nullptr};
  pid_t child = 0;
  if (posix_spawnp(&child, program, nullptr, nullptr, arguments, ::environ) != 0) return false;
  // 原生窗口独立运行，不占用输入线程。
  std::thread([child] { while (waitpid(child, nullptr, 0) < 0 && errno == EINTR) {} }).detach();
  return true;
}
}
