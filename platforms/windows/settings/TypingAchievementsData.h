#pragma once

#include "../../common/HostApiString.h"
#include "TypingAchievementsModel.h"
#include <utility>
#include <winrt/Windows.Data.Json.h>

namespace msime::settings::typing {

struct AchievementsSnapshot {
  std::vector<Achievement> badges;
  std::string signature;
};

// 在统计工作线程读取共享摘要；成功的空数组与读取失败分别保留。
inline std::optional<AchievementsSnapshot>
load_achievements(std::wstring const &directory, std::wstring const &day,
                  std::optional<std::string> const &options) {
  using namespace winrt::Windows::Data::Json;
  try {
    JsonObject action;
    action.SetNamedValue(L"operation",
                         JsonValue::CreateStringValue(L"summary"));
    action.SetNamedValue(L"day",
                         JsonValue::CreateStringValue(winrt::hstring(day)));
    if (options) {
      JsonObject count_action;
      count_action.SetNamedValue(L"operation",
                                 JsonValue::CreateStringValue(L"count"));
      count_action.SetNamedValue(L"kind",
                                 JsonValue::CreateStringValue(L"pinyin"));
      count_action.SetNamedValue(L"user_only",
                                 JsonValue::CreateBooleanValue(true));
      JsonObject count_request;
      count_request.SetNamedValue(
          L"options", JsonObject::Parse(winrt::to_hstring(*options)));
      count_request.SetNamedValue(L"action", count_action);
      const auto count_bytes = winrt::to_string(count_request.Stringify());
      auto count_raw = msime::host_api::own_string(msime_client_dictionary(
          reinterpret_cast<const uint8_t *>(count_bytes.data()),
          count_bytes.size()));
      if (count_raw) {
        JsonObject count_response{nullptr};
        if (JsonObject::TryParse(winrt::to_hstring(count_raw.get()),
                                 count_response) &&
            count_response.GetNamedBoolean(L"ok", false)) {
          const auto count =
              count_response.GetNamedObject(L"value").GetNamedValue(L"count");
          if (count.ValueType() == JsonValueType::Number)
            action.SetNamedValue(L"user_words", count);
        }
      }
    }
    JsonObject request;
    request.SetNamedValue(
        L"directory", JsonValue::CreateStringValue(winrt::hstring(directory)));
    request.SetNamedValue(L"action", action);
    const auto bytes = winrt::to_string(request.Stringify());
    auto raw = msime::host_api::own_string(msime_client_typing_statistics(
        reinterpret_cast<const uint8_t *>(bytes.data()), bytes.size()));
    if (!raw)
      return std::nullopt;
    const auto root = JsonObject::Parse(winrt::to_hstring(raw.get()));
    if (!root.GetNamedBoolean(L"ok", false))
      return std::nullopt;
    const auto array =
        root.GetNamedObject(L"value").GetNamedArray(L"achievements");
    AchievementsSnapshot result;
    result.signature = winrt::to_string(array.Stringify());
    for (auto const &entry : array) {
      const auto object = entry.GetObject();
      Achievement badge;
      badge.id = object.GetNamedString(L"id").c_str();
      badge.glyph = object.GetNamedString(L"glyph").c_str();
      badge.title = object.GetNamedString(L"title").c_str();
      badge.description = object.GetNamedString(L"description").c_str();
      badge.current = object.GetNamedNumber(L"current");
      badge.target = object.GetNamedNumber(L"target");
      if (!std::isfinite(badge.current) || badge.current < 0 ||
          !std::isfinite(badge.target) || badge.target <= 0)
        return std::nullopt;
      const auto unlocked = object.GetNamedValue(L"unlocked_day");
      if (unlocked.ValueType() == JsonValueType::String)
        badge.unlocked_day = unlocked.GetString().c_str();
      else if (unlocked.ValueType() != JsonValueType::Null)
        return std::nullopt;
      result.badges.push_back(std::move(badge));
    }
    return result;
  } catch (...) {
    return std::nullopt;
  }
}

} // namespace msime::settings::typing
