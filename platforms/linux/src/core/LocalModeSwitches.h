#pragma once

#include <string_view>

namespace msime::linux_host {

// 偏好文档缺这个本地模式的开关时，两个宿主的状态菜单把它读成什么。以 client-core 的 LocalModePreferences 为准：Engine 自己的模式默认开；计算与数字（V）、指令（/）和名单（@）在用户打开之前是关的，关着时文档里没有这几个键；拆字反查（C）的键只在用户动过开关后才出现，缺键时 host-api 在桌面构建里按关处理。缺键读成开，就会给 Engine 并没有运行的模式打上勾。
inline bool local_mode_enabled_by_default(std::string_view key) {
  return key != "expression" && key != "command" && key != "mention" && key != "chaizi";
}

} // namespace msime::linux_host
