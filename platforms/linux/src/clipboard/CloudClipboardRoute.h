#pragma once

#include <nlohmann/json.hpp>
#include <stdexcept>
#include <string>

namespace msime::linux_host {

// 选路与 Linux 设置面板一致：显式 socket、中继、自动发现的 socket。
// 回调均在后台执行；中继或偏好读取失败直接返回错误，不向其他服务重试。
template <typename Source, typename Relay, typename Provider>
nlohmann::json cloud_clipboard_list(const std::string &directory,
                                    const std::string &explicit_socket,
                                    const std::string &discovered_socket,
                                    Source source, Relay relay, Provider provider) {
  const nlohmann::json action{{"operation", "list"}, {"search", ""}};
  if (!explicit_socket.empty()) return provider(explicit_socket, action);
  if (!directory.empty()) {
    const auto selected = source(directory).at("source").template get<std::string>();
    if (selected == "relay")
      return relay(nlohmann::json{{"preferences_directory", directory},
                                  {"directory", directory}, {"action", action}});
    if (selected != "account") throw std::runtime_error("invalid cloud clipboard source");
  }
  if (!discovered_socket.empty()) return provider(discovered_socket, action);
  return nlohmann::json::object();
}

}
