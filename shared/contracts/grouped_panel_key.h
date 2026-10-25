#pragma once

#include <cstddef>
#include <cstdint>

namespace msime::panel_key {
// WM_COPYDATA 的标识和载荷均不含指针，x86、x64 与 ARM64 使用同一布局。
inline constexpr std::uint32_t CopyDataId = 0x4d53474b;
inline constexpr std::uint32_t Version = 1;
enum class Operation : std::uint32_t { Query = 1, Press = 2 };
struct Request {
  std::uint32_t version = Version;
  Operation operation = Operation::Query;
  std::uint64_t nonce = 0;
  std::uint32_t letter = 0;
  std::uint32_t reserved = 0;
};
static_assert(sizeof(Request) == 24 && offsetof(Request, nonce) == 8 &&
              offsetof(Request, letter) == 16);

inline constexpr bool valid(const Request &request) {
  return request.version == Version && request.nonce != 0 && request.reserved == 0 &&
         ((request.operation == Operation::Query && request.letter == 0) ||
          (request.operation == Operation::Press && request.letter >= 'a' && request.letter <= 'z'));
}
} // namespace msime::panel_key
