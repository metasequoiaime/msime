#pragma once

#include "../../../common/EmbeddedTranslationRetry.h"
#include <nlohmann/json.hpp>

namespace msime::linux_host {
inline msime::host_api::OwnedString embedded_translation(
    nlohmann::json query, const msime::translation::EmbeddedState::Token &cancelled) {
  const auto id = msime::translation::next_request_id();
  query["request_id"] = id;
  return msime::translation::embedded_with_retry(std::move(query), id, [cancelled] { return cancelled->load(); });
}
} // namespace msime::linux_host
