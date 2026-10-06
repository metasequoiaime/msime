#pragma once

#include <cstdint>
#include <limits>
#include <nlohmann/json.hpp>
#include <optional>
#include <stdexcept>
#include <string_view>
#include <type_traits>

namespace msime::linux_host {

// JSON numbers are allowed to be floating point values, and nlohmann::json's
// arithmetic get<T>() performs a C++ cast for them. Host state uses integers
// for identifiers and indexes, so accepting that cast would turn malformed
// values such as 3.9 into a different valid state. Read only JSON integer
// tokens and check the destination range explicitly.
template <typename T>
std::optional<T> strict_json_integer_optional(const nlohmann::json &value) {
  static_assert(std::is_integral_v<T>, "strict_json_integer requires an integer type");
  if (value.is_number_unsigned()) {
    const auto raw = value.get<uint64_t>();
    if constexpr (std::is_signed_v<T>) {
      if (raw > static_cast<uint64_t>(std::numeric_limits<T>::max()))
        return std::nullopt;
    }
    return static_cast<T>(raw);
  }
  if (value.is_number_integer()) {
    const auto raw = value.get<int64_t>();
    if constexpr (std::is_unsigned_v<T>) {
      if (raw < 0 || static_cast<uint64_t>(raw) > std::numeric_limits<T>::max())
        return std::nullopt;
    } else {
      if (raw < std::numeric_limits<T>::min() || raw > std::numeric_limits<T>::max())
        return std::nullopt;
    }
    return static_cast<T>(raw);
  }
  return std::nullopt;
}

template <typename T>
T strict_json_integer(const nlohmann::json &value, T fallback) {
  return strict_json_integer_optional<T>(value).value_or(fallback);
}

template <typename T>
T strict_json_value(const nlohmann::json &object, std::string_view key, T fallback) {
  if (!object.is_object()) return fallback;
  const auto found = object.find(std::string(key));
  return found == object.end() ? fallback : strict_json_integer(*found, fallback);
}

template <typename T>
T strict_json_required_integer(const nlohmann::json &value) {
  const auto parsed = strict_json_integer_optional<T>(value);
  if (parsed) return *parsed;
  throw std::invalid_argument("expected an in-range JSON integer");
}

} // namespace msime::linux_host
