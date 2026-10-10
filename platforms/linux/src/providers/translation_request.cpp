#include "msime_client.h"
#include "provider_socket_cli.h"

#include <array>
#include <iostream>
#include <memory>
#include "provider_response_cli.h"
#include <string>

int main(int argc, char **argv) {
  const auto socket_path = msime_cli_provider_socket(
      argc, argv, "MSIME_TRANSLATION_PROVIDER_SOCKET", "translation.sock");
  if (socket_path.empty()) return 2;
  std::array<char, 16385> buffer;
  std::cin.read(buffer.data(), buffer.size());
  const auto length = static_cast<size_t>(std::cin.gcount());
  if (std::cin.bad() || length == 0 || length > 16384)
    return 2;
  std::unique_ptr<char, decltype(&msime_client_string_free)> result(
      msime_client_translation_provider_request(
          reinterpret_cast<const uint8_t *>(buffer.data()), length,
          reinterpret_cast<const uint8_t *>(socket_path.data()),
          socket_path.size()),
      msime_client_string_free);
  return msime_cli_write_provider_response(result.get(), std::cout);
}
