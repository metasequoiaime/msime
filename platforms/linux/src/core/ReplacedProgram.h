#pragma once

#include <charconv>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <optional>
#include <string>
#include <string_view>
#include <unistd.h>

namespace msime::linux_host {

// The Windows installer stops the IME processes before it replaces their files and starts the new ones afterwards. dpkg (and `cmake --install`, and a relink in a build tree) instead renames a new file over the old path while the running process keeps executing the old inode, so after an upgrade the hosts would go on running the previous build indefinitely. The kernel marks such a file in /proc: the target of /proc/self/exe and the pathname of a /proc/self/maps entry gain this suffix once the path no longer names the file the process has open.
inline constexpr std::string_view kDeletedSuffix = " (deleted)";

enum class ProgramFileState {
  // The path still names the file the process runs, or the state could not be read.
  Current,
  // A new file sits at the original path: an upgrade or a rebuild. Restarting picks it up.
  Replaced,
  // Nothing (usable) sits at the original path any more: the package was removed. Restarting would fail, so a host must not restart itself into it.
  Removed,
};

// Classifies one /proc path. `access_mode` is what the replacement must allow for the restart to work: X_OK for an executable, F_OK for a mapped library.
inline ProgramFileState classify_proc_path(std::string_view proc_path, int access_mode) {
  if (proc_path.size() <= kDeletedSuffix.size() ||
      proc_path.substr(proc_path.size() - kDeletedSuffix.size()) != kDeletedSuffix)
    return ProgramFileState::Current;
  const std::string original(proc_path.substr(0, proc_path.size() - kDeletedSuffix.size()));
  return access(original.c_str(), access_mode) == 0 ? ProgramFileState::Replaced
                                                     : ProgramFileState::Removed;
}

// The state of the program this process was started from.
inline ProgramFileState running_executable_state() {
  char target[4096 + kDeletedSuffix.size()];
  const auto length = readlink("/proc/self/exe", target, sizeof target);
  if (length <= 0 || static_cast<size_t>(length) == sizeof target)
    return ProgramFileState::Current;
  return classify_proc_path(std::string_view(target, static_cast<size_t>(length)), X_OK);
}

// One /proc/<pid>/maps line is "start-end perms offset dev inode" followed, after padding, by the pathname, which may itself contain spaces. Returns the pathname when the mapping contains `address`; an anonymous mapping yields an empty pathname.
inline std::optional<std::string_view> maps_line_path(std::string_view line, std::uintptr_t address) {
  const auto dash = line.find('-');
  const auto space = line.find(' ');
  if (dash == std::string_view::npos || space == std::string_view::npos || dash > space)
    return std::nullopt;
  std::uintptr_t start = 0, end = 0;
  if (std::from_chars(line.data(), line.data() + dash, start, 16).ptr != line.data() + dash ||
      std::from_chars(line.data() + dash + 1, line.data() + space, end, 16).ptr != line.data() + space)
    return std::nullopt;
  if (address < start || address >= end)
    return std::nullopt;
  // Skip the range and the four fixed fields after it.
  auto position = space;
  for (int field = 0; field < 4; ++field) {
    position = line.find_first_not_of(' ', position);
    if (position == std::string_view::npos)
      return std::nullopt;
    position = line.find(' ', position);
    if (position == std::string_view::npos)
      return std::string_view{};
  }
  position = line.find_first_not_of(' ', position);
  if (position == std::string_view::npos)
    return std::string_view{};
  auto path = line.substr(position);
  if (!path.empty() && path.back() == '\n')
    path.remove_suffix(1);
  return path;
}

// The state of the file mapped at `address` in this process, which for an address inside a shared library is that library. Scans /proc/self/maps rather than asking dladdr for the name, because the kernel reports the replacement against the path it resolved, which need not be the one the loader was given.
inline ProgramFileState mapped_file_state(const void *address) {
  FILE *maps = std::fopen("/proc/self/maps", "re");
  if (!maps)
    return ProgramFileState::Current;
  char *line = nullptr;
  size_t capacity = 0;
  ssize_t length;
  auto state = ProgramFileState::Current;
  const auto target = reinterpret_cast<std::uintptr_t>(address);
  while ((length = getline(&line, &capacity, maps)) > 0) {
    if (const auto path = maps_line_path(std::string_view(line, static_cast<size_t>(length)), target)) {
      if (!path->empty() && path->front() == '/')
        state = classify_proc_path(*path, F_OK);
      break;
    }
  }
  std::free(line);
  std::fclose(maps);
  return state;
}

// The systemd user service this process runs as, read from /proc/self/cgroup, or none. A service manager that owns the process also restarts it: `fcitx5 -r` inside such a service starts a second fcitx5 outside it, which takes the bus name while the unit restarts its own copy and loses (Omarchy's omarchy-fcitx5.service has Restart=always), so the restart hint names the unit instead. The unified hierarchy's line is "0::/user.slice/user-1000.slice/user@1000.service/app.slice/omarchy-fcitx5.service". A program a desktop autostarts sits in an app-*.scope or an app-*@autostart.service that `fcitx5 -r` replaces cleanly, so those do not count.
inline std::optional<std::string> user_service_unit(std::string_view cgroup) {
  constexpr std::string_view unified = "0::";
  constexpr std::string_view service = ".service";
  while (!cgroup.empty()) {
    const auto end = cgroup.find('\n');
    auto line = cgroup.substr(0, end);
    cgroup = end == std::string_view::npos ? std::string_view{} : cgroup.substr(end + 1);
    if (line.substr(0, unified.size()) != unified)
      continue;
    line.remove_prefix(unified.size());
    if (line.find("/user@") == std::string_view::npos)
      return std::nullopt;
    const auto unit = line.substr(line.rfind('/') + 1);
    if (unit.size() <= service.size() || unit.substr(unit.size() - service.size()) != service ||
        unit.substr(0, 4) == "app-" || unit.substr(0, 5) == "user@")
      return std::nullopt;
    return std::string(unit);
  }
  return std::nullopt;
}

// 本进程所在的 systemd 用户服务（读 /proc/self/cgroup），没有时为空。升级通知的「立即重启 Fcitx5」按钮按它决定重启服务还是 fcitx5 -r。
inline std::optional<std::string> current_user_service_unit() {
  std::string cgroup;
  if (FILE *file = std::fopen("/proc/self/cgroup", "re")) {
    char buffer[4096];
    size_t length;
    while ((length = std::fread(buffer, 1, sizeof buffer, file)) > 0)
      cgroup.append(buffer, length);
    std::fclose(file);
  }
  return user_service_unit(cgroup);
}

// KWin 的 Wayland 会话把 Fcitx5 当作虚拟键盘自己启动（父进程是 kwin_wayland），并把输入法协议的连接交给它。从别处起来的 `fcitx5 -r` 拿不到那条连接，Wayland 原生应用随之打不了字；这样的 Fcitx5 还继承了 KWin 的 cgroup，systemd 启动的 Plasma 里 user_service_unit 读出的是 plasma-kwin_wayland.service，重启它等于重启整个桌面。所以 KWin 启动的 Fcitx5 没有可以交给用户的重启命令，只能注销或在系统设置里重新选一次虚拟键盘。`parent_comm` 是 /proc/<父进程>/comm，最长 15 个字节，kwin_wayland_wrapper 会被截成 kwin_wayland_wr，所以按前缀认。
inline bool kwin_launched(std::string_view parent_comm) {
  constexpr std::string_view kwin = "kwin_wayland";
  return parent_comm.substr(0, kwin.size()) == kwin;
}

// 本进程是不是 KWin 启动的（见 kwin_launched）。
inline bool parent_is_kwin() {
  std::string comm;
  const auto path = "/proc/" + std::to_string(getppid()) + "/comm";
  if (FILE *file = std::fopen(path.c_str(), "re")) {
    char buffer[64];
    if (std::fgets(buffer, sizeof buffer, file)) comm = buffer;
    std::fclose(file);
  }
  while (!comm.empty() && comm.back() == '\n') comm.pop_back();
  return kwin_launched(comm);
}

// 重启正在运行的 fcitx5、让它载入换过的插件的命令。KWin 启动的 Fcitx5 没有这样的命令（见 kwin_launched），返回空。
inline std::optional<std::string> fcitx5_restart_command() {
  if (parent_is_kwin()) return std::nullopt;
  if (const auto unit = current_user_service_unit())
    return "systemctl --user restart " + *unit;
  return std::string("fcitx5 -r");
}

}  // namespace msime::linux_host
