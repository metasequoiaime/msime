#include "ClipboardHistory.h"
#include <nlohmann/json.hpp>
#include <algorithm>
#include <array>
#include <fstream>
#ifdef _WIN32
#include <windows.h>
#endif

namespace msime::windows {
namespace {
constexpr size_t max_store_bytes = 1024 * 1024;

bool read_store_payload(std::ifstream &input, std::string &payload) {
  std::array<char, 8192> buffer{};
  while (input) {
    input.read(buffer.data(), static_cast<std::streamsize>(buffer.size()));
    const auto count = input.gcount();
    if (count <= 0) continue;
    const auto bytes = static_cast<size_t>(count);
    if (payload.size() > max_store_bytes - bytes) return false;
    payload.append(buffer.data(), bytes);
  }
  return input.eof();
}

class StoreLock final {
public:
  explicit StoreLock(const std::filesystem::path &store) {
#ifdef _WIN32
    auto lock_path = store;
    lock_path += ".lock";
    handle_ = CreateFileW(lock_path.c_str(), GENERIC_READ | GENERIC_WRITE,
                          FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                          nullptr, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, nullptr);
    if (handle_ == INVALID_HANDLE_VALUE) {
      handle_ = nullptr;
      return;
    }
    OVERLAPPED offset{};
    if (!LockFileEx(handle_, LOCKFILE_EXCLUSIVE_LOCK, 0, MAXDWORD, MAXDWORD,
                    &offset)) {
      CloseHandle(handle_);
      handle_ = nullptr;
    }
#else
    (void)store;
#endif
  }
  ~StoreLock() {
#ifdef _WIN32
    if (handle_) {
      OVERLAPPED offset{};
      UnlockFileEx(handle_, 0, MAXDWORD, MAXDWORD, &offset);
      CloseHandle(handle_);
    }
#endif
  }
  explicit operator bool() const {
#ifdef _WIN32
    return handle_ != nullptr;
#else
    return true;
#endif
  }
private:
#ifdef _WIN32
  HANDLE handle_ = nullptr;
#endif
};
std::vector<std::string> read_store(const std::filesystem::path &path) {
  std::ifstream input(path, std::ios::binary); if (!input) return {};
  std::string payload;
  if (!read_store_payload(input, payload)) return {};
  try { const auto value = nlohmann::json::parse(payload); if (!value.is_array()) return {}; std::vector<std::string> result; for (const auto &item : value) { if (!item.is_string()) continue; auto text = normalize_clipboard_text(item.get<std::string>()); if (!text.empty() && result.size() < ClipboardHistory::max_items) result.push_back(std::move(text)); } return result; } catch (...) { return {}; }
}
bool write_store(const std::filesystem::path &path, const std::vector<std::string> &items) {
  std::error_code error;
  std::filesystem::create_directories(path.parent_path(), error);
  if (error) return false;
  const auto payload = nlohmann::json(items).dump();
#ifdef _WIN32
  auto temporary = path;
  temporary += ".tmp";
  temporary += std::to_string(GetCurrentProcessId());
  std::ofstream output(temporary, std::ios::binary | std::ios::trunc);
  if (!output) return false;
  output.write(payload.data(), static_cast<std::streamsize>(payload.size()));
  output.close();
  if (!output) {
    std::filesystem::remove(temporary, error);
    return false;
  }
  if (!MoveFileExW(temporary.c_str(), path.c_str(),
                   MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
    std::filesystem::remove(temporary, error);
    return false;
  }
  return true;
#else
  std::ofstream output(path, std::ios::binary | std::ios::trunc);
  if (!output) return false;
  output.write(payload.data(), static_cast<std::streamsize>(payload.size()));
  return static_cast<bool>(output);
#endif
}
}
// Count UTF-16 units, not UTF-8 bytes.
//
// max_chars is the shipped cap of 4000 wchar_t, and client-core's clipboard.rs
// implements exactly that. Applying it to a std::string cut Chinese - the
// primary case for this IME - at about 1333 characters, and worse, stopping on
// a byte count could end inside a multi-byte sequence and leave invalid UTF-8,
// which nlohmann::json::dump() then throws on when the store is written.
namespace {
// Units a UTF-8 lead byte contributes to UTF-16: astral planes take a pair.
size_t utf16_units(unsigned char lead) { return lead >= 0xF0 ? 2 : 1; }
size_t sequence_length(unsigned char lead) {
  if (lead < 0x80) return 1;
  if ((lead & 0xE0) == 0xC0) return 2;
  if ((lead & 0xF0) == 0xE0) return 3;
  if ((lead & 0xF8) == 0xF0) return 4;
  return 1; // Not a lead byte; copy it and let validation elsewhere object.
}
} // namespace
std::string normalize_clipboard_text(std::string text) {
  // Match the shipped Windows history contract: only terminators introduced
  // by CF_UNICODETEXT are removed.  Newlines (including CRLF) and whitespace
  // are user content and must survive the round trip.
  // CF_UNICODETEXT is NUL-terminated.  The source constructs a wide string
  // from that pointer, so an embedded NUL ends the captured clipboard value.
  if (const auto terminator = text.find('\0'); terminator != std::string::npos)
    text.resize(terminator);
  while (!text.empty() && text.back() == '\r')
    text.pop_back();
  std::string normalized;
  normalized.reserve(text.size());
  size_t units = 0;
  for (size_t i = 0; i < text.size() && units < ClipboardHistory::max_chars;) {
    const auto lead = static_cast<unsigned char>(text[i]);
    const size_t length = (std::min)(sequence_length(lead), text.size() - i);
    const size_t cost = utf16_units(lead);
    // Never take part of a character: stop before one that would not fit.
    if (units + cost > ClipboardHistory::max_chars) break;
    normalized.append(text, i, length);
    units += cost;
    i += length;
  }
  return normalized;
}
ClipboardHistory::ClipboardHistory(std::filesystem::path store) : store_(std::move(store)) {}
std::vector<std::string> ClipboardHistory::load() const {
  if (!enabled_) return {};
  StoreLock lock(store_); return lock ? read_store(store_) : std::vector<std::string>{};
}
bool ClipboardHistory::add(std::string text) {
  if (!enabled_) return false;
  text = normalize_clipboard_text(std::move(text));
  if (text.empty()) return false;
  StoreLock lock(store_); if (!lock) return false;
  auto items = read_store(store_);
  if (!items.empty() && items.front() == text) return false;
  items.erase(std::remove(items.begin(), items.end(), text), items.end());
  items.insert(items.begin(), std::move(text));
  if (items.size() > max_items) items.resize(max_items);
  return write_store(store_, items);
}
bool ClipboardHistory::remove(const std::string &text) {
  const auto normalized = normalize_clipboard_text(text);
  if (normalized.empty()) return false;
  StoreLock lock(store_); if (!lock) return false;
  auto items = read_store(store_); const auto before = items.size();
  items.erase(std::remove(items.begin(), items.end(), normalized), items.end());
  if (items.size() == before) return false;
  return write_store(store_, items);
}
bool ClipboardHistory::clear() {
  StoreLock lock(store_); if (!lock) return false;
  std::error_code error; return std::filesystem::remove(store_, error) || !std::filesystem::exists(store_);
}
} // namespace msime::windows
