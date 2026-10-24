#pragma once

#include "ThemeData.h"
#include "../TypingStatisticsModel.h"

namespace msime::settings::win7::statistics_data {

namespace model = msime::settings::typing;
struct Status {
  model::Statistics statistics;
  bool never_written = true;
  std::optional<uint64_t> last_written_ms;
};
inline model::Counts counts(const Json& object) {
  model::Counts result;
  if (object.is_object()) for (auto entry = object.begin(); entry != object.end(); ++entry)
    if (entry.value().is_number()) result[entry.key()] = entry.value().get<double>();
  return result;
}
inline model::Breakdown breakdown(const Json& value) {
  if (!value.is_object()) return {};
  return {counts(value.value("characters", Json())), counts(value.value("sources", Json()))};
}
// 仅适配 C ABI 的 JSON；日期、范围、计数和图表派生规则复用现有 Windows 页面模型。
inline std::optional<Status> parse(const Json& response) {
  const auto value = theme_data::value(response);
  if (!value || !value->is_object() || !value->contains("statistics") || !value->at("statistics").is_object()) return std::nullopt;
  const auto& object = value->at("statistics");
  if (!object.contains("enabled") || !object.at("enabled").is_boolean()) return std::nullopt;
  Status status;
  auto& statistics = status.statistics;
  statistics.enabled = object.at("enabled").get<bool>();
  if (object.contains("retention") && object.at("retention").is_string()) statistics.retention = object.at("retention").get<std::string>();
  if (object.contains("total") && object.at("total").is_number()) statistics.total = object.at("total").get<double>();
  statistics.days = counts(object.value("days", Json()));
  statistics.detail = breakdown(object.value("detail", Json()));
  statistics.daily_active_ms = counts(object.value("dailyActiveMs", Json()));
  if (object.contains("dailyDetails") && object.at("dailyDetails").is_object())
    for (auto entry = object.at("dailyDetails").begin(); entry != object.at("dailyDetails").end(); ++entry)
      statistics.daily_details[entry.key()] = breakdown(entry.value());
  if (object.contains("dailyKeys") && object.at("dailyKeys").is_object())
    for (auto entry = object.at("dailyKeys").begin(); entry != object.at("dailyKeys").end(); ++entry)
      statistics.daily_keys[entry.key()] = counts(entry.value());
  if (object.contains("dailyHours") && object.at("dailyHours").is_object())
    for (auto entry = object.at("dailyHours").begin(); entry != object.at("dailyHours").end(); ++entry) {
      if (!entry.value().is_array()) continue;
      for (const auto& hour : entry.value()) statistics.daily_hours[entry.key()].push_back(hour.is_number() ? hour.get<double>() : 0);
    }
  if (object.contains("selections") && object.at("selections").is_object()) {
    const auto& selections = object.at("selections");
    if (selections.contains("ranks") && selections.at("ranks").is_array())
      for (const auto& rank : selections.at("ranks")) statistics.ranks.push_back(rank.is_number() ? rank.get<double>() : 0);
    if (selections.contains("beyond") && selections.at("beyond").is_number()) statistics.beyond = selections.at("beyond").get<double>();
  }
  const auto availability = value->find("availability");
  status.never_written = availability == value->end() || !availability->is_string() || *availability != "ready";
  if (value->contains("lastWrittenMs") && value->at("lastWrittenMs").is_number_unsigned()) status.last_written_ms = value->at("lastWrittenMs").get<uint64_t>();
  return status;
}
inline model::CivilDay local_day() {
  SYSTEMTIME now{}; GetLocalTime(&now); return {now.wYear, now.wMonth, now.wDay};
}
inline std::wstring written_text(uint64_t milliseconds) {
  if (milliseconds > 900000000000000ULL) return {};
  ULARGE_INTEGER ticks{}; ticks.QuadPart = milliseconds * 10000ULL + 116444736000000000ULL;
  const FILETIME utc{ticks.LowPart, ticks.HighPart};
  SYSTEMTIME universal{}, local{};
  if (!FileTimeToSystemTime(&utc, &universal) || !SystemTimeToTzSpecificLocalTime(nullptr, &universal, &local)) return {};
  return model::zh_date_time(local.wYear, local.wMonth, local.wDay, local.wHour, local.wMinute, local.wSecond);
}
} // namespace msime::settings::win7::statistics_data
