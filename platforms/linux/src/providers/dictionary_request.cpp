#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../core/BoundedCliInput.h"
#include "provider_response_cli.h"
#include <iostream>
#include <array>
#include <string>

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-dictionary < request.json\n";
    return 0;
  }
  if (argc != 1)
    return 2;
  std::array<char, 65537> buffer;
  const auto request_length = msime::linux_host::read_bounded_cli_input(std::cin, buffer);
  if (!request_length)
    return 2;
  const size_t length = *request_length;
  auto result = msime::host_api::own_string(
      msime_client_dictionary(reinterpret_cast<const uint8_t *>(buffer.data()),
                              length));
  return msime_cli_write_provider_response(result.get(), std::cout);
}
