#include "msime_client.h"
#include "../core/BoundedCliInput.h"
#include "provider_response_cli.h"
#include "provider_socket_cli.h"

#include <array>
#include <iostream>
#include <memory>
#include <string>

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-cloud-clipboard <provider-socket>\n";
    return 0;
  }
  const auto socket_path = msime_cli_provider_socket(
      argc, argv, "MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET", "cloud-clipboard.sock");
  if (socket_path.empty()) return 2;
  std::array<char, 65537> buffer;
  const auto request_length = msime::linux_host::read_bounded_cli_input(std::cin, buffer);
  if (!request_length)
    return 2;
  const size_t length = *request_length;
  std::unique_ptr<char, decltype(&msime_client_string_free)> result(
      msime_client_cloud_clipboard_provider_request(
          reinterpret_cast<const uint8_t *>(buffer.data()), length,
          reinterpret_cast<const uint8_t *>(socket_path.data()),
          socket_path.size()),
      msime_client_string_free);
  return msime_cli_write_provider_response(result.get(), std::cout);
}
