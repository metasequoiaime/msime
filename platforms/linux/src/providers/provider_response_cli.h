#pragma once

#include <nlohmann/json.hpp>

#include <ostream>

inline int msime_cli_write_provider_response(const char *result, std::ostream &output) {
  if (!result)
    return 1;
  try {
    auto document = nlohmann::json::parse(result);
    const bool ok = document.at("ok").get<bool>();
    output << document.dump() << '\n';
    return output ? (ok ? 0 : 1) : 1;
  } catch (...) {
    // Parser errors and request data may contain user entries.
    return 1;
  }
}
