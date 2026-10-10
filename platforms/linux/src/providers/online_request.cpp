#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../core/BoundedCliInput.h"
#include "provider_response_cli.h"
#include "provider_socket_cli.h"

#include <array>
#include <iostream>
#include <string>

int main(int argc, char **argv) {
  const auto socket_path = msime_cli_provider_socket(
      argc, argv, "MSIME_ONLINE_PROVIDER_SOCKET", "online.sock");
  if (socket_path.empty()) return 2;
  std::array<char, 16385> buffer;
  const auto request_length = msime::linux_host::read_bounded_cli_input(std::cin, buffer);
  if (!request_length)
    return 2;
  const size_t length = *request_length;
  auto result = msime::host_api::own_string(
      msime_client_online_provider_request(
          reinterpret_cast<const uint8_t *>(buffer.data()), length,
          reinterpret_cast<const uint8_t *>(socket_path.data()),
          socket_path.size()));
  return msime_cli_write_provider_response(result.get(), std::cout);
}
