#pragma once
#include "msime_client.h"
#include "../../platforms/common/HostApiString.h"
#include <nlohmann/json.hpp>
#include <optional>
#include <string>
#include <string_view>

namespace msime::voice {
// Only adapts the shared Rust policy to native HTTP header text. The result
// contains credentials: do not include it in diagnostics or persistence.
inline std::optional<std::string> doubao_auth_headers(
    std::string_view mode, std::string_view app_id, std::string_view token,
    std::string_view resource_id) {
  const auto request = nlohmann::json{{"auth_mode", mode}, {"app_id", app_id},
                                     {"token", token}, {"resource_id", resource_id}}.dump();
  auto raw = msime::host_api::own_string(msime_client_doubao_auth_headers(
      reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  if (!raw) return std::nullopt;
  const auto response = nlohmann::json::parse(raw.get(), nullptr, false);
  if (response.is_discarded() || !response.value("ok", false)) return std::nullopt;
  std::string headers;
  for (const auto &header : response.at("value").at("headers"))
    headers += header.at(0).get<std::string>() + ": " + header.at(1).get<std::string>() + "\r\n";
  return headers;
}
} // namespace msime::voice
