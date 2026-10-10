#pragma once
#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include <nlohmann/json.hpp>
#include <optional>
#include <stdexcept>
#include <string>

namespace msime::windows {
// Immutable validated publication value. Loading may block on the shared store
// lock: call on a settings worker, never inside an input task/focus callback.
class PreferenceSnapshot final {
public:
  static PreferenceSnapshot load(const std::string &directory) {
    return *read(directory, false);
  }
  // Null means lock contention only. Keep the prior publication and retry.
  static std::optional<PreferenceSnapshot>
  try_load(const std::string &directory) {
    return read(directory, true);
  }
  uint64_t revision() const { return revision_; }
  const std::string &serialized() const { return serialized_; }

private:
  static std::optional<PreferenceSnapshot> read(const std::string &directory,
                                                bool try_only) {
    const auto load = try_only ? msime_client_try_load_preferences
                               : msime_client_load_preferences;
    auto raw = msime::host_api::own_string(
        load(reinterpret_cast<const uint8_t *>(directory.data()),
             directory.size()));
    if (!raw)
      throw std::runtime_error("Missing shared preferences response");
    auto response = nlohmann::json::parse(raw.get());
    if (!response.at("ok").get<bool>())
      throw std::runtime_error("Shared preferences load failed");
    auto value = response.at("value");
    if (try_only && value.is_null())
      return std::nullopt;
    const auto revision = value.at("revision").get<uint64_t>();
    auto serialized = value.dump();
    if (serialized.size() > 16384)
      throw std::runtime_error("Shared preferences snapshot too large");
    return PreferenceSnapshot(revision, std::move(serialized));
  }
  PreferenceSnapshot(uint64_t revision, std::string serialized)
      : revision_(revision), serialized_(std::move(serialized)) {}
  uint64_t revision_;
  std::string serialized_;
};
} // namespace msime::windows
