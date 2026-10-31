#pragma once

#include "../../../common/VocabularyDueMenu.h"
#include "msime_client.h"

#include <cstdint>
#include <ctime>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>

namespace msime::linux_host {
// IBus 属性菜单和 Fcitx5 状态区里「背单词（今日待复习 N）…」那一行：按运行时配置读今日待复习，返回要显示的文字，不显示时为空。目录与设置应用的背单词页相同：偏好目录、偏好目录下的插件目录，内置词书的资源根交给 msime_client_vocabulary_due_count 按 HostOptions 求（host_options），词库在线更新后也和设置应用找的是同一处。两个宿主都只在获得焦点、属性重新注册时调一次，不在按键路径上，组字时连读都不读。
inline std::optional<std::string> vocabulary_due_label(const nlohmann::json &options, bool composing) {
  if (composing || !options.is_object())
    return std::nullopt;
  const auto directory = options.value("preferences_directory", std::string{});
  const auto day = msime::vocabulary_due::local_day(std::time(nullptr));
  if (directory.empty() || directory.front() != '/' || day.empty())
    return std::nullopt;
  // 只带求资源根要用的几个键：运行时配置里还有整份偏好，没有必要每次都序列化一遍。
  auto host_options = nlohmann::json::object();
  for (const char *key : {"resources", "user_data", "dictionaries", "preferences_directory", "edition"})
    if (const auto found = options.find(key); found != options.end())
      host_options[key] = *found;
  const auto request = nlohmann::json{{"directory", directory},
                                      {"host_options", host_options},
                                      {"plugins", directory + "/plugins"},
                                      {"day", day}}
                           .dump(-1, ' ', false, nlohmann::json::error_handler_t::replace);
  return msime::vocabulary_due::menu_label(
      msime_client_vocabulary_due_count(reinterpret_cast<const uint8_t *>(request.data()), request.size()),
      false);
}
} // namespace msime::linux_host
