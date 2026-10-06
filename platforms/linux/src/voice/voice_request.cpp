#include "msime_client.h"
#include "../providers/provider_socket_cli.h"

#include <array>
#include <iostream>
#include <memory>
#include <nlohmann/json.hpp>
#include <string>

namespace {
void print_stream_update(const uint8_t *text, size_t length, bool final,
                         void *) {
  nlohmann::json update{{"text", std::string(reinterpret_cast<const char *>(text), length)},
                        {"type", final ? "final" : "partial"}};
  std::cout << update.dump() << '\n';
  std::cout.flush();
}
}

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-voice [--stream] [provider-socket]\n"
                 "Defaults to MSIME_VOICE_PROVIDER_SOCKET, then "
                 "$XDG_RUNTIME_DIR/" MSIME_EDITION_CLIENT_DIRECTORY "/voice.sock.\n";
    return 0;
  }
  const bool stream = argc >= 2 && std::string(argv[1]) == "--stream";
  const auto socket_path = msime_cli_provider_socket(
      argc - (stream ? 1 : 0), argv + (stream ? 1 : 0),
      "MSIME_VOICE_PROVIDER_SOCKET", "voice.sock");
  if (socket_path.empty())
    return 2;
  std::array<char, 16385> buffer;
  std::cin.read(buffer.data(), buffer.size());
  const auto length = static_cast<size_t>(std::cin.gcount());
  if (std::cin.bad() || length == 0 || length > 16384)
    return 2;
  if (stream) {
    std::unique_ptr<char, decltype(&msime_client_string_free)> result(
        msime_client_voice_provider_stream(
            reinterpret_cast<const uint8_t *>(buffer.data()), length,
            reinterpret_cast<const uint8_t *>(socket_path.data()),
            socket_path.size(), print_stream_update, nullptr),
        msime_client_string_free);
    if (!result)
      return 1;
    try {
      auto document = nlohmann::json::parse(result.get());
      return document.value("ok", false) ? 0 : 1;
    } catch (...) {
      return 1;
    }
  }
  std::unique_ptr<char, decltype(&msime_client_string_free)> result(
      msime_client_voice_provider_request(
          reinterpret_cast<const uint8_t *>(buffer.data()), length,
          reinterpret_cast<const uint8_t *>(socket_path.data()),
          socket_path.size()),
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
