#pragma once
#include <filesystem>

// 设置窗口读写的数据目录（main.cpp 的 state_directory），必须和 Server 的状态根是同一个，否则开关写进 Server 从不读取的 preferences.json：开始菜单的快捷方式直接启动 msime-client-settings.exe，不带 Server 注入的环境变量，以前就这样落到了 %LOCALAPPDATA%\MSIME-Client，而默认安装的 Server 用的是安装器记在 HKLM 的 DataDir。不依赖 Windows 和 WinRT 头文件，方便单独测试；三个输入由 main.cpp 取得。
namespace msime::settings {

// injected 是 Server 启动本窗口时注入的 MSIME_CLIENT_STATE_DIR，就是 Server 自己的状态根，绝对路径时直接用。否则 server_root 是 common/StateDirectory.h 的 resolve_state_directory()（本版本的数据目录环境变量、安装器在 HKLM 记下的 DataDir、%LOCALAPPDATA%\<本版本的状态目录>），configured 是 server_root 下 runtime-options.json 的 preferences_directory，不是绝对路径（包括没有这个字段）时不用：这与 server_main.cpp 的 production_preview_document 和 Tauri 外壳的 windows_server_preferences_directory 一样，状态根是 preferences_directory，没有时是 server_root。
inline std::filesystem::path settings_state_directory(std::filesystem::path const &injected,
                                                      std::filesystem::path const &server_root,
                                                      std::filesystem::path const &configured) {
  if (injected.is_absolute())
    return injected;
  if (configured.is_absolute())
    return configured;
  return server_root;
}

} // namespace msime::settings
