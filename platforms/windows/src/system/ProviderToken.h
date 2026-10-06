#pragma once

#include <nlohmann/json.hpp>

#include <cctype>
#include <string>
#include <string_view>

namespace msime::windows {

inline std::string usable_provider_token(std::string_view token) {
  if (token.empty() ||
      (token.size() >= 2 && token.front() == '<' && token.back() == '>') ||
      token.rfind("FAKESECRET_", 0) == 0)
    return {};
  return std::string(token);
}

// Provider IDs match case-insensitively, like the Windows reference provider resolver, while the stored token itself stays untouched.
inline std::string provider_token(const nlohmann::json &input,
                                  const char *slots_key, const char *flat_key,
                                  const std::string &provider) {
  const auto slots = input.value(slots_key, nlohmann::json::object());
  if (slots.is_object() && !provider.empty()) {
    std::string wanted = provider;
    for (char &ch : wanted)
      ch = static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));
    for (auto it = slots.begin(); it != slots.end(); ++it) {
      if (!it.value().is_string() || it.key().size() != wanted.size())
        continue;
      bool matches = true;
      for (size_t i = 0; i < wanted.size(); ++i) {
        const auto ch = static_cast<char>(
            std::tolower(static_cast<unsigned char>(it.key()[i])));
        if (ch != wanted[i]) {
          matches = false;
          break;
        }
      }
      if (matches)
        return usable_provider_token(it.value().get<std::string>());
    }
  }
  return usable_provider_token(input.value(flat_key, std::string{}));
}

} // namespace msime::windows
