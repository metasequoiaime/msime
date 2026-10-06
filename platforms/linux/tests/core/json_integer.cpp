#include "../src/core/JsonInteger.h"

#include <cstdlib>
#include <cstdint>
#include <iostream>
#include <limits>
#include <nlohmann/json.hpp>

using Json = nlohmann::json;

namespace {
void require(bool condition, const char *message) {
  if (!condition) {
    std::cerr << message << '\n';
    std::exit(1);
  }
}
} // namespace

int main() {
  require(msime::linux_host::strict_json_integer<int>(Json(7), -1) == 7,
          "signed JSON integers are accepted");
  require(msime::linux_host::strict_json_integer<int>(Json(7.0), -1) == -1,
          "floating JSON integers are rejected");
  require(msime::linux_host::strict_json_integer<int>(Json(7.5), -1) == -1,
          "fractional JSON numbers are rejected");
  require(msime::linux_host::strict_json_integer<int>(Json(true), -1) == -1,
          "boolean JSON values are rejected");
  require(msime::linux_host::strict_json_integer<int>(Json("7"), -1) == -1,
          "numeric strings are rejected");
  require(msime::linux_host::strict_json_integer<int>(
              Json(std::numeric_limits<int64_t>::max()), -1) == -1,
          "out of range signed JSON integers are rejected");
  require(msime::linux_host::strict_json_integer<uint64_t>(Json(7), 0) == 7,
          "unsigned JSON integers are accepted");
  require(msime::linux_host::strict_json_integer<uint64_t>(Json(-1), 9) == 9,
          "negative JSON integers are rejected for unsigned values");
  require(msime::linux_host::strict_json_integer<uint64_t>(Json(7.0), 9) == 9,
          "floating JSON values are rejected for unsigned values");
  require(msime::linux_host::strict_json_value<int>(Json{{"value", 8}}, "value", -1) == 8,
          "object integer fields are read strictly");
  require(msime::linux_host::strict_json_value<int>(Json{{"value", 8.5}}, "value", -1) == -1,
          "object floating fields use the fallback");
  require(msime::linux_host::strict_json_value<int>(Json::object(), "value", 4) == 4,
          "missing object fields use the fallback");
}
