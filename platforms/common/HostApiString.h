#pragma once

#include "msime_client.h"

#include <memory>

namespace msime::host_api {

using OwnedString = std::unique_ptr<char, decltype(&msime_client_string_free)>;

inline OwnedString own_string(char *raw) noexcept {
  return {raw, msime_client_string_free};
}

inline void discard_string(char *raw) noexcept {
  msime_client_string_free(raw);
}

} // namespace msime::host_api
