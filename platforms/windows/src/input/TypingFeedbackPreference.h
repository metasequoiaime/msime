#pragma once
#include <nlohmann/json.hpp>
#include <string>

namespace msime::windows {
// 交给应用的键（Aux 管道的 KeySound）有没有用：按键音开着，或者打字特效画点什么（选了样式、选了特效包，或者开了连击计数）。都关着时 Server 不回 "OK"，TIP 就停一阵不发，用户没开这些功能时不会每个键都连一次管道。读不懂的字段当作关着。
inline bool typing_feedback_wanted(const nlohmann::json &preferences) {
  if (!preferences.is_object())
    return false;
  const auto plugins = preferences.find("plugins");
  if (plugins == preferences.end() || !plugins->is_object())
    return false;
  const auto flag = [&](const nlohmann::json &object, const char *key) {
    const auto found = object.find(key);
    return found != object.end() && found->is_boolean() && found->get<bool>();
  };
  const auto text = [&](const char *key) {
    const auto found = plugins->find(key);
    return found != plugins->end() && found->is_string() ? found->get<std::string>() : std::string{};
  };
  if (const auto key_sound = plugins->find("key_sound"); key_sound != plugins->end() && key_sound->is_object() &&
                                                         flag(*key_sound, "enabled"))
    return true;
  const auto style = text("effect_style");
  return style == "flash" || style == "sparks" || style == "power_mode" || !text("effect_pack").empty() ||
         flag(*plugins, "combo_counter");
}
} // namespace msime::windows
