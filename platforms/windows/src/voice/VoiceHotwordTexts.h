#pragma once

#include <nlohmann/json.hpp>

#include <string>
#include <vector>

namespace msime::windows {

inline std::vector<std::string> hotword_texts(const nlohmann::json &hotwords) {
  std::vector<std::string> texts;
  texts.reserve(hotwords.size());
  for (const auto &hotword : hotwords) {
    const auto text = hotword.find("text");
    if (hotword.is_object() && text != hotword.end() && text->is_string())
      texts.push_back(text->get<std::string>());
  }
  return texts;
}

} // namespace msime::windows
