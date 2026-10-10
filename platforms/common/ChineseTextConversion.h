#pragma once

#include "HostApiString.h"

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>

namespace msime::host_api {

inline std::optional<std::string> simplified_to_traditional(std::string_view text) {
  auto converted = own_string(msime_client_simplified_to_traditional(
      reinterpret_cast<const uint8_t *>(text.data()), text.size()));
  if (!converted)
    return std::nullopt;
  return std::string(converted.get());
}

} // namespace msime::host_api
