#include "../../HostPathEncoding.h"
#include <cassert>

int main() {
  using msime::tsf::path_to_utf8;
  assert(path_to_utf8({}).empty());
  assert(path_to_utf8(std::filesystem::path("fixture")) == "fixture");
#if defined(__cpp_char8_t)
  const auto path = std::filesystem::path(u8"fixture-\u8def\u5f84-\U0001f332");
#else
  const auto path = std::filesystem::u8path(u8"fixture-\u8def\u5f84-\U0001f332");
#endif
  const std::string expected = "fixture-\xe8\xb7\xaf\xe5\xbe\x84-\xf0\x9f\x8c\xb2";
  assert(path_to_utf8(path) == expected);
  assert(path_to_utf8(path.filename()) == expected);
}
