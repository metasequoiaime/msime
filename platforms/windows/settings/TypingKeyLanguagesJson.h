#pragma once

#include "TypingKeyLanguagesModel.h"
#include <nlohmann/json.hpp>

namespace msime::settings::typing {

// Win7 的 JSON 适配；畸形字段不能伪装成没有语言历史。
inline std::optional<DailyKeyLanguages>
key_languages_from_json(nlohmann::json const &value) {
  DailyKeyLanguages result;
  if (value.is_null())
    return result;
  if (!value.is_object())
    return std::nullopt;
  for (auto day = value.begin(); day != value.end(); ++day) {
    if (!day->is_object())
      return std::nullopt;
    for (auto language = day->begin(); language != day->end(); ++language) {
      if (!language->is_object())
        return std::nullopt;
      for (auto key = language->begin(); key != language->end(); ++key) {
        if (!key->is_number())
          return std::nullopt;
        const double count = key->get<double>();
        if (!std::isfinite(count))
          return std::nullopt;
        result[day.key()][language.key()][key.key()] = count;
      }
    }
  }
  return result;
}

} // namespace msime::settings::typing
