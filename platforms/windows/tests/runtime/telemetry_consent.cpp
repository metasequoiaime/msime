#include "TelemetryConsent.h"
#include <stdexcept>

using msime::windows::usage_reporting_enabled;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Usage reporting consent validation failed");
}
int main() {
  // On by default: the shared store omits the key while it is on, and older documents never had it.
  require(usage_reporting_enabled(nlohmann::json::object()));
  require(usage_reporting_enabled(nlohmann::json{{"clipboard_history", true}}));
  require(usage_reporting_enabled(nlohmann::json{{"usage_reporting", true}}));
  // The user's off always wins.
  require(!usage_reporting_enabled(nlohmann::json{{"usage_reporting", false}}));
  // The retired opt-in key decides nothing any more, either way.
  require(usage_reporting_enabled(nlohmann::json{{"telemetry_enabled", false}}));
  require(!usage_reporting_enabled(nlohmann::json{{"telemetry_enabled", true}, {"usage_reporting", false}}));
  // A value or document that cannot be read as the switch is off rather than an error at startup.
  require(!usage_reporting_enabled(nlohmann::json{{"usage_reporting", "true"}}));
  require(!usage_reporting_enabled(nlohmann::json{{"usage_reporting", 1}}));
  require(!usage_reporting_enabled(nlohmann::json{{"usage_reporting", nullptr}}));
  require(!usage_reporting_enabled(nlohmann::json()));
  require(!usage_reporting_enabled(nlohmann::json::array({true})));
  return 0;
}
