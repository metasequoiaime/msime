#include "msime_client.h"
#include "../../../common/HostApiString.h"
#include "../core/BoundedCliInput.h"
#include "provider_response_cli.h"
#include "../core/LocalResourcePaths.h"
#include "provider_socket_cli.h"

#include <array>
#include <cstdlib>
#include <filesystem>
#include <iostream>
#include <string>

// 我们自己发布的 deb/rpm 不带手写模型，由设置应用下载到默认数据目录 $XDG_CONFIG_HOME/<客户端目录>/resource-packs/handwriting/。安装前缀和环境变量里都找不到时，--local 再到这里找；用户把数据目录移到别处后，要用参数或 MSIME_HANDWRITING_MODEL 指定模型。
static std::string downloaded_handwriting_model() {
  std::filesystem::path root;
  if (const char *config = std::getenv("XDG_CONFIG_HOME"); config && *config && std::filesystem::path(config).is_absolute())
    root = config;
  else if (const char *home = std::getenv("HOME"); home && *home && std::filesystem::path(home).is_absolute())
    root = std::filesystem::path(home) / ".config";
  else
    return {};
  const auto model = root / MSIME_EDITION_CLIENT_DIRECTORY / "resource-packs/handwriting/handwriting-zh_CN.model";
  return msime_linux::resource_file(model) ? model.string() : std::string{};
}

int main(int argc, char **argv) {
  if (argc == 2 && std::string(argv[1]) == "--help") {
    std::cout << "Usage: msime-linux-handwriting [provider-socket] | --local [model]\n";
    return 0;
  }
  const bool local = argc >= 2 && std::string(argv[1]) == "--local";
  if (local && argc > 3)
    return 2;
  const std::string endpoint = local
      ? (argc == 3 ? argv[2] : [] {
          auto model = msime_linux::local_resource("MSIME_HANDWRITING_MODEL", MSIME_EDITION_CLIENT_DIRECTORY "/handwriting/handwriting-zh_CN.model");
          // 显式设置了 MSIME_HANDWRITING_MODEL 时不换用别的模型。
          if (model.empty() && !(std::getenv("MSIME_HANDWRITING_MODEL") && *std::getenv("MSIME_HANDWRITING_MODEL")))
            model = downloaded_handwriting_model();
          return model;
        }())
      : msime_cli_provider_socket(argc, argv, "MSIME_HANDWRITING_PROVIDER_SOCKET", "handwriting.sock");
  if (endpoint.empty() || endpoint[0] != '/')
    return 2;
  std::array<char, 262145> buffer;
  const auto request_length = msime::linux_host::read_bounded_cli_input(std::cin, buffer);
  if (!request_length)
    return 2;
  const size_t length = *request_length;
  auto result = msime::host_api::own_string(
      local ? msime_client_handwriting_local_request(
                  reinterpret_cast<const uint8_t *>(buffer.data()), length,
                  reinterpret_cast<const uint8_t *>(endpoint.data()), endpoint.size())
            : msime_client_handwriting_provider_request(
                  reinterpret_cast<const uint8_t *>(buffer.data()), length,
                  reinterpret_cast<const uint8_t *>(endpoint.data()), endpoint.size()));
  return msime_cli_write_provider_response(result.get(), std::cout);
}
