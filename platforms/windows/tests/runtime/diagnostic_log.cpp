#include "DiagnosticLog.h"
#include <windows.h>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
void require(bool value, const char *what) {
  if (!value)
    throw std::runtime_error(what);
}
std::string read(const std::filesystem::path &path) {
  std::ifstream input(path, std::ios::binary);
  return {std::istreambuf_iterator<char>(input), std::istreambuf_iterator<char>()};
}
} // namespace

int main() {
  wchar_t temp[MAX_PATH]{};
  require(GetTempPathW(MAX_PATH, temp) != 0, "temp path");
  const auto root = std::filesystem::path(temp) /
                    (L"msime-diagnostic-log-" + std::to_wstring(GetCurrentProcessId()));
  std::filesystem::remove_all(root);
  const auto file = root / L"logs" / L"server.log";
  DiagnosticLog log(file);

  // Off by default: nothing is written and no directory appears.
  log.server("disabled");
  log.tsf("disabled");
  require(!std::filesystem::exists(root), "a disabled log touched the disk");

  // Each switch gates only its own lines; the directory is created on first use.
  log.set_enabled(true, false);
  log.server("server line");
  log.tsf("tsf line");
  auto text = read(file);
  require(text.rfind("\xEF\xBB\xBF", 0) == 0, "new file lacks a byte-order mark");
  require(text.find("] server line\r\n") != std::string::npos, "server line missing");
  require(text.find("tsf line") == std::string::npos, "tsf line written while off");
  require(text.find("\xEF\xBB\xBF", 3) == std::string::npos, "second byte-order mark");

  log.set_enabled(false, true);
  log.server("second server line");
  log.tsf("tsf line");
  text = read(file);
  require(text.find("second server line") == std::string::npos, "server line written while off");
  require(text.find("] tsf line\r\n") != std::string::npos, "tsf line missing");

  // An oversized file moves aside and the next line starts a fresh one.
  {
    std::ofstream grow(file, std::ios::binary | std::ios::app);
    grow << std::string(static_cast<size_t>(DiagnosticLog::maximum_bytes), 'x');
  }
  log.tsf("after rotation");
  auto rotated = file;
  rotated += L".1";
  require(std::filesystem::file_size(rotated) >= DiagnosticLog::maximum_bytes, "not rotated");
  text = read(file);
  require(text.rfind("\xEF\xBB\xBF", 0) == 0 && text.find("after rotation") != std::string::npos &&
              text.size() < 256,
          "fresh file after rotation");

  std::filesystem::remove_all(root);
  const auto linked_root = std::filesystem::path(temp) /
                           (L"msime-diagnostic-log-linked-" +
                            std::to_wstring(GetCurrentProcessId()));
  const auto outside = linked_root / L"outside";
  std::filesystem::remove_all(linked_root);
  require(std::filesystem::create_directories(outside), "outside directory");
  const auto logs = linked_root / L"logs";
  if (CreateSymbolicLinkW(logs.c_str(), outside.c_str(), SYMBOLIC_LINK_FLAG_DIRECTORY)) {
    DiagnosticLog linked_log(logs / L"server.log");
    linked_log.set_enabled(true, false);
    linked_log.server("must not escape");
    require(!std::filesystem::exists(outside / L"server.log"),
            "diagnostic log followed a reparse point");
    std::filesystem::remove(logs);
  }

  const auto outside_file = linked_root / L"outside.log";
  const auto leaf = linked_root / L"leaf.log";
  {
    std::ofstream sentinel(outside_file, std::ios::binary);
    sentinel << "synthetic outside";
  }
  if (CreateSymbolicLinkW(leaf.c_str(), outside_file.c_str(), 0)) {
    DiagnosticLog linked_leaf(leaf);
    linked_leaf.set_enabled(true, false);
    linked_leaf.server("must not follow leaf");
    require(read(outside_file) == "synthetic outside",
            "diagnostic log followed a leaf reparse point");
    std::filesystem::remove(leaf);
  }
  std::filesystem::remove_all(linked_root);
  return 0;
}
