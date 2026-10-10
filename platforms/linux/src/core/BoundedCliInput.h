#pragma once

#include <array>
#include <cstddef>
#include <istream>
#include <optional>

namespace msime::linux_host {

// Read one byte beyond the accepted limit so oversized input is rejected.
template <std::size_t N>
inline std::optional<std::size_t> read_bounded_cli_input(
    std::istream &input, std::array<char, N> &buffer) {
  static_assert(N > 1);
  input.read(buffer.data(), buffer.size());
  const auto length = static_cast<std::size_t>(input.gcount());
  if (input.bad() || length == 0 || length >= N)
    return std::nullopt;
  return length;
}

} // namespace msime::linux_host
