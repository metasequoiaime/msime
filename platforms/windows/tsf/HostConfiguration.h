#pragma once

#include "../common/InputSchemeTraits.h"
#include <filesystem>
#include <memory>
#include <cstdint>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::tsf {
struct HostConfiguration {
  std::uint64_t revision = 0;
  std::string directory;
  nlohmann::json preferences;
  std::string preferenceSnapshot;
  std::string hostOptions;
  // hostOptions 里的 user_data：词库维护租约写在它下面（../common/DictionaryQuiesceLease.h）。没有或不是绝对路径时为空，TIP 就当作没有租约。
  std::filesystem::path userData;
  msime::windows::scheme::LanguageDictionaryPresence dictionaries;
};

// 首次激活显式准备一次；此后调用只读取已经发布的不可变内存。
void initialize_host_configuration();
std::shared_ptr<const HostConfiguration> cached_host_configuration();
void refresh_host_configuration();
// 已保存方案的通知触发后台重读；不在通知线程读取文件或应用引擎偏好。
void invalidate_host_configuration();
// 游戏策略在激活前锁定：逐次读取最新规则，避免停用期间的旧快照忽略排除项。
std::shared_ptr<const nlohmann::json> load_activation_preferences();
}
