#pragma once
#include <algorithm>
#include <array>
#include <charconv>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <vector>

// 「打字统计」页的派生数字和文案，照共享设置在 Windows 上画的那一版（`packages/ui/src/settings/typing-statistics.tsx` 的桌面分支，`mobile` 为假、`platform` 为 windows、没有 `look`）逐条移植：`typing-statistics-model.ts`、`typing-statistics-helpers.ts`、`typing-statistics-overview.tsx`、`typing-speed.ts`、`typing-breakdown.ts`、`typing-chart.ts` 的环形图几何和 `keyboard-heatmap.ts` 加 `keyboard/keyboard-layouts.ts` 的按键布局。数字格式照 `formatZhNumber`（`toLocaleString("zh-CN")`）、`formatZhPercent`（`toFixed(1)`）和 `Math.round` 的规则。只给手机外观用的摘要（`typing-summary.ts`）和手机键盘布局不在这里。不依赖 Windows 和 WinRT 头文件，「今天」由调用方传入，方便单独测试。
namespace msime::settings::typing {

// ---- 数字与文字 ----

using Counts = std::map<std::string, double>;

// JavaScript 的 Math.round：四舍五入，正好一半时往正无穷取。
inline double js_round(double value) {
  if (!std::isfinite(value))
    return value;
  const double floor = std::floor(value);
  return value - floor >= 0.5 ? floor + 1 : floor;
}

inline std::wstring widen(std::string_view ascii) { return std::wstring(ascii.begin(), ascii.end()); }

// Number.prototype.toFixed：按这个 double 的精确十进制值取 `digits` 位小数，正好一半时取较大的那个数。
inline std::string to_fixed(double value, int digits) {
  if (std::isnan(value))
    return "NaN";
  const bool negative = value < 0;
  const double magnitude = std::fabs(value);
  // 双精度数的精确十进制展开最多一千多位小数，留足位数后再手工进位。
  static thread_local char buffer[1500];
  std::snprintf(buffer, sizeof buffer, "%.1100f", magnitude);
  const std::string exact(buffer);
  const auto dot = exact.find('.');
  std::string integer = exact.substr(0, dot);
  std::string fraction = dot == std::string::npos ? std::string() : exact.substr(dot + 1);
  fraction.resize(static_cast<size_t>(digits) + 1, '0');
  const bool up = fraction[static_cast<size_t>(digits)] >= '5';
  std::string kept = integer + fraction.substr(0, static_cast<size_t>(digits));
  if (up) {
    size_t i = kept.size();
    bool carry = true;
    while (carry && i > 0) {
      --i;
      if (kept[i] == '9') {
        kept[i] = '0';
      } else {
        ++kept[i];
        carry = false;
      }
    }
    if (carry)
      kept.insert(kept.begin(), '1');
  }
  const size_t integer_length = kept.size() - static_cast<size_t>(digits);
  std::string result = kept.substr(0, integer_length);
  if (digits > 0)
    result += "." + kept.substr(integer_length);
  // 与 toFixed 一样，负数即使舍成零也带负号。
  return negative ? "-" + result : result;
}

// 模板字符串里的 `${value}`：整数照写数字，其他按最短能还原的写法。
inline std::wstring js_number(double value) {
  if (std::isnan(value))
    return L"NaN";
  if (value == std::floor(value) && std::fabs(value) < 1e21)
    return widen(to_fixed(value, 0));
  char buffer[64];
  const auto result = std::to_chars(buffer, buffer + sizeof buffer, value);
  return widen(std::string_view(buffer, static_cast<size_t>(result.ptr - buffer)));
}

// `formatZhNumber`（`toLocaleString("zh-CN")`）：最多三位小数，整数部分每三位一个逗号。
inline std::wstring zh_number(double value) {
  if (std::isnan(value))
    return L"NaN";
  std::string fixed = to_fixed(value, 3);
  bool negative = false;
  if (!fixed.empty() && fixed[0] == '-') {
    negative = true;
    fixed.erase(0, 1);
  }
  const auto dot = fixed.find('.');
  std::string integer = fixed.substr(0, dot);
  std::string fraction = dot == std::string::npos ? std::string() : fixed.substr(dot + 1);
  while (!fraction.empty() && fraction.back() == '0')
    fraction.pop_back();
  std::string grouped;
  for (size_t i = 0; i < integer.size(); ++i) {
    if (i > 0 && (integer.size() - i) % 3 == 0)
      grouped.push_back(',');
    grouped.push_back(integer[i]);
  }
  if (!fraction.empty())
    grouped += "." + fraction;
  return widen((negative ? "-" : "") + grouped);
}

// `formatZhPercent`：total 为零时是「—」，否则保留一位小数的百分比。
inline std::wstring zh_percent(double value, double total) {
  return total == 0 ? std::wstring(L"—") : widen(to_fixed((value / total) * 100, 1)) + L"%";
}

// `formatActiveTime`：「0分」「12秒」「45分」「2小时」「1小时5分」。
inline std::wstring active_time(double milliseconds) {
  if (milliseconds <= 0)
    return L"0分";
  const double total_minutes = std::floor(milliseconds / 60000);
  if (total_minutes == 0)
    return js_number(std::max(1.0, js_round(milliseconds / 1000))) + L"秒";
  const double hours = std::floor(total_minutes / 60);
  const double minutes = std::fmod(total_minutes, 60);
  if (hours == 0)
    return js_number(minutes) + L"分";
  return minutes == 0 ? js_number(hours) + L"小时"
                      : js_number(hours) + L"小时" + js_number(minutes) + L"分";
}

// `new Date(ms).toLocaleString("zh-CN")` 的样子，由调用方给出本地时间的各个部分：`2026/10/11 09:03:05`。
inline std::wstring zh_date_time(int year, int month, int day, int hour, int minute, int second) {
  wchar_t buffer[64];
  std::swprintf(buffer, 64, L"%d/%d/%d %02d:%02d:%02d", year, month, day, hour, minute, second);
  return buffer;
}

// ---- 日期 ----

// 公历日期。星期和加减天数按「自 1970-01-01 起的天数」换算，与本地时区无关，和共享页用本地日期对象算出的日子相同。
struct CivilDay {
  int year = 1970;
  int month = 1;
  int day = 1;
};

inline int64_t days_from_civil(CivilDay date) {
  const int64_t y = static_cast<int64_t>(date.year) - (date.month <= 2 ? 1 : 0);
  const int64_t era = (y >= 0 ? y : y - 399) / 400;
  const int64_t yoe = y - era * 400;
  const int64_t mp = (date.month + 9) % 12;
  const int64_t doy = (153 * mp + 2) / 5 + date.day - 1;
  const int64_t doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
  return era * 146097 + doe - 719468;
}

inline CivilDay civil_from_days(int64_t days) {
  days += 719468;
  const int64_t era = (days >= 0 ? days : days - 146096) / 146097;
  const int64_t doe = days - era * 146097;
  const int64_t yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  const int64_t doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  const int64_t mp = (5 * doy + 2) / 153;
  const int day = static_cast<int>(doy - (153 * mp + 2) / 5 + 1);
  const int month = static_cast<int>(mp < 10 ? mp + 3 : mp - 9);
  return {static_cast<int>(yoe + era * 400 + (month <= 2 ? 1 : 0)), month, day};
}

// 0 是星期日，与 Date.prototype.getDay 相同。
inline int weekday(CivilDay date) {
  const int64_t days = days_from_civil(date);
  return static_cast<int>(((days % 7) + 11) % 7);
}

inline CivilDay shift(CivilDay date, int64_t days) {
  return civil_from_days(days_from_civil(date) + days);
}

// `dayKey`：`YYYY-MM-DD`。
inline std::string day_key(CivilDay date) {
  char buffer[32];
  std::snprintf(buffer, sizeof buffer, "%04d-%02d-%02d", date.year, date.month, date.day);
  return buffer;
}

// 统计页的快照内容没有变化时，日期推进也必须触发重画。
inline bool statistics_date_changed(std::optional<std::string> const &previous,
                                    std::string_view current) {
  return !previous || *previous != current;
}

// `formatZhMonthDay`：`9月21日`。
inline std::wstring month_day(CivilDay date) {
  return std::to_wstring(date.month) + L"月" + std::to_wstring(date.day) + L"日";
}

// `/^\d{4}-\d{2}-\d{2}$/`。
inline bool well_formed_day(std::string_view key) {
  if (key.size() != 10 || key[4] != '-' || key[7] != '-')
    return false;
  for (size_t i = 0; i < key.size(); ++i)
    if (i != 4 && i != 7 && (key[i] < '0' || key[i] > '9'))
      return false;
  return true;
}

// 格式对、也是真实存在的日期时给出它。
inline std::optional<CivilDay> parse_day(std::string_view key) {
  if (!well_formed_day(key))
    return std::nullopt;
  auto number = [key](size_t from, size_t length) {
    int value = 0;
    for (size_t i = from; i < from + length; ++i)
      value = value * 10 + (key[i] - '0');
    return value;
  };
  const CivilDay date{number(0, 4), number(5, 2), number(8, 2)};
  if (date.month < 1 || date.month > 12 || date.day < 1)
    return std::nullopt;
  const auto round_trip = civil_from_days(days_from_civil(date));
  if (round_trip.month != date.month || round_trip.day != date.day)
    return std::nullopt;
  return date;
}

struct DayEntry {
  std::string key;
  std::wstring label;
};

// `recentDays`：以今天结尾的 `length` 天，从早到晚。
inline std::vector<DayEntry> recent_days(size_t length, CivilDay today) {
  std::vector<DayEntry> days;
  days.reserve(length);
  for (size_t index = 0; index < length; ++index) {
    const auto date = shift(today, -static_cast<int64_t>(length - index - 1));
    days.push_back({day_key(date), month_day(date)});
  }
  return days;
}

// `statisticDayKeys`：格式正确的记录日，从早到晚。
inline std::vector<std::string> statistic_day_keys(Counts const &days) {
  std::vector<std::string> keys;
  for (auto const &[key, value] : days)
    if (well_formed_day(key))
      keys.push_back(key);
  return keys;
}

inline double value_of(Counts const &values, std::string const &key) {
  const auto found = values.find(key);
  return found == values.end() ? 0 : found->second;
}

// `sumStatisticValues`。
inline double sum_values(Counts const &values, std::vector<std::string> const &keys) {
  double total = 0;
  for (auto const &key : keys)
    total += value_of(values, key);
  return total;
}

// `dayLabel`：`YYYY-MM-DD` 写成 `9月21日`。
inline std::wstring day_label(std::string_view key) {
  const auto first = key.find('-');
  const auto second = first == std::string_view::npos ? first : key.find('-', first + 1);
  if (second == std::string_view::npos)
    return L"NaN月NaN日";
  auto number = [](std::string_view part) -> std::wstring {
    if (part.empty())
      return L"0";
    int value = 0;
    for (const char c : part) {
      if (c < '0' || c > '9')
        return L"NaN";
      value = value * 10 + (c - '0');
    }
    return std::to_wstring(value);
  };
  const auto third = key.find('-', second + 1);
  const auto day_part = key.substr(second + 1, third == std::string_view::npos
                                                   ? std::string_view::npos
                                                   : third - second - 1);
  return number(key.substr(first + 1, second - first - 1)) + L"月" + number(day_part) + L"日";
}

struct HeatmapDay {
  std::string key;
  std::wstring label;
  double count = 0;
  bool future = false;
};

// `statisticsHeatmapWeeks`：53 列，每列一周，从星期一排到星期日，最后一列是本周。
inline std::vector<std::array<HeatmapDay, 7>> heatmap_weeks(Counts const &days, CivilDay today) {
  const int64_t current = days_from_civil(today);
  const int64_t this_monday = current - ((weekday(today) + 6) % 7);
  const int64_t start = this_monday - 52 * 7;
  std::vector<std::array<HeatmapDay, 7>> weeks(53);
  for (int64_t week = 0; week < 53; ++week)
    for (int64_t row = 0; row < 7; ++row) {
      const int64_t number = start + week * 7 + row;
      const auto date = civil_from_days(number);
      auto &cell = weeks[static_cast<size_t>(week)][static_cast<size_t>(row)];
      cell.key = day_key(date);
      cell.label = month_day(date);
      cell.count = value_of(days, cell.key);
      cell.future = number > current;
    }
  return weeks;
}

// 热力图上方的月份：只标在含该月 1 日的那一列。
inline std::vector<std::wstring> heatmap_month_labels(std::vector<std::array<HeatmapDay, 7>> const &weeks) {
  std::vector<std::wstring> labels;
  for (auto const &week : weeks) {
    std::wstring label;
    for (auto const &day : week)
      if (day.key.size() >= 3 && day.key.compare(day.key.size() - 3, 3, "-01") == 0) {
        label = day.label.substr(0, day.label.find(L'月')) + L"月";
        break;
      }
    labels.push_back(label);
  }
  return labels;
}

// `addDays`：按日历加减天数；不是日期时原样返回。
inline std::string add_days(std::string const &key, int64_t days) {
  const auto date = parse_day(key);
  return date ? day_key(shift(*date, days)) : key;
}

// `currentStreak`：截至今天（今天还没有记录时截至昨天）的连续记录天数。
inline int current_streak(std::vector<std::string> const &recorded, std::string const &today) {
  const std::set<std::string> present(recorded.begin(), recorded.end());
  std::string cursor = present.count(today) ? today : add_days(today, -1);
  int streak = 0;
  while (present.count(cursor)) {
    ++streak;
    cursor = add_days(cursor, -1);
  }
  return streak;
}

// `longestStreak`：`recorded` 从早到晚排好。
inline int longest_streak(std::vector<std::string> const &recorded) {
  if (recorded.empty())
    return 0;
  int longest = 1;
  int run = 1;
  for (size_t index = 1; index < recorded.size(); ++index) {
    if (recorded[index] == recorded[index - 1])
      continue;
    run = recorded[index] == add_days(recorded[index - 1], 1) ? run + 1 : 1;
    if (run > longest)
      longest = run;
  }
  return longest;
}

// ---- 统计 ----

struct Breakdown {
  Counts characters;
  Counts sources;
};

// 共享页的 `TypingStatistics`，只留页面读的字段。没有的项为空，与共享页把缺的项读成 undefined 一样。
struct Statistics {
  bool enabled = false;
  std::string retention = "forever";
  double total = 0;
  Counts days;
  Breakdown detail;
  std::map<std::string, Breakdown> daily_details;
  std::vector<double> ranks;
  double beyond = 0;
  Counts daily_active_ms;
  std::map<std::string, std::vector<double>> daily_hours;
  std::map<std::string, Counts> daily_keys;
};

inline Breakdown const *daily_detail(Statistics const &statistics, std::string const &key) {
  const auto found = statistics.daily_details.find(key);
  return found == statistics.daily_details.end() ? nullptr : &found->second;
}

inline double sum_all(Counts const &values) {
  double total = 0;
  for (auto const &[key, value] : values)
    total += value;
  return total;
}

// `withUnknown`：没归类的部分记在 `unknown` 下。
inline Breakdown with_unknown(Breakdown const *value, double total) {
  Breakdown result = value ? *value : Breakdown{};
  const double characters = sum_all(result.characters);
  const double sources = sum_all(result.sources);
  result.characters["unknown"] = value_of(result.characters, "unknown") + std::max(0.0, total - characters);
  result.sources["unknown"] = value_of(result.sources, "unknown") + std::max(0.0, total - sources);
  return result;
}

// `readableCharacters`：计入速度的汉字、拉丁字母和其他文字。
inline double readable_characters(Breakdown const *detail) {
  if (!detail)
    return 0;
  return value_of(detail->characters, "han") + value_of(detail->characters, "latin") +
         value_of(detail->characters, "otherLetter");
}

// `charactersPerMinute`。
inline double characters_per_minute(double characters, double active_ms) {
  if (characters <= 0 || active_ms <= 0)
    return 0;
  return characters / (active_ms / 60000);
}

// 一天至少有这么长的活跃时间，才能当「最快」的一天，也才在速度趋势上有点。
constexpr double fastest_day_minimum_active_ms = 60000;
constexpr size_t hours_per_day = 24;

struct Activity {
  size_t recorded_days = 0;
  double average_per_day = 0;
  double today_active_ms = 0;
  double total_active_ms = 0;
  double today_speed = 0;
  double average_speed = 0;
  double fastest_speed = 0;
  std::optional<std::string> fastest_day;
  int current_streak = 0;
  int longest_streak = 0;
  std::optional<std::string> best_day;
  double best_day_characters = 0;
  std::optional<std::vector<double>> today_hours;
  bool has_activity = false;
};

// `activityMetrics`。
inline Activity activity_metrics(Statistics const &statistics, std::string const &today) {
  Activity activity;
  const auto recorded = statistic_day_keys(statistics.days);
  double total_readable = 0;
  double recorded_characters = 0;
  for (auto const &key : recorded) {
    const double active_ms = value_of(statistics.daily_active_ms, key);
    const double readable = readable_characters(daily_detail(statistics, key));
    if (active_ms > 0) {
      activity.total_active_ms += active_ms;
      total_readable += readable;
      if (active_ms >= fastest_day_minimum_active_ms) {
        const double speed = characters_per_minute(readable, active_ms);
        if (speed > activity.fastest_speed) {
          activity.fastest_speed = speed;
          activity.fastest_day = key;
        }
      }
    }
    const double characters = value_of(statistics.days, key);
    recorded_characters += characters;
    if (characters > activity.best_day_characters) {
      activity.best_day_characters = characters;
      activity.best_day = key;
    }
  }
  activity.recorded_days = recorded.size();
  activity.average_per_day =
      recorded.empty() ? 0 : recorded_characters / static_cast<double>(recorded.size());
  activity.today_active_ms = value_of(statistics.daily_active_ms, today);
  activity.today_speed = characters_per_minute(readable_characters(daily_detail(statistics, today)),
                                               activity.today_active_ms);
  activity.average_speed = characters_per_minute(total_readable, activity.total_active_ms);
  activity.current_streak = current_streak(recorded, today);
  activity.longest_streak = longest_streak(recorded);
  if (const auto hours = statistics.daily_hours.find(today);
      hours != statistics.daily_hours.end() && hours->second.size() == hours_per_day)
    activity.today_hours = hours->second;
  activity.has_activity = activity.total_active_ms > 0;
  return activity;
}

struct UsualHours {
  std::vector<double> hours;
  int days = 0;
};

// `usualHours`：今天以前每个有完整 24 小时记录、且有字的日子的平均分布。
inline std::optional<UsualHours> usual_hours(Statistics const &statistics, std::string const &today) {
  std::vector<double> sums(hours_per_day, 0);
  int days = 0;
  for (auto const &[key, hours] : statistics.daily_hours) {
    if (key >= today || hours.size() != hours_per_day)
      continue;
    if (std::none_of(hours.begin(), hours.end(), [](double count) { return count > 0; }))
      continue;
    for (size_t hour = 0; hour < hours_per_day; ++hour)
      sums[hour] += hours[hour];
    ++days;
  }
  if (days == 0)
    return std::nullopt;
  for (auto &sum : sums)
    sum /= days;
  return UsualHours{std::move(sums), days};
}

// `dailySpeeds`：活跃不足一分钟的日子没有速度。
inline std::vector<std::optional<double>> daily_speeds(Statistics const &statistics,
                                                       std::vector<std::string> const &keys) {
  std::vector<std::optional<double>> speeds;
  for (auto const &key : keys) {
    const double active_ms = value_of(statistics.daily_active_ms, key);
    if (active_ms < fastest_day_minimum_active_ms)
      speeds.push_back(std::nullopt);
    else
      speeds.push_back(characters_per_minute(readable_characters(daily_detail(statistics, key)), active_ms));
  }
  return speeds;
}

constexpr size_t detail_days = 30;

struct DailyDetailRow {
  std::string key;
  double total = 0, han = 0, latin = 0, number = 0, punctuation = 0, other = 0;
  std::optional<double> active_ms;
  std::optional<double> speed;
};

// `dailyDetailRows`：截至今天最近的 `days` 个记录日，新的在前。
inline std::vector<DailyDetailRow> daily_detail_rows(Statistics const &statistics, std::string const &today,
                                                     size_t days = detail_days) {
  auto keys = statistic_day_keys(statistics.days);
  keys.erase(std::remove_if(keys.begin(), keys.end(), [&today](std::string const &key) { return key > today; }),
             keys.end());
  if (keys.size() > days)
    keys.erase(keys.begin(), keys.end() - static_cast<std::ptrdiff_t>(days));
  std::reverse(keys.begin(), keys.end());
  std::vector<DailyDetailRow> rows;
  for (auto const &key : keys) {
    DailyDetailRow row;
    row.key = key;
    row.total = value_of(statistics.days, key);
    const auto *detail = daily_detail(statistics, key);
    const auto characters = with_unknown(detail, row.total).characters;
    row.han = value_of(characters, "han");
    row.latin = value_of(characters, "latin");
    row.number = value_of(characters, "number");
    row.punctuation = value_of(characters, "punctuation");
    row.other = std::max(0.0, sum_all(characters) - row.han - row.latin - row.number - row.punctuation);
    if (const auto active = statistics.daily_active_ms.find(key); active != statistics.daily_active_ms.end()) {
      row.active_ms = active->second;
      row.speed = characters_per_minute(readable_characters(detail), active->second);
    }
    rows.push_back(std::move(row));
  }
  return rows;
}

// `scopedBreakdown`：`keys` 为空表示整个保留期。
inline Breakdown scoped_breakdown(Statistics const &statistics,
                                  std::optional<std::vector<std::string>> const &keys) {
  if (!keys)
    return with_unknown(&statistics.detail, statistics.total);
  Breakdown result;
  for (auto const &key : *keys) {
    const auto detail = with_unknown(daily_detail(statistics, key), value_of(statistics.days, key));
    for (auto const &[id, value] : detail.characters)
      result.characters[id] += value;
    for (auto const &[id, value] : detail.sources)
      result.sources[id] += value;
  }
  return result;
}

// ---- 概览 ----

struct Metric {
  std::wstring label;
  std::wstring title;
  std::wstring value;
  std::wstring unit;
  std::wstring accessible;
  std::wstring note;
};

// `statisticsOverviewMetrics`：标签行上方的六格。
inline std::vector<Metric> overview_metrics(Statistics const &statistics, std::string const &today,
                                            std::wstring const &scope_title, double scope_total,
                                            Activity const &activity) {
  std::vector<Metric> metrics;
  metrics.push_back({L"今日输入", L"", zh_number(value_of(statistics.days, today)), L"字符", L"今日输入字符数", L""});
  metrics.push_back({scope_title, L"", zh_number(scope_total), L"字符", L"当前范围输入字符数", L""});
  metrics.push_back({L"今日活跃", L"连续打字的时间", active_time(activity.today_active_ms), L"", L"今日活跃时长", L""});
  metrics.push_back({L"今日速度", L"", zh_number(js_round(activity.today_speed)), L"字 / 分钟", L"今日输入速度", L""});
  metrics.push_back({L"平均速度", L"", zh_number(js_round(activity.average_speed)), L"字 / 分钟", L"平均输入速度",
                     activity.has_activity ? L"共 " + active_time(activity.total_active_ms) : L""});
  metrics.push_back({L"连续天数", L"", zh_number(activity.current_streak), L"天", L"连续输入天数",
                     L"最长 " + zh_number(activity.longest_streak) + L" 天"});
  return metrics;
}

struct OverviewDetails {
  std::wstring summary;
  std::wstring best;
};

// `statisticsOverviewDetails`：六格下面的两行。
inline OverviewDetails overview_details(Activity const &activity) {
  OverviewDetails details;
  details.summary = L"日均 " + zh_number(js_round(activity.average_per_day)) + L" 字符 · " +
                    zh_number(static_cast<double>(activity.recorded_days)) + L" 天有记录";
  details.best = activity.best_day ? L"最多 " + day_label(*activity.best_day) + L"，" +
                                         zh_number(activity.best_day_characters) + L" 字符"
                                   : std::wstring(L"还没有记录");
  if (activity.fastest_day)
    details.best += L" · 最快 " + day_label(*activity.fastest_day) + L"，" +
                    zh_number(js_round(activity.fastest_speed)) + L" 字 / 分钟";
  return details;
}

// `availabilityNotice` 在 Windows（不是 iOS）上的文字；`written` 是最后写入时间的本地写法，没有时为空。
inline std::wstring availability_notice(bool enabled, double total, bool never_written,
                                        std::wstring const &written) {
  if (!enabled)
    return {};
  if (never_written)
    return L"键盘从未写入过统计。请用水杉键盘成功输入几个字符，再返回此页刷新。";
  if (total != 0)
    return {};
  if (!written.empty())
    return L"统计最后写入于 " + written + L"，当前计数为零；如果刚刚清空过统计，这是正常的。";
  return L"统计文件已建立，但当前还没有输入记录。";
}

// ---- 趋势 ----

constexpr size_t desktop_trend_days = 30;

// 趋势柱的参照：窗口里各天的最大值（至少 1）和有记录的日子的平均。
struct TrendScale {
  double maximum = 1;
  double average = 0;
  bool empty = true;
};

inline TrendScale trend_scale(Statistics const &statistics, std::vector<DayEntry> const &days) {
  TrendScale scale;
  double sum = 0;
  int recorded = 0;
  for (auto const &day : days) {
    const double count = value_of(statistics.days, day.key);
    scale.maximum = std::max(scale.maximum, count);
    if (count > 0) {
      sum += count;
      ++recorded;
    }
    if (count != 0)
      scale.empty = false;
  }
  scale.average = recorded == 0 ? 0 : sum / recorded;
  return scale;
}

// 「每日趋势」标题下的一行。
inline std::wstring trend_caption(TrendScale const &scale) {
  std::wstring caption = L"最高 " + (scale.maximum == 1 && scale.empty ? std::wstring(L"0") : zh_number(scale.maximum)) +
                         L" 字符 / 天";
  if (scale.average > 0)
    caption += L" · 虚线为日均 " + zh_number(js_round(scale.average)) + L" 字符";
  return caption;
}

// 柱高占绘图区的百分比，至少 2。
inline double bar_percent(double count, double maximum) { return std::max(2.0, (count / maximum) * 100); }

// `columnCenter`：第 `index` 列中点的横坐标，0–100。
inline double column_center(size_t index, size_t count) {
  return ((static_cast<double>(index) + 0.5) / static_cast<double>(std::max<size_t>(1, count))) * 100;
}

// 速度趋势的纵轴：不从零开始，上下端都标出来。
struct SpeedScale {
  double top = 0;
  double bottom = 0;
  double fastest = 0;
  double slowest = 0;
  size_t measured = 0;
};

inline std::optional<SpeedScale> speed_scale(std::vector<std::optional<double>> const &speeds,
                                             double average_speed) {
  SpeedScale scale;
  bool any = false;
  double high = average_speed;
  double low = average_speed;
  for (auto const &speed : speeds) {
    if (!speed)
      continue;
    if (!any) {
      scale.fastest = *speed;
      scale.slowest = *speed;
    }
    any = true;
    ++scale.measured;
    scale.fastest = std::max(scale.fastest, *speed);
    scale.slowest = std::min(scale.slowest, *speed);
    high = std::max(high, *speed);
    low = std::min(low, *speed);
  }
  if (!any)
    return std::nullopt;
  const double spread = std::max({high - low, high * 0.2, 1.0});
  scale.top = std::ceil(high + spread * 0.15);
  scale.bottom = std::max(0.0, std::floor(low - spread * 0.25));
  return scale;
}

// 速度在绘图区里的高度百分比。
inline double speed_height(SpeedScale const &scale, double value) {
  return ((value - scale.bottom) / std::max(1.0, scale.top - scale.bottom)) * 100;
}

// 点距足够时每个有速度的日子都画点，否则只画前后都没有速度的孤点。
inline bool speed_dotted(std::vector<std::optional<double>> const &speeds, size_t index, size_t day_count) {
  if (!speeds[index])
    return false;
  if (day_count <= 45)
    return true;
  const bool before = index > 0 && speeds[index - 1];
  const bool after = index + 1 < speeds.size() && speeds[index + 1];
  return !before && !after;
}

inline std::wstring speed_caption(SpeedScale const &scale, double average_speed) {
  return L"最快 " + zh_number(js_round(scale.fastest)) + L" · 最慢 " + zh_number(js_round(scale.slowest)) +
         L" · 虚线为平均 " + zh_number(js_round(average_speed)) + L" 字 / 分钟 · " +
         js_number(static_cast<double>(scale.measured)) + L" 天有测量";
}

// 每一段连续的已知值：`linePath` 遇到未知值抬笔。点是（列中点，纵坐标值）。
inline std::vector<std::vector<std::pair<double, double>>> line_runs(std::vector<std::optional<double>> const &values) {
  std::vector<std::vector<std::pair<double, double>>> runs;
  bool drawing = false;
  for (size_t index = 0; index < values.size(); ++index) {
    if (!values[index]) {
      drawing = false;
      continue;
    }
    if (!drawing)
      runs.emplace_back();
    runs.back().emplace_back(column_center(index, values.size()), *values[index]);
    drawing = true;
  }
  return runs;
}

// ---- 分布 ----

struct Slice {
  std::string id;
  std::wstring title;
  double count = 0;
  uint32_t color = 0;
  std::wstring symbol;
};

constexpr std::array<uint32_t, 8> chart_palette{0x19a78d, 0x4c8ee8, 0x7772df, 0xe59b43,
                                                0xdb6f9f, 0xa879d7, 0x98705a, 0x888b92};
// 选中的那一天：趋势柱、热力图格子和速度点都用它。
constexpr uint32_t selected_color = 0xe59b43;

// 「字符类型」。
inline std::vector<Slice> character_slices(Breakdown const &breakdown) {
  struct Kind {
    const char *id;
    const wchar_t *title;
    const wchar_t *symbol;
  };
  static constexpr std::array<Kind, 8> kinds{{{"han", L"汉字", L"汉"},
                                              {"latin", L"拉丁字母", L"A"},
                                              {"otherLetter", L"其他文字", L"文"},
                                              {"number", L"数字", L"123"},
                                              {"punctuation", L"标点", L"，"},
                                              {"emoji", L"表情", L"😀"},
                                              {"symbol", L"其他符号", L"#"},
                                              {"unknown", L"历史未分类", L"?"}}};
  std::vector<Slice> slices;
  for (size_t index = 0; index < kinds.size(); ++index)
    slices.push_back({kinds[index].id, kinds[index].title, value_of(breakdown.characters, kinds[index].id),
                      chart_palette[index % chart_palette.size()], kinds[index].symbol});
  return slices;
}

// 「输入方案」。
inline std::vector<Slice> source_slices(Breakdown const &breakdown) {
  struct Source {
    const char *id;
    const wchar_t *title;
    const wchar_t *symbol;
  };
  static constexpr std::array<Source, 21> sources{{{"quanpin", L"全拼 26 键", L"全"},
                                                   {"nineKey", L"全拼 9 键", L"9"},
                                                   {"shuangpin", L"小鹤双拼", L"鹤"},
                                                   {"ziranma", L"自然码双拼", L"自"},
                                                   {"microsoft", L"微软双拼", L"微"},
                                                   {"shoudao", L"首道双拼", L"首"},
                                                   {"wubi", L"五笔", L"五"},
                                                   {"japanese", L"日语", L"日"},
                                                   {"korean", L"韩语", L"韩"},
                                                   {"cantonese", L"粤拼", L"粤"},
                                                   {"zhuyin", L"注音", L"注"},
                                                   {"vietnamese", L"越南语", L"越"},
                                                   {"tibetan", L"藏文", L"藏"},
                                                   {"stroke", L"笔画", L"笔"},
                                                   {"handwriting", L"手写", L"手"},
                                                   {"english", L"英文键盘", L"A"},
                                                   {"local", L"本地输入", L"本"},
                                                   {"ai", L"AI 润色", L"✦"},
                                                   {"reply", L"高情商回复", L"回"},
                                                   {"voice", L"语音输入", L"♪"},
                                                   {"unknown", L"历史未分类", L"?"}}};
  std::vector<Slice> slices;
  for (size_t index = 0; index < sources.size(); ++index)
    slices.push_back({sources[index].id, sources[index].title, value_of(breakdown.sources, sources[index].id),
                      chart_palette[index % chart_palette.size()], sources[index].symbol});
  return slices;
}

// 「语言模式」。
inline std::vector<Slice> language_slices(Breakdown const &breakdown) {
  auto source = [&breakdown](const char *id) { return value_of(breakdown.sources, id); };
  return {
      {"chinese", L"中文模式",
       sum_values(breakdown.sources, {"quanpin", "nineKey", "shuangpin", "ziranma", "microsoft", "shoudao", "wubi",
                                      "cantonese", "zhuyin", "stroke"}),
       chart_palette[0], L"中"},
      {"japanese", L"日语模式", source("japanese"), chart_palette[4], L"日"},
      {"korean", L"韩语模式", source("korean"), chart_palette[6], L"韩"},
      {"vietnamese", L"越南语模式", source("vietnamese"), 0xd0605e, L"越"},
      {"tibetan", L"藏文模式", source("tibetan"), 0xb8873a, L"藏"},
      {"english", L"英文模式", source("english"), chart_palette[1], L"A"},
      {"local", L"本地输入", source("local"), chart_palette[5], L"本"},
      {"ai", L"AI 润色", source("ai"), chart_palette[3], L"✦"},
      {"reply", L"高情商回复", source("reply"), 0x55bfa0, L"回"},
      {"voice", L"语音输入", source("voice"), chart_palette[2], L"♪"},
      {"unknown", L"历史未分类", source("unknown"), chart_palette[7], L"?"},
  };
}

inline double slices_total(std::vector<Slice> const &slices) {
  double total = 0;
  for (auto const &slice : slices)
    total += slice.count;
  return total;
}

// 环形图的图例：有字的，以及除「历史未分类」以外的所有类别。
inline std::vector<Slice> legend_slices(std::vector<Slice> const &slices) {
  std::vector<Slice> visible;
  for (auto const &slice : slices)
    if (slice.count > 0 || slice.id != "unknown")
      visible.push_back(slice);
  return visible;
}

// 排行：有字的类别，多的在前，一样多时保持原来的顺序。
inline std::vector<Slice> ranked_slices(std::vector<Slice> const &slices) {
  std::vector<Slice> ranked;
  for (auto const &slice : slices)
    if (slice.count > 0)
      ranked.push_back(slice);
  std::stable_sort(ranked.begin(), ranked.end(),
                   [](Slice const &a, Slice const &b) { return a.count > b.count; });
  return ranked;
}

// 环形图的几何，单位是 0–100 的方框，与共享页的 SVG viewBox 相同。
constexpr double donut_outer = 50;
constexpr double donut_thickness = 13;
constexpr double donut_inner = donut_outer - donut_thickness;
constexpr double donut_gap = 1.1;
constexpr double donut_min_sweep = 5;

struct DonutPoint {
  double x = 0;
  double y = 0;
};

// 一个扇区：外弧从 outer_start 顺时针到 outer_end，连到 inner_end，内弧逆时针回到 inner_start。`ring` 时只画一整圈。
struct DonutSegment {
  uint32_t color = 0;
  bool ring = false;
  bool large = false;
  DonutPoint outer_start, outer_end, inner_end, inner_start;
};

inline DonutPoint donut_point(double radius, double degrees) {
  const double angle = degrees * 3.14159265358979323846 / 180;
  return {50 + radius * std::sin(angle), 50 - radius * std::cos(angle)};
}

// `sweeps`：按占比分角度，太小的扇区抬到最小角度，反复分配直到稳定。
inline std::vector<double> donut_sweeps(std::vector<double> const &counts) {
  std::vector<bool> pinned(counts.size(), false);
  for (;;) {
    size_t pinned_count = 0;
    double rest = 0;
    for (size_t index = 0; index < counts.size(); ++index) {
      if (pinned[index])
        ++pinned_count;
      else
        rest += counts[index];
    }
    const double free = 360 - static_cast<double>(pinned_count) * donut_min_sweep;
    std::vector<double> result;
    for (size_t index = 0; index < counts.size(); ++index)
      result.push_back(pinned[index] ? donut_min_sweep : (counts[index] / rest) * free);
    size_t below = counts.size();
    for (size_t index = 0; index < counts.size(); ++index)
      if (!pinned[index] && result[index] < donut_min_sweep) {
        below = index;
        break;
      }
    if (below == counts.size())
      return result;
    pinned[below] = true;
  }
}

// `donutSegments`：从 12 点钟方向顺时针，顺序与图例相同；只有一个非零类别时画一整圈。
inline std::vector<DonutSegment> donut_segments(std::vector<Slice> const &slices) {
  std::vector<Slice> visible;
  for (auto const &slice : slices)
    if (slice.count > 0)
      visible.push_back(slice);
  std::vector<DonutSegment> segments;
  if (visible.size() == 1) {
    DonutSegment ring;
    ring.color = visible[0].color;
    ring.ring = true;
    segments.push_back(ring);
    return segments;
  }
  constexpr double degrees = 180 / 3.14159265358979323846;
  const double outer_inset = std::asin(donut_gap / 2 / donut_outer) * degrees;
  const double inner_inset = std::asin(donut_gap / 2 / donut_inner) * degrees;
  std::vector<double> counts;
  for (auto const &slice : visible)
    counts.push_back(slice.count);
  const auto sweeps = donut_sweeps(counts);
  double cursor = 0;
  for (size_t index = 0; index < sweeps.size(); ++index) {
    const double start = cursor;
    cursor += sweeps[index];
    DonutSegment segment;
    segment.color = visible[index].color;
    segment.large = sweeps[index] - 2 * outer_inset > 180;
    segment.outer_start = donut_point(donut_outer, start + outer_inset);
    segment.outer_end = donut_point(donut_outer, cursor - outer_inset);
    segment.inner_end = donut_point(donut_inner, cursor - inner_inset);
    segment.inner_start = donut_point(donut_inner, start + inner_inset);
    segments.push_back(segment);
  }
  return segments;
}

// ---- 候选命中位置 ----

struct RankRow {
  std::wstring label;
  double count = 0;
};

struct CandidateRanks {
  std::vector<RankRow> rows;
  double total = 0;
  double peak = 1;
  double first = 0;
};

// `CandidateRanks`：第 1 到第 9 条，以及第 10 条以后。
inline CandidateRanks candidate_ranks(Statistics const &statistics) {
  CandidateRanks ranks;
  for (size_t index = 0; index < 9; ++index) {
    const double count = index < statistics.ranks.size() ? statistics.ranks[index] : 0;
    ranks.rows.push_back({L"第 " + std::to_wstring(index + 1) + L" 条", count});
    ranks.total += count;
    ranks.peak = std::max(ranks.peak, count);
  }
  ranks.first = ranks.rows[0].count;
  ranks.rows.push_back({L"第 10 条以后", statistics.beyond});
  ranks.total += statistics.beyond;
  ranks.peak = std::max(ranks.peak, statistics.beyond);
  return ranks;
}

// ---- 按键热力图 ----

struct LayoutKey {
  std::wstring label;
  double weight = 1;
  std::string code;
};

using KeyRows = std::vector<std::vector<LayoutKey>>;

namespace layouts {

inline std::string character_code(wchar_t character) {
  if (character >= L'a' && character <= L'z')
    return std::string("Key") + static_cast<char>(character - L'a' + 'A');
  if (character >= L'0' && character <= L'9')
    return std::string("Digit") + static_cast<char>(character - L'0' + '0');
  switch (character) {
  case L'`':
    return "Backquote";
  case L'-':
    return "Minus";
  case L'=':
    return "Equal";
  case L'[':
    return "BracketLeft";
  case L']':
    return "BracketRight";
  case L'\\':
    return "Backslash";
  case L';':
    return "Semicolon";
  case L'\'':
    return "Quote";
  case L',':
    return "Comma";
  case L'.':
    return "Period";
  case L'/':
    return "Slash";
  default:
    return {};
  }
}

inline std::vector<LayoutKey> letters(std::wstring_view text) {
  std::vector<LayoutKey> keys;
  for (const wchar_t character : text)
    keys.push_back({std::wstring(1, character), 1, character_code(character)});
  return keys;
}

inline std::vector<LayoutKey> join(std::vector<std::vector<LayoutKey>> parts) {
  std::vector<LayoutKey> row;
  for (auto &part : parts)
    row.insert(row.end(), part.begin(), part.end());
  return row;
}

inline std::vector<LayoutKey> function_keys(int from, int to) {
  std::vector<LayoutKey> keys;
  for (int number = from; number <= to; ++number) {
    const auto name = "F" + std::to_string(number);
    keys.push_back({widen(name), 1, name});
  }
  return keys;
}

} // namespace layouts

// `ansiHeatmapKeyboardRows`：Esc 和功能键一行，加上 `desktopKeyboardRows` 的前四行和真正 ANSI 键盘的底行。
inline KeyRows const &ansi_rows() {
  using namespace layouts;
  static const KeyRows rows{
      join({{{L"Esc", 1, "Escape"}, {L"", 0.9, ""}},
            function_keys(1, 4),
            {{L"", 0.5, ""}},
            function_keys(5, 8),
            {{L"", 0.5, ""}},
            function_keys(9, 12)}),
      join({letters(L"`1234567890-="), {{L"Backspace", 1.9, "Backspace"}}}),
      join({{{L"Tab", 1.5, "Tab"}}, letters(L"qwertyuiop[]"), {{L"\\", 1.4, "Backslash"}}}),
      join({{{L"Caps Lock", 1.85, "CapsLock"}}, letters(L"asdfghjkl;'"), {{L"Enter", 2, "Enter"}}}),
      join({{{L"Shift", 2.35, "ShiftLeft"}}, letters(L"zxcvbnm,./"), {{L"Shift", 2.15, "ShiftRight"}}}),
      {{L"Ctrl", 1.25, "ControlLeft"},
       {L"Win", 1.25, "MetaLeft"},
       {L"Alt", 1.25, "AltLeft"},
       {L"Space", 6.7, "Space"},
       {L"Alt", 1.25, "AltRight"},
       {L"Win", 1.25, "MetaRight"},
       {L"Menu", 1.25, "ContextMenu"},
       {L"Ctrl", 1.25, "ControlRight"}},
  };
  return rows;
}

// `touchKeyboardRows`：记到屏幕键盘专有的键时画手机的 26 键。
inline KeyRows const &touch_rows() {
  using namespace layouts;
  static const KeyRows rows{
      letters(L"qwertyuiop"),
      join({{{L"", 0.5, ""}}, letters(L"asdfghjkl"), {{L"", 0.5, ""}}}),
      join({{{L"⇧", 1.5, "ShiftLeft"}}, letters(L"zxcvbnm"), {{L"⌫", 1.5, "Backspace"}}}),
      {{L"符号", 1.5, "SoftSymbol"},
       {L"中/英", 1.5, "SoftLanguage"},
       {L"空格", 4.5, "Space"},
       {L"，", 1, "Comma"},
       {L"↵", 1.5, "Enter"}},
  };
  return rows;
}

// `nineKeyHeatmapRows`。
inline KeyRows const &nine_rows() {
  static const KeyRows rows{
      {{L"1 ，。", 1, "Nine1"}, {L"2 ABC", 1, "Nine2"}, {L"3 DEF", 1, "Nine3"}},
      {{L"4 GHI", 1, "Nine4"}, {L"5 JKL", 1, "Nine5"}, {L"6 MNO", 1, "Nine6"}},
      {{L"7 PQRS", 1, "Nine7"}, {L"8 TUV", 1, "Nine8"}, {L"9 WXYZ", 1, "Nine9"}},
      {{L"", 1, ""}, {L"0", 1, "Nine0"}, {L"", 1, ""}},
  };
  return rows;
}

inline bool starts_with(std::string_view value, std::string_view prefix) {
  return value.substr(0, prefix.size()) == prefix;
}

inline bool all_digits(std::string_view value) {
  return !value.empty() && std::all_of(value.begin(), value.end(), [](char c) { return c >= '0' && c <= '9'; });
}

// `keyLabel` 在 Windows 上的写法：键被列出和读出的名字；不认识的 id 原样显示。
inline std::wstring key_label(std::string const &code) {
  static const std::map<std::string, std::wstring> fixed{
      {"Backquote", L"`"},
      {"Minus", L"-"},
      {"Equal", L"="},
      {"BracketLeft", L"["},
      {"BracketRight", L"]"},
      {"Backslash", L"\\"},
      {"Semicolon", L";"},
      {"Quote", L"'"},
      {"Comma", L","},
      {"Period", L"."},
      {"Slash", L"/"},
      {"IntlBackslash", L"ISO \\"},
      {"IntlRo", L"ろ"},
      {"IntlYen", L"¥"},
      {"Lang1", L"Lang1"},
      {"Lang2", L"Lang2"},
      {"Convert", L"変換"},
      {"NonConvert", L"無変換"},
      {"KanaMode", L"かな"},
      {"Space", L"空格"},
      {"Enter", L"回车"},
      {"Backspace", L"退格"},
      {"Tab", L"Tab"},
      {"Escape", L"Esc"},
      {"Delete", L"Delete"},
      {"Insert", L"Insert"},
      {"Home", L"Home"},
      {"End", L"End"},
      {"PageUp", L"Page Up"},
      {"PageDown", L"Page Down"},
      {"ArrowUp", L"上箭头"},
      {"ArrowDown", L"下箭头"},
      {"ArrowLeft", L"左箭头"},
      {"ArrowRight", L"右箭头"},
      {"CapsLock", L"Caps Lock"},
      {"ShiftLeft", L"左 Shift"},
      {"ShiftRight", L"右 Shift"},
      {"Fn", L"Fn"},
      {"ContextMenu", L"菜单键"},
      {"NumpadDecimal", L"小键盘 ."},
      {"NumpadEnter", L"小键盘回车"},
      {"NumpadAdd", L"小键盘 +"},
      {"NumpadSubtract", L"小键盘 -"},
      {"NumpadMultiply", L"小键盘 *"},
      {"NumpadDivide", L"小键盘 /"},
      {"NumLock", L"Num Lock"},
      {"Nine1", L"九宫格 1（标点）"},
      {"SoftPunctuation", L"九宫格侧栏标点"},
      {"SoftSymbol", L"符号"},
      {"SoftLayer", L"123"},
      {"SoftLanguage", L"中/英"},
      {"SoftGlobe", L"地球键"},
      {"SoftEmoji", L"表情"},
      {"SoftVoice", L"语音"},
  };
  if (const auto found = fixed.find(code); found != fixed.end())
    return found->second;
  const std::string_view view(code);
  if (view.size() == 4 && starts_with(view, "Key") && view[3] >= 'A' && view[3] <= 'Z')
    return widen(view.substr(3));
  if (view.size() == 6 && starts_with(view, "Digit") && all_digits(view.substr(5)))
    return widen(view.substr(5));
  if ((view.size() == 2 || view.size() == 3) && view[0] == 'F' && all_digits(view.substr(1)))
    return widen(view);
  if (view.size() == 7 && starts_with(view, "Numpad") && all_digits(view.substr(6)))
    return L"小键盘 " + widen(view.substr(6));
  if (view.size() == 5 && starts_with(view, "Nine") && all_digits(view.substr(4)))
    return L"九宫格 " + widen(view.substr(4));
  for (const auto &[prefix, name] : std::array<std::pair<std::string_view, std::wstring_view>, 3>{
           {{"Control", L"Ctrl"}, {"Alt", L"Alt"}, {"Meta", L"Win"}}}) {
    if (!starts_with(view, prefix))
      continue;
    const auto side = view.substr(prefix.size());
    if (side == "Left" || side == "Right")
      return std::wstring(side == "Left" ? L"左 " : L"右 ") + std::wstring(name);
  }
  return widen(view);
}

// `capLabel` 在 Windows 上的写法：键帽上印的字。
inline std::wstring cap_label(LayoutKey const &key) {
  const std::string_view code(key.code);
  if (code.empty())
    return key.label;
  if (code.size() == 4 && starts_with(code, "Key") && code[3] >= 'A' && code[3] <= 'Z')
    return widen(code.substr(3));
  if (starts_with(code, "Meta"))
    return L"Win";
  if (starts_with(code, "Alt"))
    return L"Alt";
  if (starts_with(code, "Control"))
    return L"Ctrl";
  return key.label;
}

// `scopedKeyCounts`：范围内各天的按键次数相加，只留正的有限值。
inline Counts scoped_key_counts(std::map<std::string, Counts> const &daily_keys,
                                std::optional<std::vector<std::string>> const &scope) {
  Counts result;
  auto add = [&result](Counts const &keys) {
    for (auto const &[code, count] : keys)
      if (std::isfinite(count) && count > 0)
        result[code] += count;
  };
  if (!scope) {
    for (auto const &[day, keys] : daily_keys)
      add(keys);
    return result;
  }
  for (auto const &day : *scope)
    if (const auto found = daily_keys.find(day); found != daily_keys.end())
      add(found->second);
  return result;
}

struct HeatKey {
  std::string code;
  std::wstring label;
  std::wstring name;
  double weight = 1;
  double count = 0;
};

struct KeyCount {
  std::string code;
  std::wstring label;
  double count = 0;
};

struct KeyboardModel {
  bool touch = false;
  std::vector<std::vector<HeatKey>> rows;
  std::optional<std::vector<std::vector<HeatKey>>> nine;
  std::vector<KeyCount> others;
  std::vector<KeyCount> top;
  double maximum = 1;
  double total = 0;
};

// `keyboardHeatmapModel`（桌面页，`mobile` 为假）：记到屏幕键盘专有的键时画手机的 26 键（按过九宫格时再加九宫格），否则画 ANSI 键盘。
inline KeyboardModel keyboard_model(Counts const &counts) {
  KeyboardModel model;
  auto soft = [](std::string const &code) { return starts_with(code, "Nine") || starts_with(code, "Soft"); };
  model.touch = std::any_of(counts.begin(), counts.end(), [&soft](auto const &entry) { return soft(entry.first); });
  std::set<std::string> drawn;
  auto place = [&drawn, &counts](KeyRows const &rows) {
    std::vector<std::vector<HeatKey>> placed;
    for (auto const &row : rows) {
      std::vector<HeatKey> keys;
      for (auto const &key : row) {
        if (!key.code.empty())
          drawn.insert(key.code);
        keys.push_back({key.code, cap_label(key), key.code.empty() ? std::wstring() : key_label(key.code), key.weight,
                        key.code.empty() ? 0 : value_of(counts, key.code)});
      }
      placed.push_back(std::move(keys));
    }
    return placed;
  };
  model.rows = place(model.touch ? touch_rows() : ansi_rows());
  if (model.touch &&
      std::any_of(counts.begin(), counts.end(), [](auto const &entry) { return starts_with(entry.first, "Nine"); }))
    model.nine = place(nine_rows());
  std::vector<KeyCount> listed;
  for (auto const &[code, count] : counts)
    if (count > 0)
      listed.push_back({code, key_label(code), count});
  std::sort(listed.begin(), listed.end(), [](KeyCount const &a, KeyCount const &b) {
    if (a.count != b.count)
      return a.count > b.count;
    return a.code < b.code;
  });
  for (auto const &key : listed) {
    model.total += key.count;
    model.maximum = std::max(model.maximum, key.count);
    if (!drawn.count(key.code))
      model.others.push_back(key);
  }
  model.top.assign(listed.begin(), listed.begin() + static_cast<std::ptrdiff_t>(std::min<size_t>(5, listed.size())));
  return model;
}

// `keyHeatLevel`：0（没按过）到 4。
inline int key_heat_level(double count, double maximum) {
  if (count <= 0)
    return 0;
  return static_cast<int>(std::clamp(std::ceil((count / maximum) * 4), 1.0, 4.0));
}

// 日历热力图的档位：零是 0，其余至少 1。
inline int day_heat_level(double count, double maximum) {
  if (count == 0)
    return 0;
  return static_cast<int>(std::max(1.0, std::ceil((count / maximum) * 4)));
}

// 五档颜色：0 档是底色，其余是强调色的这几种不透明度。
constexpr std::array<double, 5> heat_opacity{0, 0.28, 0.46, 0.68, 1};

// 「自动清理」的选项；别的取值按永久保留读。
struct RetentionChoice {
  const char *value;
  const wchar_t *label;
};
constexpr std::array<RetentionChoice, 5> retention_choices{{{"forever", L"永久保留"},
                                                            {"30d", L"保留最近 30 天"},
                                                            {"90d", L"保留最近 90 天"},
                                                            {"180d", L"保留最近 180 天"},
                                                            {"365d", L"保留最近 365 天"}}};

inline std::string retention_value(std::string const &retention) {
  for (auto const &choice : retention_choices)
    if (retention == choice.value)
      return retention;
  return "forever";
}

} // namespace msime::settings::typing
