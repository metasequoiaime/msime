#pragma once

#include "../../../common/ReadAloudText.h"

#include <cerrno>
#include <cstdlib>
#include <spawn.h>
#include <string>
#include <sys/stat.h>
#include <sys/wait.h>
#include <thread>
#include <unistd.h>

extern char **environ;

namespace msime::linux_host {

// Linux 的「朗读」不链接任何语音库：系统装了 speech-dispatcher 的命令行 `spd-say` 时起它，由用户配置的输出模块合成；没装时 IBus 和 Fcitx5 的候选菜单都不出这一项。见 .agents/notes/implemented/feature/2026-11-01-system-tts-read-aloud.md。
inline constexpr const char *read_aloud_program = "spd-say";
// 起不来 `spd-say` 时显示的提示。
inline constexpr const char *read_aloud_failure_notice = "无法朗读";

// `PATH` 上有没有可执行的 `spd-say`。只查几个目录里的一个文件名，打开菜单时调用，不缓存：用户装上 speech-dispatcher 以后下一次打开菜单就有这一项。
inline bool read_aloud_available() {
  const char *path = std::getenv("PATH");
  if (!path || !*path)
    return false;
  const std::string directories(path);
  size_t start = 0;
  while (start <= directories.size()) {
    const auto end = directories.find(':', start);
    const auto directory = directories.substr(start, end == std::string::npos ? std::string::npos : end - start);
    // 空的一段按惯例是当前目录；输入法进程的当前目录不可信，跳过。
    if (!directory.empty() && directory.front() == '/') {
      const auto candidate = directory + "/" + read_aloud_program;
      struct stat info {};
      if (stat(candidate.c_str(), &info) == 0 && S_ISREG(info.st_mode) && access(candidate.c_str(), X_OK) == 0)
        return true;
    }
    if (end == std::string::npos)
      break;
    start = end + 1;
  }
  return false;
}

// 起一次 `spd-say -l en <text>` 读出 `text`（msime_client_read_aloud_text 答的英文，以字母开头，不会被当成选项）。不等它读完：`spd-say` 把文字交给 speech-dispatcher 就退出，另起一个线程收掉它，免得留下僵尸进程。起不来时返回 false。
inline bool read_aloud(const std::string &text) {
  if (text.empty())
    return false;
  std::string program = read_aloud_program, language_flag = "-l", language = "en", spoken = text;
  char *argv[] = {program.data(), language_flag.data(), language.data(), spoken.data(), nullptr};
  pid_t child = 0;
  if (posix_spawnp(&child, read_aloud_program, nullptr, nullptr, argv, environ) != 0)
    return false;
  std::thread([child] {
    int status = 0;
    while (waitpid(child, &status, 0) < 0 && errno == EINTR) {
    }
  }).detach();
  return true;
}

} // namespace msime::linux_host
