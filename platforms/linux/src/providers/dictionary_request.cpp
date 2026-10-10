#include "msime_client.h"
#include "provider_request_cli.h"
#include "provider_response_cli.h"
#include <iostream>
#include <array>
#include <memory>
#include <string>

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-dictionary < request.json\n";
    return 0;
  }
  if (argc != 1)
    return 2;
  std::array<char, 65537> buffer;
  const auto request_length = msime_cli_read_provider_request(std::cin, buffer);
  if (!request_length)
    return 2;
  const size_t length = *request_length;
  std::unique_ptr<char, decltype(&msime_client_string_free)> result(
      msime_client_dictionary(reinterpret_cast<const uint8_t *>(buffer.data()),
                              length),
      msime_client_string_free);
  return msime_cli_write_provider_response(result.get(), std::cout);
}
