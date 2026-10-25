#pragma once

#include <windows.h>
#include <shlobj.h>
#include <filesystem>

#include "../../../shared/contracts/msime_edition.h"

namespace msime::windows {
// 后台更新检查的记录目录（共享层的 update-check.json 和 update-check.lock）：%LOCALAPPDATA%\<本版本的用户目录>\update（full 是 %LOCALAPPDATA%\MSIME\update，版本表 platforms.windows.user_data_directory）。Server 的后台检查往这里写，设置窗口「关于」页读 `msime_client_update_pending` 也读这里，两边必须用这同一个函数。不放状态根：安装后的状态根是安装器的 DataDir，整台机器一个目录，几个 Windows 用户共用一份记录时，一个用户被提醒过的构建在别的用户那里也算提醒过了，他们永远收不到通知。与匿名账号目录（server_main.cpp 的 anonymous_account_directory）同一个父目录，每个版本各自一份。known folder 查询失败时返回空，调用方不做后台检查、也不读缓存。
inline std::filesystem::path update_record_directory() {
  PWSTR local = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    CoTaskMemFree(local);
    return {};
  }
  const auto directory = std::filesystem::path(local) / MSIME_EDITION_USER_DATA_DIRECTORY / L"update";
  CoTaskMemFree(local);
  return directory;
}
} // namespace msime::windows
