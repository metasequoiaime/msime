#pragma once

#include "msime_client.h"
#include "../../common/HostApiString.h"
#include <nlohmann/json.hpp>
#include <stdexcept>

namespace msime::windows {
inline nlohmann::json panel_response(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) throw std::runtime_error("Missing panel host response");
  const auto document = nlohmann::json::parse(owned.get());
  if (!document.at("ok").get<bool>()) throw std::runtime_error("Panel host operation failed");
  return document.at("value");
}

// TIP 与 Server 复用同一个来源/布局切换规则；候选与多义解码仍只由共享引擎负责。
inline nlohmann::json panel_character(uint64_t session, const nlohmann::json &current,
                                      uint8_t letter, bool shift, bool grouped) {
  const bool empty = current.at("editing_text").get_ref<const std::string &>().empty() &&
                     current.value("phrase_prefix", std::string{}).empty();
  const auto grid = current.value("key_grid", std::string{"none"});
  if (grouped) {
    if (current.value("scheme", 0u) != 0 || current.value("dedicated_english", false) ||
        current.value("local_mode", std::string{"none"}) != "none" ||
        shift || letter < 'a' || letter > 'z' || (!empty && grid != "eighteen_key"))
      throw std::invalid_argument("Invalid grouped panel character state");
    if (grid != "eighteen_key") (void)panel_response(msime_client_set_key_grid(session, 4));
    return panel_response(msime_client_grid_key(session, letter));
  }
  const bool physical_letter = (letter >= 'a' && letter <= 'z') || (letter >= 'A' && letter <= 'Z');
  std::string committed;
  if (grid == "eighteen_key" && !empty && physical_letter) {
    // 确定字母不能掺进多义组码：先确认当前高亮候选，再开始确定全拼，避免吞掉硬件键。
    auto view = current;
    for (unsigned count = 0; count < 128; ++count) {
      if (view.at("candidates").empty()) {
        // 没有读音可选时只保留已选词段，丢弃未完成组码，不能上屏组首字母。
        committed += view.value("phrase_prefix", std::string{});
        (void)panel_response(msime_client_command(session, MSIME_CANCEL));
        break;
      }
      const auto selected = panel_response(msime_client_command(session, MSIME_COMMIT_CANDIDATE));
      if (selected.at("commit").is_string()) committed += selected.at("commit").get<std::string>();
      const auto &after = selected.at("view");
      if (after.at("editing_text").get_ref<const std::string &>().empty() &&
          after.value("phrase_prefix", std::string{}).empty()) break;
      if (after.at("editing_text") == view.at("editing_text") &&
          after.value("phrase_prefix", std::string{}) == view.value("phrase_prefix", std::string{}))
        throw std::runtime_error("Panel composition cannot finish");
      view = after;
      if (count == 127) throw std::runtime_error("Panel composition exceeded finish limit");
    }
  }
  if (grid == "eighteen_key" && (empty || physical_letter))
    (void)panel_response(msime_client_set_key_grid(session, 0));
  auto result = panel_response(msime_client_character(session, letter, shift));
  if (!committed.empty()) {
    result["commit"] = committed + (result.at("commit").is_string()
        ? result.at("commit").get<std::string>() : std::string{});
    result["handled"] = true;
  }
  return result;
}
} // namespace msime::windows
