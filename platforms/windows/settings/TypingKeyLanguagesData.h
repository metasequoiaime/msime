#pragma once

#include "TypingKeyLanguagesModel.h"
#include <winrt/Windows.Data.Json.h>

namespace msime::settings::typing {

// 现代设置继续使用既有 WinRT JSON，不额外解析整份统计答复。
inline std::optional<DailyKeyLanguages>
key_languages_from_winrt(winrt::Windows::Data::Json::IJsonValue const &value) {
  using namespace winrt::Windows::Data::Json;
  DailyKeyLanguages result;
  if (!value || value.ValueType() == JsonValueType::Null)
    return result;
  if (value.ValueType() != JsonValueType::Object)
    return std::nullopt;
  for (auto const &day : value.GetObject()) {
    if (day.Value().ValueType() != JsonValueType::Object)
      return std::nullopt;
    for (auto const &language : day.Value().GetObject()) {
      if (language.Value().ValueType() != JsonValueType::Object)
        return std::nullopt;
      for (auto const &key : language.Value().GetObject()) {
        if (key.Value().ValueType() != JsonValueType::Number)
          return std::nullopt;
        const double count = key.Value().GetNumber();
        if (!std::isfinite(count))
          return std::nullopt;
        result[winrt::to_string(day.Key())][winrt::to_string(language.Key())]
              [winrt::to_string(key.Key())] = count;
      }
    }
  }
  return result;
}

} // namespace msime::settings::typing
