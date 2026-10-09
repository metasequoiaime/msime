#include "../../src/core/DiagnosticLog.h"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <regex>
#include <string>
#include <unistd.h>

int main() {
  const auto directory = std::filesystem::temp_directory_path() /
                         ("msime-macos-diagnostic-" + std::to_string(getpid()));
  std::filesystem::remove_all(directory);
  std::filesystem::create_directories(directory);
  const auto log = directory / "diagnostic.log";

  msime_macos_diagnostic_configure(directory.string(), false);
  assert(!msime_macos_diagnostic_enabled());
  msime_macos_diagnostic_write("disabled_event");
  msime_macos_diagnostic_writef("disabled_formatted elapsed_ms=%.1f", 1.5);
  assert(!std::filesystem::exists(log));

  // A relative directory is refused, so the gate stays closed and formatting is skipped.
  msime_macos_diagnostic_configure("relative", true);
  assert(!msime_macos_diagnostic_enabled());

  msime_macos_diagnostic_configure(directory.string(), true);
  assert(msime_macos_diagnostic_enabled());
  msime_macos_diagnostic_write("focus_in");
  msime_macos_diagnostic_write("operation_failed operation=synthetic\nprivate");
  msime_macos_diagnostic_writef("[key-latency] stage=handle type=%s handled=%d elapsed_ms=%.1f", "down", 1, 2.5);
  msime_macos_diagnostic_writef("formatted %s", "tab\there");
  msime_macos_diagnostic_writef("long %s", std::string(400, 'x').c_str());
  std::ifstream input(log);
  const std::string contents((std::istreambuf_iterator<char>(input)), {});
  assert(contents.find("focus_in") != std::string::npos);
  assert(contents.find("operation_failed operation=synthetic?private") !=
         std::string::npos);
  assert(contents.find("disabled_event") == std::string::npos);
  assert(contents.find("disabled_formatted") == std::string::npos);
  assert(contents.find("[key-latency] stage=handle type=down handled=1 elapsed_ms=2.5") != std::string::npos);
  assert(contents.find("formatted tab?here") != std::string::npos);
  // The formatted event is cut to the 192-byte cap: "long " plus 187 x.
  const auto longStart = contents.find("long x");
  assert(longStart != std::string::npos);
  const auto longEnd = contents.find('\n', longStart);
  assert(longEnd - longStart == 192);
  // Every record starts with a millisecond timestamp and the process and thread ids, the shape the Windows log uses and the MCP reader parses by its first 19 bytes.
  const std::regex prefix(R"(^\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d{3} \[p\d+:t\d+\] )");
  std::size_t lines = 0;
  for (std::size_t start = 0; start < contents.size();) {
    const auto end = contents.find('\n', start);
    assert(end != std::string::npos);
    assert(std::regex_search(contents.substr(start, end - start), prefix));
    ++lines;
    start = end + 1;
  }
  assert(lines == 5);
  assert(contents.find(" [p" + std::to_string(getpid()) + ":t") != std::string::npos);

  const auto outside = directory.parent_path() /
                       (directory.filename().string() + "-outside");
  std::filesystem::create_directory(outside);
  const auto linked = directory / "linked-parent";
  std::filesystem::create_directory_symlink(outside, linked);
  msime_macos_diagnostic_configure(linked.string(), true);
  assert(!msime_macos_diagnostic_enabled());
  msime_macos_diagnostic_write("must_not_escape");
  assert(!std::filesystem::exists(outside / "diagnostic.log"));
  std::filesystem::remove(linked);
  std::filesystem::remove_all(outside);

  // 同 UID 的硬链接不是私有日志文件；追加会同时改写另一个目录项，所以直接拒绝该 inode。
  const auto hardlinkTarget = directory.parent_path() /
                              (directory.filename().string() + "-hardlink-target");
  std::ofstream(hardlinkTarget) << "outside\n";
  std::filesystem::remove(log);
  std::filesystem::create_hard_link(hardlinkTarget, log);
  msime_macos_diagnostic_configure(directory.string(), true);
  msime_macos_diagnostic_write("must_not_modify_hardlink");
  std::ifstream protectedLog(hardlinkTarget);
  const std::string protectedContents((std::istreambuf_iterator<char>(protectedLog)), {});
  assert(protectedContents == "outside\n");
  std::filesystem::remove(log);
  std::filesystem::remove(hardlinkTarget);
  std::ofstream(log) << contents;

  msime_macos_diagnostic_configure(directory.string(), true);
  assert(msime_macos_diagnostic_enabled());

  msime_macos_diagnostic_configure(directory.string(), false);
  assert(!msime_macos_diagnostic_enabled());
  msime_macos_diagnostic_write("after_disable");
  msime_macos_diagnostic_writef("after_disable_formatted %d", 1);
  std::ifstream after(log);
  const std::string unchanged((std::istreambuf_iterator<char>(after)), {});
  assert(unchanged == contents);
  std::filesystem::remove_all(directory);

  // Host API lines keep only the fixed category before the first colon: the pack id, the path and the error text after it never reach the file, and a line that arrives while the log is off is dropped, not kept for when it is turned on.
  const auto hostDirectory = std::filesystem::temp_directory_path() /
                             ("msime-macos-host-line-" + std::to_string(getpid()));
  std::filesystem::remove_all(hostDirectory);
  std::filesystem::create_directories(hostDirectory);
  const auto hostLog = hostDirectory / "diagnostic.log";
  msime_macos_diagnostic_host_line("helpcode pack unavailable, falling back to the scheme's schema: early-pack -> zrm: missing");
  msime_macos_diagnostic_configure(hostDirectory.string(), false);
  msime_macos_diagnostic_host_line("sound pack not loaded: off-pack: /Users/someone/Library/pack: corrupt");
  assert(!std::filesystem::exists(hostLog));
  msime_macos_diagnostic_configure(hostDirectory.string(), true);
  assert(!std::filesystem::exists(hostLog));
  msime_macos_diagnostic_host_line(nullptr);
  msime_macos_diagnostic_host_line("helpcode pack unavailable, falling back to the scheme's schema: radicals -> zrm: /Users/someone/Library/pack/table.txt: No such file or directory");
  msime_macos_diagnostic_host_line("sentence model ignored: /Users/someone/model.bin: corrupt");
  msime_macos_diagnostic_host_line("no colon /Users/someone/secret");
  msime_macos_diagnostic_host_line(("long " + std::string(400, 'x') + ": detail").c_str());
  std::ifstream hostInput(hostLog);
  const std::string hostContents((std::istreambuf_iterator<char>(hostInput)), {});
  assert(hostContents.find("] host_api: helpcode pack unavailable, falling back to the scheme's schema\n") != std::string::npos);
  assert(hostContents.find("] host_api: sentence model ignored\n") != std::string::npos);
  assert(hostContents.find("] host_api: uncategorized\n") != std::string::npos);
  for (const char *detail : {"radicals", "zrm", "/Users", "someone", "No such file", "corrupt", "early-pack", "off-pack", "detail"})
    assert(hostContents.find(detail) == std::string::npos);
  // Nothing was retained: exactly the four lines written while the log was on, each one record.
  std::size_t hostLines = 0;
  for (std::size_t start = 0; start < hostContents.size();) {
    const auto end = hostContents.find('\n', start);
    assert(end != std::string::npos);
    assert(end - start <= 64 + 192);
    ++hostLines;
    start = end + 1;
  }
  assert(hostLines == 4);
  // Turning the log off and on again writes nothing either.
  msime_macos_diagnostic_configure(hostDirectory.string(), false);
  msime_macos_diagnostic_host_line("music not played: late-pack: missing");
  msime_macos_diagnostic_configure(hostDirectory.string(), true);
  std::ifstream hostAgain(hostLog);
  const std::string hostRepeated((std::istreambuf_iterator<char>(hostAgain)), {});
  assert(hostRepeated == hostContents);
  msime_macos_diagnostic_configure(hostDirectory.string(), false);
  std::filesystem::remove_all(hostDirectory);
}
