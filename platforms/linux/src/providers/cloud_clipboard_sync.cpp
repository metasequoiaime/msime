#include "../clipboard/CloudClipboardRoute.h"
#include "../core/RuntimeOptionsFile.h"
#include "../core/BoundedCliInput.h"
#include "provider_socket_cli.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <array>
#include <cstdlib>
#include <filesystem>
#include <iostream>
#include <nlohmann/json.hpp>
#include <string>

using Json = nlohmann::json;

namespace {
Json response(char *raw) {
  auto owned = msime::host_api::own_string(raw);
  if (!owned) throw std::runtime_error("cloud clipboard request failed");
  auto value = Json::parse(owned.get());
  if (!value.value("ok", false)) throw std::runtime_error("cloud clipboard request failed");
  return value.at("value");
}

std::string socket_for(const Json &options) {
  auto value = options.value("cloud_clipboard_provider_socket", std::string{});
  if (value.empty()) {
    if (const auto *environment = std::getenv("MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET"))
      value = environment;
  }
  if (!value.empty()) return value;
  if (const auto *runtime = std::getenv("XDG_RUNTIME_DIR")) {
    const auto candidate = std::filesystem::path(runtime) / MSIME_EDITION_CLIENT_DIRECTORY / "cloud-clipboard.sock";
    std::error_code error;
    if (std::filesystem::is_socket(candidate, error)) return candidate.string();
  }
  return {};
}

Json request(const Json &options, const Json &action) {
  const auto directory = options.value("preferences_directory", std::string{});
  if (directory.empty() || directory.front() != '/') return Json::object();
  const auto socket = socket_for(options);
  return msime::linux_host::cloud_clipboard_request(
      directory, options.value("cloud_clipboard_provider_socket", std::string{}), socket,
      action,
      [](const std::string &path) {
        return response(msime_client_cloud_clipboard_source(
            reinterpret_cast<const uint8_t *>(path.data()), path.size()));
      },
      [](const Json &body) {
        const auto encoded = body.dump();
        return response(msime_client_relay_cloud_clipboard(
            reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size()));
      },
      [](const std::string &path, const Json &action) {
        const auto encoded = action.dump();
        return response(msime_client_cloud_clipboard_provider_request(
            reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(),
            reinterpret_cast<const uint8_t *>(path.data()), path.size()));
      });
}
}

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-cloud-clipboard-sync /absolute/runtime-options.json\n";
    return 0;
  }
  if (argc != 2 || argv[1][0] != '/') return 2;
  std::array<char, 65537> input;
  const auto length = msime::linux_host::read_bounded_cli_input(std::cin, input);
  if (!length) return 2;
  try {
    const auto options = Json::parse(msime::linux_host::read_runtime_options(argv[1]));
    const auto command = Json::parse(std::string(input.data(), *length));
    if (!command.is_object()) return 2;
    auto result = request(options, command);
    std::cout << result.dump() << '\n';
    return std::cout ? 0 : 1;
  } catch (...) {
    return 1;
  }
}
