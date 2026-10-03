#include "../src/system/DiagnosticLog.h"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <string>
#include <unistd.h>

int main() {
  const auto directory = std::filesystem::temp_directory_path() /
                         ("msime-diagnostic-test-" + std::to_string(getpid()));
  std::filesystem::remove_all(directory);
  std::filesystem::create_directory(directory);

  msime_linux_diagnostic_configure(directory.string(), false);
  msime_linux_diagnostic_write("disabled_event");
  assert(!std::filesystem::exists(directory / "diagnostic.log"));

  msime_linux_diagnostic_configure(directory.string(), true);
  msime_linux_diagnostic_write("focus_in");
  msime_linux_diagnostic_write("operation_failed operation=synthetic\nprivate");
  std::ifstream input(directory / "diagnostic.log");
  const std::string document((std::istreambuf_iterator<char>(input)), {});
  assert(document.find("focus_in") != std::string::npos);
  assert(document.find("operation_failed operation=synthetic?private") !=
         std::string::npos);
  assert(document.find("synthetic\nprivate") == std::string::npos);

  const auto outside = directory.parent_path() /
                       (directory.filename().string() + "-outside");
  std::filesystem::create_directory(outside);
  const auto linked = directory / "linked-parent";
  std::filesystem::create_directory_symlink(outside, linked);
  // 以 root 身份运行时（Linux 容器里就是这样），root 自己不对外开放的目录里的链接会被当成受信任的系统链接（见 `src/core/SafePath.h`）；把目录改成其他人可写，这条链接就成了任何人都可能放进去的链接。
  std::filesystem::permissions(directory, std::filesystem::perms::others_write, std::filesystem::perm_options::add);
  msime_linux_diagnostic_configure(linked.string(), true);
  msime_linux_diagnostic_write("must_not_escape");
  assert(!std::filesystem::exists(outside / "diagnostic.log"));
  std::filesystem::remove(linked);
  std::filesystem::remove_all(outside);

  msime_linux_diagnostic_configure(directory.string(), false);
  std::filesystem::remove_all(directory);
}
