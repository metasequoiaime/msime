#pragma once
#include <nlohmann/json.hpp>

namespace msime::windows {
// Whether the shared `usage_reporting` preference lets the Server report. Reporting is on by default, so a document without the key (the shared store leaves it out while it is on, and documents from before the switch never had it) is on; an explicit false is off. What cannot be read as the switch - a malformed value, a document that is not an object - is off, because a choice the Server cannot read must not be overridden. The retired `telemetry_enabled` key is not read.
inline bool usage_reporting_enabled(const nlohmann::json &preferences) {
  if (!preferences.is_object())
    return false;
  const auto value = preferences.find("usage_reporting");
  if (value == preferences.end())
    return true;
  return value->is_boolean() && value->get<bool>();
}
} // namespace msime::windows
