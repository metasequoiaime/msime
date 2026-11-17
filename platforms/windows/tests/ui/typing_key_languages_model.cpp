#include "../../settings/TypingKeyLanguagesJson.h"
#include <cstdio>

using namespace msime::settings::typing;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

std::vector<std::string> choice_ids(KeyLanguageSnapshot const &snapshot) {
  std::vector<std::string> result;
  for (auto const &choice : snapshot.choices())
    result.emplace_back(choice.id);
  return result;
}
} // namespace

int main() {
  // 三个合成记录日：全部总计还包含尚未识别的未来语言。
  std::map<std::string, Counts> totals{
      {"2026-11-01", {{"KeyA", 2}}},
      {"2026-11-16", {{"KeyA", 8}, {"Space", 3}, {"Nine2", 2}, {"SoftEmoji", 1}}},
      {"2026-11-17", {{"KeyA", 5}, {"KeyZ", 6}}}};
  DailyKeyLanguages languages{
      {"2026-11-01", {{"japanese", {{"KeyA", 2}}}}},
      {"2026-11-16", {{"chinese", {{"KeyA", 5}}}, {"english", {{"KeyA", 3}}},
                      {"common", {{"Space", 3}}}, {"unknown", {{"Nine2", 2}, {"SoftEmoji", 1}}}}},
      {"2026-11-17", {{"english", {{"KeyA", 5}}}, {"future-language", {{"KeyZ", 6}}}}}};
  const KeyLanguageSnapshot all(totals, languages, std::nullopt);
  check(all.counts("all").at("KeyA") == 15, "whole retention includes all recorded days");
  check(all.counts("english").at("KeyA") == 8, "language totals span the same days");
  check(choice_ids(all) == std::vector<std::string>{"all", "chinese", "english", "japanese", "common", "unknown"},
        "only recorded known languages appear in stable order");
  check(all.counts("all").at("KeyZ") == 6 && all.resolved("future-language") == "all",
        "future languages remain in complete totals without invented picker labels");

  const std::vector<std::string> recent{"2026-11-16", "2026-11-17"};
  const KeyLanguageSnapshot week(totals, languages, recent);
  check(week.counts("all").at("KeyA") == 13 && week.counts("english").at("KeyA") == 8,
        "recent range applies to complete and filtered counts");
  check(week.resolved("japanese") == "all" && week.resolved("missing") == "all",
        "missing selections fall back after a range change");
  const KeyLanguageSnapshot day(totals, languages, std::vector<std::string>{"2026-11-16"});
  check(day.counts("all").at("KeyA") == 8 && day.counts("english").at("KeyA") == 3,
        "selected day excludes other days");
  check(day.counts("common") == Counts{{"Space", 3}}, "common keys have their own filter");
  check(day.counts("unknown") == Counts({{"Nine2", 2}, {"SoftEmoji", 1}}),
        "unclassified keys remain accessible");
  const KeyLanguageSnapshot duplicate(totals, languages,
      std::vector<std::string>{"2026-11-16", "2026-11-16", "missing"});
  check(duplicate.counts("all") == day.counts("all") &&
            duplicate.counts("english") == day.counts("english"), "scope dates are a set");
  const KeyLanguageSnapshot empty(totals, languages, std::vector<std::string>{});
  check(choice_ids(empty) == std::vector<std::string>{"all"} && empty.counts("all").empty(),
        "explicit empty scope does not mean all history");

  const KeyLanguageSnapshot legacy(totals, {}, std::nullopt);
  check(choice_ids(legacy) == std::vector<std::string>{"all"} && legacy.counts("all") == all.counts("all"),
        "older cores retain complete totals without guessing historical languages");
  totals.clear();
  languages.clear();
  check(week.counts("english").at("KeyA") == 8 && week.counts("all").at("KeyA") == 13,
        "snapshots own counts after source reload");

  // 同一份筛选计数驱动热力、最常按键、九键布局和其他键。
  const auto latin = keyboard_model(day.counts("english"));
  check(!latin.touch && !latin.nine && latin.total == 3 && latin.others.empty() &&
            latin.top.size() == 1 && latin.top.front().code == "KeyA", "language filters feed the actual keyboard model");
  bool found = false;
  for (auto const &row : latin.rows)
    for (auto const &key : row)
      if (key.code == "KeyA") found = key.count == 3;
  check(found, "heatmap cells consume the filtered value");
  const auto unknown = keyboard_model(day.counts("unknown"));
  check(unknown.touch && unknown.nine && unknown.total == 3 && unknown.others.size() == 1 &&
            unknown.others.front().code == "SoftEmoji", "touch layout and other keys use the same filter");

  const auto maximum = std::numeric_limits<double>::max();
  const Counts invalid{{"KeyA", 0}, {"KeyB", -1}, {"KeyC", std::numeric_limits<double>::infinity()},
                       {"KeyD", std::numeric_limits<double>::quiet_NaN()}};
  const KeyLanguageSnapshot clean({{"one", invalid}}, {{"one", {{"english", invalid}}}}, std::nullopt);
  check(clean.counts("all").empty() && choice_ids(clean) == std::vector<std::string>{"all"},
        "nonpositive and nonfinite values create no picker option");
  const KeyLanguageSnapshot saturated({{"one", {{"KeyA", maximum}}}, {"two", {{"KeyA", maximum}}}},
      {{"one", {{"english", {{"KeyA", maximum}}}}}, {"two", {{"english", {{"KeyA", maximum}}}}}}, std::nullopt);
  check(saturated.counts("all").at("KeyA") == maximum && saturated.counts("english").at("KeyA") == maximum,
        "per-key accumulation remains finite");

  // 测真实 Win7 数据适配器，不用测试内的第二套解析。
  using Json = nlohmann::json;
  const auto parsed = key_languages_from_json(Json::parse(R"({"2026-11-17":{"english":{"KeyA":5},"common":{"Space":2},"future-language":{"KeyZ":6}}})"));
  check(parsed && parsed->at("2026-11-17").at("english").at("KeyA") == 5 &&
            parsed->at("2026-11-17").at("future-language").at("KeyZ") == 6, "JSON adapter retains language keys and counts");
  check(key_languages_from_json(Json()) && key_languages_from_json(Json())->empty(), "null supports older documents");
  check(key_languages_from_json(Json::object()) && key_languages_from_json(Json::object())->empty(), "empty language history is valid");
  for (auto const &bad : {R"([])", R"({"day":[]})", R"({"day":{"english":[]}})",
                         R"({"day":{"english":{"KeyA":"5"}}})", R"({"day":{"english":{"KeyA":true}}})", R"({"day":null})"})
    check(!key_languages_from_json(Json::parse(bad)), "malformed language history is unavailable, not empty");
  Json nonfinite = {{"day", {{"english", {{"KeyA", std::numeric_limits<double>::infinity()}}}}}};
  check(!key_languages_from_json(nonfinite), "JSON adapter rejects nonfinite numeric values");
  const auto labels = all.choices();
  check(std::wstring(labels.front().title) == L"全部" && std::wstring(labels.back().title) == L"未分类",
        "native picker uses Chinese labels");
  return failures ? 1 : 0;
}
