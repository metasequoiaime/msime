#include <nlohmann/json.hpp>
#include "ClipboardText.h"
#include "ClipboardAtomicWrite.h"
#include <algorithm>
#include <filesystem>
#include <fcntl.h>
#include <iostream>
#include <string>
#include <sys/file.h>
#include <unistd.h>
#include <vector>

using Json = nlohmann::json;
namespace {
constexpr size_t kMaxItems = 50;
constexpr size_t kMaxChars = 4000;
constexpr size_t kMaxStoreBytes = 1024 * 1024;

class HistoryLock {
 public:
  explicit HistoryLock(const std::filesystem::path &history) {
    fd_ = msime::linux_host::open_clipboard_lock(history);
    if (fd_ >= 0 && flock(fd_, LOCK_EX) != 0) { close(fd_); fd_ = -1; }
  }
  ~HistoryLock() { if (fd_ >= 0) { flock(fd_, LOCK_UN); close(fd_); } }
  bool acquired() const { return fd_ >= 0; }
  HistoryLock(const HistoryLock &) = delete;
  HistoryLock &operator=(const HistoryLock &) = delete;
 private:
  int fd_ = -1;
};
std::string normalize(std::string text) {
  while (!text.empty() && (text.back() == '\0' || text.back() == '\r')) text.pop_back();
  size_t units = 0, cut = text.size();
  for (size_t i = 0; i < text.size();) {
    const auto first = static_cast<unsigned char>(text[i]);
    size_t width = 0; uint32_t codepoint = 0;
    if (first < 0x80) { width = 1; codepoint = first; }
    else if (first >= 0xc2 && first <= 0xdf) { width = 2; codepoint = first & 0x1f; }
    else if (first >= 0xe0 && first <= 0xef) { width = 3; codepoint = first & 0x0f; }
    else if (first >= 0xf0 && first <= 0xf4) { width = 4; codepoint = first & 7; }
    else return {};
    if (i + width > text.size()) return {};
    for (size_t j = 1; j < width; ++j) {
      const auto byte = static_cast<unsigned char>(text[i + j]);
      if ((byte & 0xc0) != 0x80) return {};
      codepoint = (codepoint << 6) | (byte & 0x3f);
    }
    if ((width == 2 && codepoint < 0x80) || (width == 3 && codepoint < 0x800) ||
        (width == 4 && (codepoint < 0x10000 || codepoint > 0x10ffff)) ||
        (codepoint >= 0xd800 && codepoint <= 0xdfff) || codepoint == 0) return {};
    const size_t next = units + (codepoint > 0xffff ? 2 : 1);
    if (next > kMaxChars) { cut = i; break; }
    units = next; i += width;
  }
  text.resize(cut);
  return text;
}
std::vector<std::string> load(const std::filesystem::path &path) {
  const auto payload = msime::linux_host::read_clipboard_file(path, kMaxStoreBytes);
  if (!payload) return {};
  try { auto value = Json::parse(*payload); if (!value.is_array()) return {};
    std::vector<std::string> items;
    items.reserve(kMaxItems);
    items.reserve(kMaxItems);
    for (const auto &item : value) if (item.is_string() && items.size() < kMaxItems) {
      auto text = normalize(item.get<std::string>()); if (!text.empty()) items.push_back(std::move(text));
    }
    return items;
  } catch (...) { return {}; }
}
bool save(const std::filesystem::path &path, const std::vector<std::string> &items) {
  return msime::linux_host::write_clipboard_file_atomically(path, Json(items).dump());
}
}
int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-clipboard <history.json> <list|get|add|add-stdin|remove|remove-index|clear> [value]\n";
    return 0;
  }
  if (argc < 3) return 2;
  const std::filesystem::path path = argv[1];
  const std::string op = argv[2];
  std::string added_text;
  bool had_input = false;
  if (op == "add-stdin") {
    if (argc != 3) return 2;
    // Read before taking the history lock so a slow pipe cannot block readers.
    std::array<char, 1024 * 1024 + 1> input;
    std::cin.read(input.data(), input.size());
    const auto size = static_cast<size_t>(std::cin.gcount());
    if (std::cin.bad() || size == input.size()) return 2;
    added_text.assign(input.data(), size);
    had_input = size != 0;
  } else if (op == "add" && argc == 4) {
    added_text = argv[3];
  }
  if (op == "add-stdin" || (op == "add" && argc == 4)) {
    const std::string original_text = added_text;
    added_text = normalize(std::move(added_text));
    const bool invalid_input = op == "add-stdin" && had_input && added_text.empty() &&
                               original_text.find_first_not_of(std::string("\0\r", 2)) != std::string::npos;
    // Reject malformed text without allowing JSON serialization to terminate
    // the process or echo clipboard content in an exception diagnostic.
    try { (void)Json(added_text).dump(); } catch (...) { return 2; }
    if (invalid_input) return 2;
    if (added_text.empty()) return 0;
    if (!path.parent_path().empty() &&
        !msime::linux_host::prepare_clipboard_directory(path.parent_path()))
      return 1;
  }
  HistoryLock lock(path);
  if (!lock.acquired()) return 1;
  auto items = load(path);
  if (op == "list") { std::cout << Json(items).dump() << '\n'; return 0; }
  // A compositor or desktop launcher can use this explicit stream operation to
  // paste a selected entry.  It never touches the system clipboard itself.
  if (op == "get" && argc == 4) {
    try {
      const auto index = std::stoul(argv[3]);
      if (index >= items.size()) return 1;
      std::cout << items[index];
      return static_cast<bool>(std::cout) ? 0 : 1;
    } catch (...) { return 2; }
  }
  if ((op == "add" && argc == 4) || op == "add-stdin") {
    auto text = std::move(added_text); if (!items.empty() && items.front() == text) return 0;
    items.erase(std::remove(items.begin(), items.end(), text), items.end()); items.insert(items.begin(), std::move(text));
    if (items.size() > kMaxItems) items.resize(kMaxItems);
    return save(path, items) ? 0 : 1;
  }
  if (op == "remove" && argc == 4) { auto old = items.size(); items.erase(std::remove(items.begin(), items.end(), argv[3]), items.end()); return old == items.size() ? 0 : (save(path, items) ? 0 : 1); }
  if (op == "remove-index" && argc == 4) {
    try {
      const auto index = std::stoul(argv[3]);
      if (index >= items.size()) return 1;
      items.erase(items.begin() + static_cast<std::ptrdiff_t>(index));
      return save(path, items) ? 0 : 1;
    } catch (...) { return 2; }
  }
  if (op == "clear") return msime::linux_host::remove_clipboard_file(path) ? 0 : 1;
  return 2;
}
