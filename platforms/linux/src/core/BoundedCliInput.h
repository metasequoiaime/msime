#pragma once

#include <array>
#include <cstddef>
#include <istream>
#include <optional>

namespace msime::linux_host {

// Read one byte beyond the accepted limit so oversized input is rejected.
// Most commands require a nonempty request; callers with an empty no-op use 0.
template <std::size_t N>
inline std::optional<std::size_t> read_bounded_cli_input(
    std::istream &input, std::array<char, N> &buffer,
    std::size_t minimum_length = 1) {
  static_assert(N > 1);
  input.read(buffer.data(), buffer.size());
  const auto length = static_cast<std::size_t>(input.gcount());
  if (input.bad() || length < minimum_length || length >= N)
    return std::nullopt;
  return length;
}

} // namespace msime::linux_host
