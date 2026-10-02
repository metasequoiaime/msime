#include "DiagnosticLog.h"
#include "StateRootLease.h"
#include <windows.h>
#include <cstdio>
#include <string>
#include <system_error>

namespace msime::windows {
namespace {
std::string timestamp() {
  SYSTEMTIME now{};
  GetLocalTime(&now);
  char buffer[32]{};
  std::snprintf(buffer, sizeof(buffer), "%04u-%02u-%02u %02u:%02u:%02u.%03u",
                static_cast<unsigned>(now.wYear), static_cast<unsigned>(now.wMonth),
                static_cast<unsigned>(now.wDay), static_cast<unsigned>(now.wHour),
                static_cast<unsigned>(now.wMinute), static_cast<unsigned>(now.wSecond),
                static_cast<unsigned>(now.wMilliseconds));
  return buffer;
}
} // namespace

DiagnosticLog::DiagnosticLog(std::filesystem::path file) : file_(std::move(file)) {}

void DiagnosticLog::set_enabled(bool server, bool tsf) {
  server_.store(server, std::memory_order_release);
  tsf_.store(tsf, std::memory_order_release);
}

void DiagnosticLog::server(std::string_view line) {
  if (server_enabled())
    append(line);
}

void DiagnosticLog::tsf(std::string_view line) {
  if (tsf_enabled())
    append(line);
}

void DiagnosticLog::append(std::string_view line) {
  // Diagnostics must never affect the input path, so every failure here - a missing directory, a full disk, a string that cannot be built - drops the line.
  try {
    std::lock_guard<std::mutex> lock(mutex_);
    reject_reparse_ancestors(file_.parent_path());
    std::error_code error;
    std::filesystem::create_directories(file_.parent_path(), error);
    const auto size = std::filesystem::file_size(file_, error);
    if (!error && size >= maximum_bytes) {
      auto previous = file_;
      previous += L".1";
      MoveFileExW(file_.c_str(), previous.c_str(), MOVEFILE_REPLACE_EXISTING);
    }
    std::string record = timestamp() + " [p" + std::to_string(GetCurrentProcessId()) +
                         ":t" + std::to_string(GetCurrentThreadId()) + "] ";
    record.append(line);
    record.append("\r\n");
    HANDLE file = CreateFileW(file_.c_str(), FILE_APPEND_DATA,
                              FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                              nullptr, OPEN_ALWAYS,
                              FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT, nullptr);
    if (file == INVALID_HANDLE_VALUE)
      return;
    const bool existed = GetLastError() == ERROR_ALREADY_EXISTS;
    BY_HANDLE_FILE_INFORMATION info{};
    if (!GetFileInformationByHandle(file, &info) ||
        (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)) {
      CloseHandle(file);
      return;
    }
    DWORD written = 0;
    // A byte-order mark on a new file, as the reference writes, so Notepad on older Windows reads it as UTF-8.
    if (!existed)
      WriteFile(file, "\xEF\xBB\xBF", 3, &written, nullptr);
    WriteFile(file, record.data(), static_cast<DWORD>(record.size()), &written, nullptr);
    CloseHandle(file);
  } catch (...) {
  }
}
} // namespace msime::windows
