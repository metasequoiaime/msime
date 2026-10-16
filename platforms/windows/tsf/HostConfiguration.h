#pragma once

#include "../common/InputSchemeTraits.h"
#include <memory>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::tsf {
struct HostConfiguration {
  std::string directory;
  nlohmann::json preferences;
  std::string preferenceSnapshot;
  std::string hostOptions;
  msime::windows::scheme::LanguageDictionaryPresence dictionaries;
};

// 首次激活显式准备一次；此后调用只读取已经发布的不可变内存。
void initialize_host_configuration();
std::shared_ptr<const HostConfiguration> cached_host_configuration();
void refresh_host_configuration();
// 游戏策略在激活前锁定：逐次读取最新规则，避免停用期间的旧快照忽略排除项。
std::shared_ptr<const nlohmann::json> load_activation_preferences();
}
