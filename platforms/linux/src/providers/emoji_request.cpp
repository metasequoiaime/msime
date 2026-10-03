#include "msime_client.h"
#include "../core/LocalResourcePaths.h"
#include "provider_socket_cli.h"

#include <array>
#include <iostream>
#include <memory>
#include <nlohmann/json.hpp>
#include <string>

std::string local_resources(int argc, char **argv, bool *local) {
  *local = argc >= 2 && std::string(argv[1]) == "--local";
  if (!*local)
    return msime_cli_provider_socket(argc, argv, "MSIME_EMOJI_PROVIDER_SOCKET", "emoji.sock");
  if (argc == 3)
    return argv[2];
  if (argc != 2)
    return {};
  return msime_linux::local_resource(
      "MSIME_EMOJI_RESOURCES", MSIME_EDITION_CLIENT_DIRECTORY "/emoji/others.db", true);
}

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-emoji [provider-socket] | --local [resources]\n";
    return 0;
  }
  bool local = false;
  const auto target = local_resources(argc, argv, &local);
  if (target.empty() || target[0] != '/')
    return 2;
  std::array<char, 16385> buffer;
  std::cin.read(buffer.data(), buffer.size());
  const auto length = static_cast<size_t>(std::cin.gcount());
  if (std::cin.bad() || length == 0 || length > 16384)
    return 2;
  std::unique_ptr<char, decltype(&msime_client_string_free)> result(
      local ? msime_client_emoji_catalog_request(
                  reinterpret_cast<const uint8_t *>(buffer.data()), length,
                  reinterpret_cast<const uint8_t *>(target.data()), target.size())
            : msime_client_emoji_provider_request(
                  reinterpret_cast<const uint8_t *>(buffer.data()), length,
                  reinterpret_cast<const uint8_t *>(target.data()), target.size()),
      msime_client_string_free);
  if (!result)
    return 1;
  try {
    auto document = nlohmann::json::parse(result.get());
    const bool ok = document.at("ok").get<bool>();
    std::cout << document.dump() << '\n';
    return std::cout ? (ok ? 0 : 1) : 1;
  } catch (...) {
    return 1;
  }
}
