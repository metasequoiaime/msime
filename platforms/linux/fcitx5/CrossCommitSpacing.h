#pragma once

#include "../../common/HostApiString.h"

#include <cstdint>
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>

namespace msime::fcitx_host {

// 模式、应用排除与中英文边界由 Host API 判断，插件只提供客户端实报的前文。
inline std::optional<std::string> format_cross_commit(
    const nlohmann::json &preferences, std::string_view application,
    const nlohmann::json &context, const std::string &text, std::uint32_t preceding) {
  using Json = nlohmann::json;
  const auto request = Json{
      {"config", {{"enabled", true},
                  {"excluded_apps", preferences.value("cross_commit_spacing_excluded_apps", Json::array())}}},
      {"application", application}, {"context", context},
      {"text", text}, {"preceding", preceding}}.dump();
  const auto raw = host_api::own_string(msime_client_cross_commit_spacing(
      reinterpret_cast<const std::uint8_t *>(request.data()), request.size()));
  if (!raw) return std::nullopt;
  const auto response = Json::parse(raw.get(), nullptr, false);
  if (!response.is_object() || !response.value("ok", false)) return std::nullopt;
  const auto &result = response.at("value").at("text").get_ref<const std::string &>();
  if (result == text) return std::nullopt;
  return result;
}

} // namespace msime::fcitx_host
