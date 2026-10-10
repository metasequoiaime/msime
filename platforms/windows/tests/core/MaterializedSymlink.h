#pragma once

#include <cstdio>
#include <filesystem>
#include <stdexcept>
#include <windows.h>

namespace msime::windows::tests {

inline bool running_under_wine() {
  const HMODULE ntdll = GetModuleHandleW(L"ntdll.dll");
  return ntdll && GetProcAddress(ntdll, "wine_get_version") != nullptr;
}

// Wine can report success from CreateSymbolicLinkW without creating an entry.
// Only run a reparse-point regression when Win32 can observe the actual link.
inline bool create_materialized_symlink(const std::filesystem::path &link,
                                        const std::filesystem::path &target,
                                        DWORD flags) {
  if (!CreateSymbolicLinkW(link.c_str(), target.c_str(), flags))
    return false; // Symlink privilege or filesystem support is unavailable.

  const DWORD attributes = GetFileAttributesW(link.c_str());
  const DWORD open_flags = FILE_FLAG_OPEN_REPARSE_POINT |
      ((flags & SYMBOLIC_LINK_FLAG_DIRECTORY) ? FILE_FLAG_BACKUP_SEMANTICS : 0);
  const HANDLE handle = CreateFileW(
      link.c_str(), FILE_READ_ATTRIBUTES,
      FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
      OPEN_EXISTING, open_flags, nullptr);
  FILE_ATTRIBUTE_TAG_INFO info{};
  const bool observed = attributes != INVALID_FILE_ATTRIBUTES &&
      (attributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 &&
      handle != INVALID_HANDLE_VALUE &&
      GetFileInformationByHandleEx(handle, FileAttributeTagInfo, &info,
                                   sizeof(info)) != 0 &&
      info.ReparseTag == IO_REPARSE_TAG_SYMLINK;
  if (handle != INVALID_HANDLE_VALUE)
    CloseHandle(handle);
  if (observed)
    return true;
  if (running_under_wine()) {
    std::fputs("skipped: Wine did not materialize a symbolic-link fixture\n",
               stderr);
    return false;
  }
  throw std::runtime_error("CreateSymbolicLinkW succeeded without a readable symbolic link");
}

} // namespace msime::windows::tests
