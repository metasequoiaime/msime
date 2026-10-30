#pragma once

#include <windows.h>
#include <shlobj.h>
#include <filesystem>

#include "../../../shared/contracts/msime_edition.h"

namespace msime::windows {
// 「我的词条」经 WebDAV 网盘同步的本机目录（`msime_client_dictionary_sync` 的 `directory`）：%LOCALAPPDATA%\<本版本的用户目录>\dictionary-sync（full 是 %LOCALAPPDATA%\MSIME\dictionary-sync，版本表 platforms.windows.user_data_directory）。里面是同步状态（含词条明文）、词库自己的密钥串和两把锁，同一个安装只能有这一份：Server 的自动同步、WinUI 设置窗口、Win7 设置窗口和 Win7 词库同步面板都调这同一个函数，否则同一台电脑会在网盘上登记成几台设备、要批准几次。与匿名账号目录（account）同一个父目录，但不能就用 account：云剪贴板中继的密钥串也放在那里，文件名同样是 cloud-clipboard-relay-keystore，两个功能的身份必须分开。不放状态根：安装后的状态根是安装器的 DataDir，整台机器一个目录、每个用户都能改，词条明文和设备私钥只能放在本用户自己的目录里。known folder 查询失败时返回空，调用方不同步。
inline std::filesystem::path dictionary_sync_directory() {
  PWSTR local = nullptr;
  if (FAILED(SHGetKnownFolderPath(FOLDERID_LocalAppData, 0, nullptr, &local))) {
    CoTaskMemFree(local);
    return {};
  }
  const auto directory = std::filesystem::path(local) / MSIME_EDITION_USER_DATA_DIRECTORY / L"dictionary-sync";
  CoTaskMemFree(local);
  return directory;
}
} // namespace msime::windows
