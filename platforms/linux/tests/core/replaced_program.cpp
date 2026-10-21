// How the hosts tell that a package upgrade replaced their program under them: the IBus host checks /proc/self/exe, the Fcitx5 addon checks the /proc/self/maps entry holding its own code. The real-kernel cases do what dpkg does, renaming a new file over one that is running or mapped, and then removing it.
#include "../src/core/ReplacedProgram.h"

#include <cassert>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <string>
#include <sys/mman.h>
#include <sys/wait.h>
#include <unistd.h>

using msime::linux_host::classify_proc_path;
using msime::linux_host::mapped_file_state;
using msime::linux_host::maps_line_path;
using msime::linux_host::ProgramFileState;
using msime::linux_host::running_executable_state;
using msime::linux_host::user_service_unit;

namespace {

void write_file(const std::filesystem::path &path, const std::string &contents) {
  std::ofstream output(path, std::ios::binary | std::ios::trunc);
  output << contents;
  assert(output.good());
}

// Child mode: answer each byte on stdin with the state of this process's own executable.
int report_executable_state() {
  char request;
  while (read(STDIN_FILENO, &request, 1) == 1) {
    const char answer = static_cast<char>('0' + static_cast<int>(running_executable_state()));
    if (write(STDOUT_FILENO, &answer, 1) != 1)
      return 1;
  }
  return 0;
}

struct Reporter {
  pid_t pid;
  int to_child;
  int from_child;

  ProgramFileState ask() const {
    const char request = '?';
    assert(write(to_child, &request, 1) == 1);
    char answer = 0;
    assert(read(from_child, &answer, 1) == 1);
    return static_cast<ProgramFileState>(answer - '0');
  }
};

Reporter start_reporter(const std::filesystem::path &program) {
  int requests[2], answers[2];
  assert(pipe(requests) == 0 && pipe(answers) == 0);
  const pid_t pid = fork();
  assert(pid >= 0);
  if (pid == 0) {
    dup2(requests[0], STDIN_FILENO);
    dup2(answers[1], STDOUT_FILENO);
    close(requests[0]); close(requests[1]); close(answers[0]); close(answers[1]);
    const std::string path = program.string();
    execl(path.c_str(), path.c_str(), "--report-executable", static_cast<char *>(nullptr));
    _exit(127);
  }
  close(requests[0]);
  close(answers[1]);
  return {pid, requests[1], answers[0]};
}

void classification() {
  char pattern[] = "/tmp/msime-replaced-program-XXXXXX";
  const char *created = mkdtemp(pattern);
  assert(created != nullptr);
  const std::filesystem::path root(created);
  const auto file = root / "program";
  write_file(file, "new build");
  const auto deleted = file.string() + " (deleted)";

  // A path the kernel reports without the suffix is the file the process runs.
  assert(classify_proc_path(file.string(), F_OK) == ProgramFileState::Current);
  assert(classify_proc_path(" (deleted)", F_OK) == ProgramFileState::Current);
  assert(classify_proc_path("", F_OK) == ProgramFileState::Current);
  // Deleted with a new file at the same path is an upgrade; nothing there is a removal.
  assert(classify_proc_path(deleted, F_OK) == ProgramFileState::Replaced);
  // An executable replaced by something that cannot be executed is no better than a removal: restarting into it would fail and the launcher would back off forever.
  std::filesystem::permissions(file, std::filesystem::perms::owner_read | std::filesystem::perms::owner_write);
  assert(classify_proc_path(deleted, X_OK) == ProgramFileState::Removed);
  std::filesystem::permissions(file, std::filesystem::perms::owner_all);
  assert(classify_proc_path(deleted, X_OK) == ProgramFileState::Replaced);
  std::filesystem::remove(file);
  assert(classify_proc_path(deleted, F_OK) == ProgramFileState::Removed);
  std::filesystem::remove_all(root);
}

void maps_parsing() {
  const std::string_view library =
      "7f1c2a000000-7f1c2a021000 r-xp 00001000 08:01 1234                       /usr/lib/x86_64-linux-gnu/fcitx5/libmsime-fcitx5.so (deleted)\n";
  auto path = maps_line_path(library, 0x7f1c2a000000);
  assert(path && *path == "/usr/lib/x86_64-linux-gnu/fcitx5/libmsime-fcitx5.so (deleted)");
  assert(maps_line_path(library, 0x7f1c2a020fff));
  // The end of the range is exclusive.
  assert(!maps_line_path(library, 0x7f1c2a021000));
  assert(!maps_line_path(library, 0x7f1c29ffffff));
  // Pathnames can contain spaces.
  path = maps_line_path("1000-2000 r--p 00000000 00:2a 77 /home/user/build dir/libx.so\n", 0x1800);
  assert(path && *path == "/home/user/build dir/libx.so");
  // Anonymous mappings have no pathname, with or without trailing padding.
  path = maps_line_path("1000-2000 rw-p 00000000 00:00 0 \n", 0x1000);
  assert(path && path->empty());
  path = maps_line_path("1000-2000 rw-p 00000000 00:00 0", 0x1000);
  assert(path && path->empty());
  path = maps_line_path("1000-2000 rw-p 00000000 00:00 0                          [heap]", 0x1000);
  assert(path && *path == "[heap]");
  assert(!maps_line_path("garbage", 0x1000));
  assert(!maps_line_path("zz-2000 r--p 0 0 0 /x", 0x1000));
}

// Only a systemd user service other than a desktop's autostart wrapper changes how fcitx5 is restarted.
void service_unit_detection() {
  assert(user_service_unit("0::/user.slice/user-1000.slice/user@1000.service/app.slice/omarchy-fcitx5.service\n") ==
         "omarchy-fcitx5.service");
  // A hybrid hierarchy lists the named controllers first.
  assert(user_service_unit("1:name=systemd:/x.service\n0::/user.slice/user-1000.slice/user@1000.service/app.slice/fcitx5.service\n") ==
         "fcitx5.service");
  assert(!user_service_unit("0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-org.fcitx.Fcitx5@autostart.service\n"));
  assert(!user_service_unit("0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-Hyprland-fcitx5-1234.scope\n"));
  assert(!user_service_unit("0::/user.slice/user-1000.slice/session-2.scope\n"));
  // A system service is not the user's to restart.
  assert(!user_service_unit("0::/system.slice/fcitx5.service\n"));
  assert(!user_service_unit("0::/user.slice/user-1000.slice/user@1000.service/init.scope\n"));
  assert(!user_service_unit(""));
}

// KWin 启动的 Fcitx5 没有重启命令：父进程的 comm 是 kwin_wayland，或被截到 15 个字节的 kwin_wayland_wrapper。
void kwin_launch_detection() {
  using msime::linux_host::kwin_launched;
  assert(kwin_launched("kwin_wayland"));
  assert(kwin_launched("kwin_wayland_wr"));
  assert(!kwin_launched("kwin_x11"));
  assert(!kwin_launched("systemd"));
  assert(!kwin_launched("Hyprland"));
  assert(!kwin_launched(""));
}

void mapped_file_replaced_and_removed() {
  char pattern[] = "/tmp/msime-replaced-mapping-XXXXXX";
  const char *created = mkdtemp(pattern);
  assert(created != nullptr);
  const std::filesystem::path root(created);
  const auto library = root / "libmsime-fcitx5.so";
  write_file(library, std::string(8192, 'a'));
  const std::string path = library.string();
  FILE *file = std::fopen(path.c_str(), "r");
  assert(file != nullptr);
  void *mapping = mmap(nullptr, 8192, PROT_READ, MAP_PRIVATE, fileno(file), 0);
  assert(mapping != MAP_FAILED);
  std::fclose(file);
  const void *inside = static_cast<const char *>(mapping) + 4096;

  assert(mapped_file_state(inside) == ProgramFileState::Current);
  // dpkg unpacks next to the file and renames over it.
  write_file(root / "libmsime-fcitx5.so.dpkg-new", std::string(8192, 'b'));
  std::filesystem::rename(root / "libmsime-fcitx5.so.dpkg-new", library);
  assert(mapped_file_state(inside) == ProgramFileState::Replaced);
  std::filesystem::remove(library);
  assert(mapped_file_state(inside) == ProgramFileState::Removed);
  // Memory that is not a file mapping is never reported.
  int local = 0;
  assert(mapped_file_state(&local) == ProgramFileState::Current);
  munmap(mapping, 8192);
  std::filesystem::remove_all(root);
}

void running_executable_replaced_and_removed() {
  // Under the build directory rather than /tmp, which may be mounted noexec.
  std::string pattern = (std::filesystem::current_path() / "msime-replaced-executable-XXXXXX").string();
  const char *created = mkdtemp(pattern.data());
  assert(created != nullptr);
  const std::filesystem::path root(created);
  const auto program = root / "msime-linux-ibus";
  std::filesystem::copy_file("/proc/self/exe", program);
  std::filesystem::permissions(program, std::filesystem::perms::owner_all);
  const auto reporter = start_reporter(program);

  assert(reporter.ask() == ProgramFileState::Current);
  std::filesystem::copy_file("/proc/self/exe", root / "msime-linux-ibus.dpkg-new");
  std::filesystem::permissions(root / "msime-linux-ibus.dpkg-new", std::filesystem::perms::owner_all);
  std::filesystem::rename(root / "msime-linux-ibus.dpkg-new", program);
  assert(reporter.ask() == ProgramFileState::Replaced);
  // Removing the package leaves no program to restart into.
  std::filesystem::remove(program);
  assert(reporter.ask() == ProgramFileState::Removed);

  close(reporter.to_child);
  int status = 0;
  assert(waitpid(reporter.pid, &status, 0) == reporter.pid);
  assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
  close(reporter.from_child);
  std::filesystem::remove_all(root);
  // This process was not touched.
  assert(running_executable_state() == ProgramFileState::Current);
}

}  // namespace

int main(int argc, char **argv) {
  if (argc > 1 && std::strcmp(argv[1], "--report-executable") == 0)
    return report_executable_state();
  classification();
  maps_parsing();
  service_unit_detection();
  kwin_launch_detection();
  mapped_file_replaced_and_removed();
  running_executable_replaced_and_removed();
  return 0;
}
