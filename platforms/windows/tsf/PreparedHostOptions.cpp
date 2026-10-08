#include "HostOptionsPaths.h"
#include "../src/system/StateRootLease.h"

namespace msime::tsf {
std::string read_prepared_host_options(const std::filesystem::path &file) {
  // Match msime_client_create's limit; an extra byte detects truncation.
  constexpr std::size_t limit = 16384;
  const auto document = msime::windows::read_private_file(file, limit);
  return document && !document->empty() ? *document : std::string{};
}
}
