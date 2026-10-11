#include "../../settings/TypingStatisticsModel.h"
#include <cstdio>
#include <string>

using namespace msime::settings::typing;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
bool near(double a, double b) { return std::fabs(a - b) < 1e-9; }

// 合成的统计：三天记录，其中两天量过活跃时间。
Statistics sample() {
  Statistics statistics;
  statistics.enabled = true;
  statistics.total = 1300;
  statistics.days = {{"2026-10-09", 300}, {"2026-10-10", 400}, {"2026-10-11", 600}, {"bad", 9}};
  statistics.detail.characters = {{"han", 900}, {"latin", 200}, {"number", 50}};
  statistics.detail.sources = {{"quanpin", 1000}, {"voice", 100}};
  statistics.daily_details["2026-10-10"].characters = {{"han", 300}, {"latin", 60}, {"punctuation", 20}};
  statistics.daily_details["2026-10-10"].sources = {{"quanpin", 380}};
  statistics.daily_details["2026-10-11"].characters = {{"han", 500}, {"otherLetter", 40}};
  statistics.daily_active_ms = {{"2026-10-10", 120000}, {"2026-10-11", 30000}};
  statistics.daily_hours["2026-10-10"] = std::vector<double>(24, 0);
  statistics.daily_hours["2026-10-10"][9] = 400;
  statistics.daily_hours["2026-10-11"] = std::vector<double>(24, 0);
  statistics.daily_hours["2026-10-11"][21] = 600;
  statistics.ranks = {80, 15, 5};
  statistics.beyond = 2;
  statistics.daily_keys["2026-10-10"] = {{"KeyA", 30}, {"Space", 12}, {"ArrowLeft", 3}};
  statistics.daily_keys["2026-10-11"] = {{"KeyA", 10}, {"Delete", 4}};
  return statistics;
}
} // namespace

int main() {
  // 数字格式。
  check(to_fixed(0.25, 1) == "0.3", "toFixed rounds an exact half up");
  check(to_fixed(1.005, 2) == "1.00", "toFixed uses the exact binary value");
  check(to_fixed(99.95, 1) == "100.0", "toFixed carries");
  check(to_fixed(9.96, 1) == "10.0", "toFixed carries into the integer part");
  check(to_fixed(-0.04, 1) == "-0.0" && to_fixed(-0.0, 1) == "0.0", "toFixed keeps the sign of a negative number");
  check(js_round(2.5) == 3 && js_round(-2.5) == -2 && js_round(0.49999999999999994) == 0, "Math.round");
  check(zh_number(1234567) == L"1,234,567", "grouping");
  check(zh_number(0) == L"0" && zh_number(999) == L"999" && zh_number(1000) == L"1,000", "small numbers");
  check(zh_number(12.34567) == L"12.346", "three fraction digits");
  check(zh_number(-1500) == L"-1,500", "negative numbers");
  check(zh_percent(1, 3) == L"33.3%" && zh_percent(1, 0) == L"—", "percent");
  check(active_time(0) == L"0分" && active_time(400) == L"1秒" && active_time(12400) == L"12秒",
        "active seconds");
  check(active_time(45 * 60000.0) == L"45分" && active_time(120 * 60000.0) == L"2小时" &&
            active_time(65 * 60000.0) == L"1小时5分",
        "active minutes and hours");
  check(js_number(42) == L"42" && js_number(0.5) == L"0.5", "plain numbers");
  check(zh_date_time(2026, 1, 5, 9, 3, 5) == L"2026/1/5 09:03:05", "zh-CN date and time");

  // 日期。
  const CivilDay today{2026, 10, 11};
  check(weekday(today) == 0, "2026-10-11 is a Sunday");
  check(weekday({1970, 1, 1}) == 4, "1970-01-01 is a Thursday");
  check(add_days("2026-03-01", -1) == "2026-02-28" && add_days("2024-03-01", -1) == "2024-02-29",
        "adding days crosses months and leap days");
  check(add_days("2026-02-30", 1) == "2026-02-30" && add_days("x", 1) == "x", "not a date stays as it is");
  const auto days = recent_days(30, today);
  check(days.size() == 30 && days.front().key == "2026-09-12" && days.back().key == "2026-10-11" &&
            days.front().label == L"9月12日",
        "the trend covers thirty days ending today");
  check(day_label("2026-09-01") == L"9月1日", "day label drops the leading zeros");
  check(statistic_day_keys(sample().days).size() == 3, "malformed day keys are ignored");

  const auto weeks = heatmap_weeks(sample().days, today);
  check(weeks.size() == 53, "53 weeks");
  check(weeks.back()[6].key == "2026-10-11" && !weeks.back()[6].future, "today ends the last week on Sunday");
  check(weeks.back()[0].key == "2026-10-05", "weeks start on Monday");
  check(weeks.front()[0].key == "2025-10-06", "the first column is 52 weeks earlier");
  check(weeks.back()[6].count == 600, "cells carry their counts");
  const auto wednesday = heatmap_weeks({}, CivilDay{2026, 10, 7});
  check(wednesday.back()[2].key == "2026-10-07" && !wednesday.back()[2].future && wednesday.back()[3].future,
        "days after today are future");
  const auto months = heatmap_month_labels(weeks);
  check(months.back().empty() && months[months.size() - 2] == L"10月", "a month is labelled where it begins");

  check(current_streak({"2026-10-09", "2026-10-10"}, "2026-10-11") == 2, "a streak may end yesterday");
  check(current_streak({"2026-10-09", "2026-10-10", "2026-10-11"}, "2026-10-11") == 3, "today extends it");
  check(current_streak({"2026-10-08"}, "2026-10-11") == 0, "a gap breaks it");
  check(longest_streak({"2026-02-27", "2026-02-28", "2026-03-01", "2026-03-05"}) == 3,
        "the longest streak crosses a month");

  // 活跃与速度。
  const auto statistics = sample();
  const auto activity = activity_metrics(statistics, "2026-10-11");
  check(activity.recorded_days == 3 && near(activity.average_per_day, 1300.0 / 3), "average per recorded day");
  check(activity.best_day == std::optional<std::string>("2026-10-11") && activity.best_day_characters == 600,
        "best day");
  check(near(activity.total_active_ms, 150000) && near(activity.average_speed, 900 / 2.5), "average speed");
  check(near(activity.today_speed, 540 / 0.5), "today's speed");
  check(activity.fastest_day == std::optional<std::string>("2026-10-10") && near(activity.fastest_speed, 180),
        "a day under one minute cannot be fastest");
  check(activity.current_streak == 3 && activity.longest_streak == 3, "streaks");
  check(activity.today_hours && (*activity.today_hours)[21] == 600, "today's hours");
  const auto metrics = overview_metrics(statistics, "2026-10-11", L"累计输入", statistics.total, activity);
  check(metrics.size() == 6 && metrics[0].value == L"600" && metrics[1].value == L"1,300", "overview tiles");
  check(metrics[2].value == L"30秒" && metrics[3].value == L"1,080" && metrics[4].note == L"共 2分",
        "activity tiles");
  check(metrics[5].note == L"最长 3 天", "streak tile");
  const auto details = overview_details(activity);
  check(details.summary == L"日均 433 字符 · 3 天有记录", "overview summary");
  check(details.best == L"最多 10月11日，600 字符 · 最快 10月10日，180 字 / 分钟", "overview best");
  check(overview_details(Activity{}).best == L"还没有记录", "no records yet");

  const auto usual = usual_hours(statistics, "2026-10-11");
  check(usual && usual->days == 1 && usual->hours[9] == 400, "the usual day leaves today out");
  const auto speeds = daily_speeds(statistics, {"2026-10-09", "2026-10-10", "2026-10-11"});
  check(!speeds[0] && speeds[1] && near(*speeds[1], 180) && !speeds[2], "speeds need a minute");
  const auto scale = speed_scale(speeds, activity.average_speed);
  check(scale && scale->measured == 1 && scale->top == std::ceil(360 + 180 * 0.15) &&
            scale->bottom == std::floor(180 - 180 * 0.25),
        "the speed scale crops around the data");
  check(speed_caption(*scale, activity.average_speed) == L"最快 180 · 最慢 180 · 虚线为平均 360 字 / 分钟 · 1 天有测量",
        "speed caption");
  check(!speed_scale({std::nullopt}, 0), "no measured day draws nothing");
  check(speed_dotted(speeds, 1, 3) && !speed_dotted(speeds, 0, 3), "dots");
  const auto runs = line_runs({1.0, std::nullopt, 2.0, 3.0});
  check(runs.size() == 2 && runs[1].size() == 2 && near(runs[0][0].first, 12.5), "a gap lifts the pen");

  const auto rows = daily_detail_rows(statistics, "2026-10-11");
  check(rows.size() == 3 && rows[0].key == "2026-10-11", "detail rows are newest first");
  check(rows[0].han == 500 && rows[0].other == 100 && rows[0].active_ms && *rows[0].speed == 1080,
        "other folds the remainder");
  check(!rows[2].active_ms && !rows[2].speed && rows[2].other == 300, "an unmeasured day has no speed");
  check(daily_detail_rows(statistics, "2026-10-10").size() == 2, "future days are not rows");

  const auto whole = scoped_breakdown(statistics, std::nullopt);
  check(whole.characters.at("unknown") == 150 && whole.sources.at("unknown") == 200, "the unclassified remainder");
  const auto day = scoped_breakdown(statistics, std::vector<std::string>{"2026-10-10"});
  check(day.characters.at("han") == 300 && day.characters.at("unknown") == 20 && day.sources.at("unknown") == 20,
        "a day's breakdown");

  const auto trend = trend_scale(statistics, days);
  check(trend.maximum == 600 && near(trend.average, 1300.0 / 3), "trend scale");
  check(trend_caption(trend) == L"最高 600 字符 / 天 · 虚线为日均 433 字符", "trend caption");
  check(trend_caption(trend_scale(Statistics{}, days)) == L"最高 0 字符 / 天", "an empty trend");
  check(bar_percent(0, 600) == 2 && bar_percent(300, 600) == 50, "bar height");

  check(availability_notice(false, 0, true, L"").empty(), "nothing to explain while recording is off");
  check(availability_notice(true, 0, true, L"") ==
            L"键盘从未写入过统计。请用水杉键盘成功输入几个字符，再返回此页刷新。",
        "never written");
  check(availability_notice(true, 5, false, L"x").empty(), "counts need no notice");
  check(availability_notice(true, 0, false, L"2026/10/11 09:00:00") ==
            L"统计最后写入于 2026/10/11 09:00:00，当前计数为零；如果刚刚清空过统计，这是正常的。",
        "written but zero");
  check(availability_notice(true, 0, false, L"") == L"统计文件已建立，但当前还没有输入记录。", "no time");

  // 分布。
  const auto kinds = character_slices(whole);
  check(kinds.size() == 8 && kinds[0].title == L"汉字" && kinds[0].count == 900 && kinds[7].count == 150,
        "character slices");
  check(slices_total(kinds) == 1300, "slices add up");
  check(legend_slices(character_slices(Breakdown{})).size() == 7, "an empty unknown is not in the legend");
  const auto languages = language_slices(whole);
  check(languages[0].count == 1000 && languages[9].count == 100 && languages[10].count == 200, "language modes");
  const auto ranked = ranked_slices(source_slices(whole));
  check(ranked.size() == 3 && ranked[0].id == "quanpin" && ranked[1].id == "unknown" && ranked[2].id == "voice",
        "the ranking is by count");
  const auto ring = donut_segments(language_slices(day));
  check(ring.size() == 2, "two slices are two segments");
  const auto single = donut_segments({Slice{"a", L"a", 3, 1, L""}, Slice{"b", L"b", 0, 2, L""}});
  check(single.size() == 1 && single[0].ring, "one slice is a whole ring");
  const auto sweeps = donut_sweeps({1000, 1, 1});
  check(near(sweeps[0] + sweeps[1] + sweeps[2], 360) && near(sweeps[1], 5) && near(sweeps[2], 5),
        "tiny slices get the minimum sweep");
  const auto segments = donut_segments({Slice{"a", L"a", 1, 1, L""}, Slice{"b", L"b", 3, 2, L""}});
  check(segments[1].large && !segments[0].large, "a slice over half the ring takes the large arc");
  check(near(segments[0].outer_start.y, 50 - 50 * std::cos(std::asin(0.55 / 50))), "segments start at 12 o'clock");

  const auto ranks = candidate_ranks(statistics);
  check(ranks.rows.size() == 10 && ranks.total == 102 && ranks.peak == 80 && ranks.first == 80, "candidate ranks");
  check(ranks.rows[9].label == L"第 10 条以后" && ranks.rows[2].label == L"第 3 条", "rank labels");

  // 按键。
  check(key_label("KeyQ") == L"Q" && key_label("Digit7") == L"7" && key_label("F11") == L"F11", "key names");
  check(key_label("Numpad3") == L"小键盘 3" && key_label("Nine4") == L"九宫格 4" && key_label("Nine1") ==
            L"九宫格 1（标点）",
        "keypad names");
  check(key_label("MetaLeft") == L"左 Win" && key_label("ControlRight") == L"右 Ctrl" &&
            key_label("AltLeft") == L"左 Alt",
        "modifier names on Windows");
  check(key_label("Weird") == L"Weird" && key_label("Space") == L"空格", "fixed and unknown names");
  const auto counts = scoped_key_counts(statistics.daily_keys, std::nullopt);
  check(counts.at("KeyA") == 40 && counts.size() == 4, "all days");
  const auto keyboard = keyboard_model(counts);
  check(!keyboard.touch && keyboard.rows.size() == 6 && !keyboard.nine, "a physical keyboard is ANSI");
  check(keyboard.total == 59 && keyboard.maximum == 40 && keyboard.top.size() == 4 && keyboard.top[0].code == "KeyA",
        "totals and top keys");
  check(keyboard.others.size() == 2 && keyboard.others[0].code == "Delete" && keyboard.others[1].code == "ArrowLeft",
        "keys the board does not draw are listed");
  check(keyboard.rows[5][1].label == L"Win" && keyboard.rows[3][1].label == L"A" && keyboard.rows[3][1].count == 40,
        "caps and counts");
  const auto touch = keyboard_model({{"Nine4", 2}, {"KeyA", 1}});
  check(touch.touch && touch.nine && touch.others.empty(), "an on-screen key draws the phone layout");
  check(key_heat_level(0, 10) == 0 && key_heat_level(1, 10) == 1 && key_heat_level(10, 10) == 4, "key levels");
  check(day_heat_level(0, 10) == 0 && day_heat_level(3, 10) == 2, "day levels");
  check(retention_value("90d") == "90d" && retention_value("7d") == "forever", "retention");

  if (failures == 0)
    std::puts("typing statistics model: ok");
  return failures == 0 ? 0 : 1;
}
