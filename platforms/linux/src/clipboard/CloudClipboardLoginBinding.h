#pragma once

#include <nlohmann/json.hpp>
#include <stdexcept>
#include <string>

namespace msime::linux_host {

// provider 持有自己的登录，不能拿本机水杉账号推断；不支持握手时暂停自动同步。
template <typename Provider, typename Account>
nlohmann::json cloud_clipboard_login_binding(const std::string &socket,
                                              Provider provider, Account account) {
  auto login = socket.empty() ? account() : provider(socket);
  if (!login.is_object() || login.size() != 2 ||
      !login.contains("user_id") || !login.at("user_id").is_string() ||
      !login.contains("session_id") || !login.at("session_id").is_string())
    throw std::runtime_error("account unavailable");
  const auto user = login.at("user_id").template get<std::string>();
  const auto session = login.at("session_id").template get<std::string>();
  if (user.empty() || user.size() > 256 || session.empty() || session.size() > 128)
    throw std::runtime_error("account unavailable");
  return login;
}

}
