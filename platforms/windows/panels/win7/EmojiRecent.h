#pragma once

#include "EmojiCatalog.h"
#include <algorithm>

namespace msime::windows::win7::emoji {
struct Handle {
  HANDLE value = INVALID_HANDLE_VALUE;
  explicit Handle(HANDLE handle) : value(handle) {}
  ~Handle() { if (value != INVALID_HANDLE_VALUE) CloseHandle(value); }
  Handle(const Handle&) = delete;
  Handle& operator=(const Handle&) = delete;
};
struct Recent { std::vector<Item> items; bool readable = true; };
inline Recent read_recent(const std::filesystem::path& directory) {
  Recent result;
  const auto path = directory / L"emoji-panel-recent.json";
  Handle file(CreateFileW(path.c_str(), GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr, OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT, nullptr));
  if (file.value == INVALID_HANDLE_VALUE) {
    const auto error = GetLastError(); result.readable = error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND; return result;
  }
  BY_HANDLE_FILE_INFORMATION info{}; LARGE_INTEGER size{};
  if (!GetFileInformationByHandle(file.value,&info) || (info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)) ||
      !GetFileSizeEx(file.value,&size) || size.QuadPart <= 0 || size.QuadPart > 65536) { result.readable = false; return result; }
  std::string bytes(static_cast<size_t>(size.QuadPart),'\0'); DWORD count = 0;
  if (!ReadFile(file.value,bytes.data(),static_cast<DWORD>(bytes.size()),&count,nullptr) || count != bytes.size()) { result.readable = false; return result; }
  const auto document = Json::parse(bytes,nullptr,false);
  if (!document.is_object() || !document.contains("version") || document.at("version") != 1 || !document.contains("items") || !document.at("items").is_array() || document.at("items").size() > 56) { result.readable = false; return result; }
  size_t emoji_count = 0, symbol_count = 0;
  for (const auto& item : document.at("items")) {
    if (!item.is_object()) { result.readable = false; break; }
    for (const auto* key : {"text", "annotation", "tab"}) if (!item.contains(key) || !valid_text(item.at(key))) result.readable = false;
    if (!result.readable) break;
    const auto tab = item.at("tab").get<std::string>();
    if ((tab != "emoji" && tab != "kaomoji" && tab != "symbols") || item.at("text").get_ref<const std::string&>().empty() ||
        (tab == "symbols" ? ++symbol_count : ++emoji_count) > 28) { result.readable = false; break; }
    result.items.push_back({item.at("text"),item.at("annotation"),tab,"recent"});
  }
  if (!result.readable) result.items.clear();
  return result;
}
inline Recent remember(const std::filesystem::path& directory, const Item& item) {
  // 最近使用只是界面元数据；串行占有固定锁文件后重读，损坏文件绝不自动覆盖。
  const auto lock_path = directory / L"emoji-panel-recent.lock";
  Handle lock(CreateFileW(lock_path.c_str(),GENERIC_READ | GENERIC_WRITE,0,nullptr,OPEN_ALWAYS,FILE_FLAG_OPEN_REPARSE_POINT,nullptr));
  if (lock.value == INVALID_HANDLE_VALUE) return {{},false};
  BY_HANDLE_FILE_INFORMATION info{};
  if (!GetFileInformationByHandle(lock.value,&info) || (info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY))) return {{},false};
  auto result = read_recent(directory); if (!result.readable) return result;
  result.items.erase(std::remove_if(result.items.begin(),result.items.end(),[&](const Item& prior) {
    return (prior.tab == "symbols") == (item.tab == "symbols") && prior.text == item.text;
  }),result.items.end());
  result.items.insert(result.items.begin(),item);
  size_t emoji_count = 0, symbol_count = 0;
  result.items.erase(std::remove_if(result.items.begin(),result.items.end(),[&](const Item& current) { return (current.tab == "symbols" ? ++symbol_count : ++emoji_count) > 28; }),result.items.end());
  Json items = Json::array(); for (const auto& current : result.items) items.push_back({{"text",current.text},{"annotation",current.annotation},{"tab",current.tab}});
  const auto bytes = Json{{"version",1},{"items",items}}.dump(); if (bytes.size() > 65536) return {{},false};
  const auto temporary = directory / (L"emoji-panel-recent." + std::to_wstring(GetCurrentProcessId()) + L"." + std::to_wstring(GetTickCount64()) + L".tmp");
  bool written = false;
  {
    Handle file(CreateFileW(temporary.c_str(),GENERIC_WRITE,0,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr));
    if (file.value == INVALID_HANDLE_VALUE) return {{},false};
    DWORD count = 0; written = WriteFile(file.value,bytes.data(),static_cast<DWORD>(bytes.size()),&count,nullptr) && count == bytes.size() && FlushFileBuffers(file.value);
  }
  const auto path = directory / L"emoji-panel-recent.json";
  if (!written || !MoveFileExW(temporary.c_str(),path.c_str(),MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
    DeleteFileW(temporary.c_str()); return {{},false};
  }
  return result;
}
}
