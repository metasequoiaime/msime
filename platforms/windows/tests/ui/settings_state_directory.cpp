#include "../../settings/SettingsStateDirectory.h"
#include <cstdio>
#include <filesystem>
#include <stdexcept>
#include <string>

using msime::settings::settings_state_directory;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("Settings state directory test failed at line " +
                             std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)
int main() {
  try {
    // 绝对路径按运行测试的主机算（Windows 上是盘符路径，交叉编译的检查在 POSIX 上跑），所以从临时目录拼出来，而不是写死 C:\。
    const auto base = std::filesystem::temp_directory_path();
    const auto injected = base / "server-launched";
    const auto root = base / "installer-data-dir";
    const auto configured = base / "configured-preferences";
    const std::filesystem::path none;
    const auto relative = std::filesystem::path("relative") / "state";

    // Server 启动本窗口时注入的状态目录优先于其他一切：它就是 Server 自己算好的状态根。
    require(settings_state_directory(injected, root, configured) == injected);
    require(settings_state_directory(injected, root, none) == injected);

    // 开始菜单启动没有注入：用 Server 自己解析的状态根，运行时选项写了绝对的 preferences_directory 时用它，与 production_preview_document 相同。以前这里回落到 %LOCALAPPDATA%\MSIME-Client，和默认安装的 Server 不是同一个目录。
    require(settings_state_directory(none, root, configured) == configured);
    require(settings_state_directory(none, root, none) == root);

    // 相对路径的注入值和 preferences_directory 都不采用，不按工作目录去解析。
    require(settings_state_directory(relative, root, configured) == configured);
    require(settings_state_directory(relative, root, none) == root);
    require(settings_state_directory(none, root, relative) == root);

    // 找不到状态根（known folder 查询失败）且没有其他来源时为空，调用方据此说明找不到数据目录。
    require(settings_state_directory(none, none, none).empty());

    std::puts("Settings state directory tests passed");
    return 0;
  } catch (const std::exception &error) {
    std::fputs(error.what(), stderr);
    std::fputc('\n', stderr);
    return 1;
  }
}
