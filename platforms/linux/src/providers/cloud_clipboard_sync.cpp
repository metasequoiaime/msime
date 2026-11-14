#include "../clipboard/CloudClipboardRoute.h"
#include "../clipboard/CloudClipboardLoginBinding.h"
#include "../core/RuntimeOptionsFile.h"
#include "../core/AuxiliaryCrashCapture.h"
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

// 自动同步携带来源快照；任何变化都要求监视器重新建立本机基线。
Json binding(const Json &options) {
  const auto directory = options.value("preferences_directory", std::string{});
  if (directory.empty() || directory.front() != '/') throw std::runtime_error("invalid directory");
  auto value = response(msime_client_cloud_clipboard_source(
      reinterpret_cast<const uint8_t *>(directory.data()), directory.size()));
  auto explicit_socket = options.value("cloud_clipboard_provider_socket", std::string{});
  if (explicit_socket.empty()) {
    if (const auto *environment = std::getenv("MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET"))
      explicit_socket = environment;
  }
  const auto socket = socket_for(options);
  if (!explicit_socket.empty() || value.at("source") == "account") {
    const auto login = msime::linux_host::cloud_clipboard_login_binding(socket,
        [](const std::string &path) {
          const auto encoded = Json{{"operation", "binding"}}.dump();
          return response(msime_client_cloud_clipboard_provider_request(
              reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(),
              reinterpret_cast<const uint8_t *>(path.data()), path.size()));
        },
        [&directory] {
          const auto body = Json{{"directory", directory}, {"action", {{"operation", "binding"}}}}.dump();
          return response(msime_client_account_cloud_clipboard(
              reinterpret_cast<const uint8_t *>(body.data()), body.size()));
        });
    value["login"] = login;
    value["user_id"] = login.at("user_id");
    value["socket"] = socket;
  }
  value["directory"] = directory;
  return value;
}

Json bound_request(const Json &options, Json action, const Json &expected) {
  if (binding(options) != expected) throw std::runtime_error("source changed");
  const auto directory = expected.at("directory");
  Json result;
  if (expected.contains("socket") && !expected.at("socket").get<std::string>().empty()) {
    action["expected_user_id"] = expected.at("user_id");
    action["expected_login"] = expected.at("login");
    const auto encoded = action.dump();
    const auto socket = expected.at("socket").get<std::string>();
    result = response(msime_client_cloud_clipboard_provider_request(
        reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size(),
        reinterpret_cast<const uint8_t *>(socket.data()), socket.size()));
  } else {
    Json body{{"directory", directory}, {"preferences_directory", directory}, {"action", action}};
    char *raw;
    if (expected.at("source") == "relay") {
      body["expected_revision"] = expected.at("revision");
      const auto encoded = body.dump();
      raw = msime_client_relay_cloud_clipboard(
          reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size());
    } else {
      body["action"]["expected_user_id"] = expected.at("user_id");
      body["action"]["expected_login"] = expected.at("login");
      const auto encoded = body.dump();
      raw = msime_client_account_cloud_clipboard(
          reinterpret_cast<const uint8_t *>(encoded.data()), encoded.size());
    }
    result = response(raw);
  }
  if (binding(options) != expected) throw std::runtime_error("source changed");
  return result;
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
  msime::telemetry::CaptureOnly crash_capture([options = std::filesystem::path(argv[1])] {
    return msime::linux_host::auxiliary_crash_host("cloud-clipboard-sync", {}, options);
  });
  std::array<char, 65537> input;
  const auto length = msime::linux_host::read_bounded_cli_input(std::cin, input);
  if (!length) return 2;
  try {
    const auto options = Json::parse(msime::linux_host::read_runtime_options(argv[1]));
    const auto command = Json::parse(std::string(input.data(), *length));
    if (!command.is_object()) return 2;
    Json result;
    if (command.value("operation", std::string{}) == "binding") {
      result = binding(options);
    } else if (command.contains("expected_binding")) {
      auto action = command;
      action.erase("expected_binding");
      result = bound_request(options, action, command.at("expected_binding"));
    } else {
      result = request(options, command);
    }
    std::cout << result.dump() << '\n';
    return std::cout ? 0 : 1;
  } catch (...) {
    return 1;
  }
}
