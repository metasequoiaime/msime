#include "ClipboardHistory.h"
#include <nlohmann/json.hpp>
#include <algorithm>
#include <array>
#include <fstream>
#include <optional>
#ifdef _WIN32
#include "StateRootLease.h"
#include <windows.h>
#endif

namespace msime::windows {
namespace {
constexpr size_t max_store_bytes = 1024 * 1024;

bool store_parent_is_safe(const std::filesystem::path &store) {
#ifdef _WIN32
  try {
    auto parent = store.parent_path();
    if (parent.empty())
      parent = L".";
    if (!parent.is_absolute())
      parent = std::filesystem::absolute(parent);
    reject_reparse_ancestors(parent);
  } catch (...) {
    return false;
  }
#else
  (void)store;
#endif
  return true;
}

bool store_leaf_is_safe(const std::filesystem::path &store) {
#ifdef _WIN32
  HANDLE handle = CreateFileW(
      store.c_str(), FILE_READ_ATTRIBUTES,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
      OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
      nullptr);
  if (handle == INVALID_HANDLE_VALUE) {
    const auto error = GetLastError();
    return error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND;
  }
  const bool safe = handle_is_trusted_file(handle);
  CloseHandle(handle);
  return safe;
#else
  (void)store;
  return true;
#endif
}

#ifndef _WIN32
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
#else
bool read_store_payload(HANDLE input, std::string &payload) {
  std::array<char, 8192> buffer{};
  for (;;) {
    DWORD count = 0;
    if (!ReadFile(input, buffer.data(), static_cast<DWORD>(buffer.size()),
                  &count, nullptr))
      return false;
    if (count == 0)
      return true;
    const auto bytes = static_cast<size_t>(count);
    if (payload.size() > max_store_bytes - bytes)
      return false;
    payload.append(buffer.data(), bytes);
  }
}
#endif

class StoreLock final {
public:
  explicit StoreLock(const std::filesystem::path &store) {
#ifdef _WIN32
    if (!store_parent_is_safe(store))
      return;
    auto lock_path = store;
    lock_path += ".lock";
    handle_ = CreateFileW(lock_path.c_str(), GENERIC_READ | GENERIC_WRITE,
                          FILE_SHARE_READ | FILE_SHARE_WRITE,
                          nullptr, OPEN_ALWAYS,
                          FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                          nullptr);
    if (handle_ == INVALID_HANDLE_VALUE) {
      handle_ = nullptr;
      return;
    }
    if (!handle_is_trusted_file(handle_)) {
      CloseHandle(handle_);
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
#ifdef _WIN32
  HANDLE input = CreateFileW(
      path.c_str(), GENERIC_READ,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
      OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
      nullptr);
  if (input == INVALID_HANDLE_VALUE)
    return {};
  LARGE_INTEGER size{};
  if (!handle_is_trusted_file(input) ||
      !GetFileSizeEx(input, &size) || size.QuadPart < 0 ||
      static_cast<ULONGLONG>(size.QuadPart) > max_store_bytes) {
    CloseHandle(input);
    return {};
  }
#else
  std::ifstream input(path, std::ios::binary);
  if (!input)
    return {};
#endif
  std::string payload;
#ifdef _WIN32
  const bool read = read_store_payload(input, payload);
  CloseHandle(input);
#else
  const bool read = read_store_payload(input, payload);
#endif
  if (!read) return {};
  try { const auto value = nlohmann::json::parse(payload); if (!value.is_array()) return {}; std::vector<std::string> result; result.reserve(ClipboardHistory::max_items); for (const auto &item : value) { if (!item.is_string()) continue; auto text = normalize_clipboard_text(item.get<std::string>()); if (!text.empty() && result.size() < ClipboardHistory::max_items) result.push_back(std::move(text)); } return result; } catch (...) { return {}; }
}
bool write_store(const std::filesystem::path &path, const std::vector<std::string> &items) {
  if (!store_parent_is_safe(path) || !store_leaf_is_safe(path)) return false;
  std::error_code error;
  std::filesystem::create_directories(path.parent_path(), error);
  if (error) return false;
  const auto payload = nlohmann::json(items).dump();
#ifdef _WIN32
  auto temporary = path;
  wchar_t temporary_name[MAX_PATH] = {};
  if (!GetTempFileNameW(path.parent_path().c_str(), L"msi", 0, temporary_name))
    return false;
  temporary = temporary_name;
  HANDLE handle = CreateFileW(temporary.c_str(), GENERIC_WRITE, 0, nullptr,
                              OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                              nullptr);
  if (handle == INVALID_HANDLE_VALUE || !handle_is_trusted_file(handle)) {
    if (handle != INVALID_HANDLE_VALUE)
      CloseHandle(handle);
    (void)remove_private_file(temporary);
    return false;
  }
  DWORD written = 0;
  const bool complete = payload.size() <= MAXDWORD &&
                        WriteFile(handle, payload.data(),
                                  static_cast<DWORD>(payload.size()), &written,
                                  nullptr) &&
                        written == static_cast<DWORD>(payload.size()) &&
                        FlushFileBuffers(handle);
  CloseHandle(handle);
  if (!complete || !MoveFileExW(temporary.c_str(), path.c_str(),
                                MOVEFILE_REPLACE_EXISTING |
                                    MOVEFILE_WRITE_THROUGH)) {
    (void)remove_private_file(temporary);
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
// Return the length of a well-formed UTF-8 scalar at offset, rejecting
// overlong encodings, surrogate code points and values above U+10FFFF.
std::optional<size_t> valid_sequence_length(const std::string &text,
                                            size_t offset) {
  const auto lead = static_cast<unsigned char>(text[offset]);
  size_t length = 0;
  uint32_t minimum = 0;
  if (lead < 0x80) {
    return 1;
  } else if ((lead & 0xE0) == 0xC0) {
    length = 2;
    minimum = 0x80;
  } else if ((lead & 0xF0) == 0xE0) {
    length = 3;
    minimum = 0x800;
  } else if ((lead & 0xF8) == 0xF0) {
    length = 4;
    minimum = 0x10000;
  } else {
    return std::nullopt;
  }
  if (offset > text.size() || length > text.size() - offset)
    return std::nullopt;
  uint32_t codepoint = lead & ((1u << (8 - length - 1)) - 1u);
  for (size_t index = 1; index < length; ++index) {
    const auto byte = static_cast<unsigned char>(text[offset + index]);
    if ((byte & 0xC0) != 0x80) return std::nullopt;
    codepoint = (codepoint << 6) | (byte & 0x3F);
  }
  if (codepoint < minimum || codepoint > 0x10FFFF ||
      (codepoint >= 0xD800 && codepoint <= 0xDFFF))
    return std::nullopt;
  return length;
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
    const auto length = valid_sequence_length(text, i);
    if (!length) return {};
    const size_t cost = *length == 4 ? 2 : 1;
    // Never take part of a character: stop before one that would not fit.
    if (units + cost > ClipboardHistory::max_chars) break;
    normalized.append(text, i, *length);
    units += cost;
    i += *length;
  }
  return normalized;
}

#ifdef _WIN32
std::wstring bounded_clipboard_text(const wchar_t *text, size_t bytes) {
  if (!text || bytes < sizeof(wchar_t)) return {};
  const size_t units = (std::min)(bytes / sizeof(wchar_t), ClipboardHistory::max_chars + 1);
  std::wstring value(text, units);
  if (const auto terminator = value.find(L'\0'); terminator != std::wstring::npos)
    value.resize(terminator);
  return value;
}
#endif
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
#ifdef _WIN32
  if (remove_private_file(store_)) return true;
  const auto attributes = GetFileAttributesW(store_.c_str());
  if (attributes != INVALID_FILE_ATTRIBUTES) return false;
  const auto error = GetLastError();
  return error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND;
#else
  std::error_code error; return std::filesystem::remove(store_, error) || !std::filesystem::exists(store_);
#endif
}
} // namespace msime::windows
