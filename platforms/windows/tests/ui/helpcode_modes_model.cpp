#include "../../settings/HelpcodeModesModel.h"
#include <cstdio>
#include <set>
#include <string>

using namespace msime::settings::helpcode;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

Mode const &mode(std::wstring_view field) {
  for (auto const &entry : shuangpin_modes)
    if (entry.field == field)
      return entry;
  std::fprintf(stderr, "FAIL: missing mode\n");
  ++failures;
  return shuangpin_modes.front();
}
} // namespace

int main() {
  // 字段和顺序照共享页 modesRows；不含只在 macOS 生效的 manual_tab。
  const std::wstring_view order[] = {
      L"direct",          L"trigger_semicolon",      L"trigger_backtick",
      L"trigger_uppercase", L"mid_sentence",         L"mid_sentence_backtick",
      L"mid_sentence_semicolon", L"mid_sentence_uppercase", L"direct_helpcode",
      L"direct_helpcode_slash", L"direct_helpcode_uppercase"};
  check(shuangpin_modes.size() == std::size(order), "eleven modes");
  for (size_t index = 0; index < shuangpin_modes.size() && index < std::size(order); ++index)
    check(shuangpin_modes[index].field == order[index], "modes follow the shared page order");
  std::set<std::wstring_view> fields;
  for (auto const &entry : shuangpin_modes) {
    fields.insert(entry.field);
    check(entry.field != L"manual_tab", "manual_tab stays macOS only");
    check(!entry.title.empty() && !entry.description.empty(), "every mode has a title and an explanation");
  }
  check(fields.size() == shuangpin_modes.size(), "no mode is listed twice");

  // 缺字段时的读法必须等于 client-core 的 ShuangpinHelpcodeModes::default()：末尾直接、末尾分号、句中反引号和逐音节 / 结束开，其余关。
  const std::set<std::wstring_view> on_by_default = {L"direct", L"trigger_semicolon",
                                                     L"mid_sentence_backtick", L"direct_helpcode_slash"};
  for (auto const &entry : shuangpin_modes)
    check(entry.fallback == (on_by_default.count(entry.field) == 1), "fallback equals the client-core default");

  // 标题与共享页逐字相同（共享页在前面加「双拼」）。
  check(mode(L"direct").title == L"末尾直接辅助码", "direct title");
  check(mode(L"trigger_semicolon").title == L"末尾间接触发（分号）", "semicolon trigger title");
  check(mode(L"mid_sentence").title == L"句中间接辅助码", "mid-sentence title");
  check(mode(L"mid_sentence_uppercase").title == L"句中触发键（大写字母）", "mid-sentence uppercase title");
  check(mode(L"direct_helpcode").title == L"逐音节直接辅助码", "per-syllable title");
  check(mode(L"direct_helpcode_uppercase").title == L"逐音节直接（第二码大写结束）", "per-syllable uppercase title");

  // 置灰：辅助码关着时全部不可改；末尾间接三枚触发键要末尾直接和逐音节直接都关着，句中三枚触发键随句中间接，两种结束方式随逐音节直接，其余只看辅助码开关。参数依次是辅助码、末尾直接、句中间接、逐音节直接。
  for (auto const &entry : shuangpin_modes)
    check(!mode_enabled(entry, false, false, true, true), "every mode is greyed while helpcode is off");
  check(mode_enabled(mode(L"direct"), true, true, false, true), "direct only needs helpcode");
  check(mode_enabled(mode(L"mid_sentence"), true, true, false, false), "the mid-sentence switch only needs helpcode");
  check(mode_enabled(mode(L"direct_helpcode"), true, true, false, false), "the per-syllable switch only needs helpcode");
  // 引擎只在末尾直接和逐音节直接都关着时认末尾间接触发键（`apply_request_options`），出厂值的末尾直接开着，所以这三行出厂时置灰。
  for (auto const field : {L"trigger_semicolon", L"trigger_backtick", L"trigger_uppercase"}) {
    check(mode(field).parent == ModeParent::end_trigger, "end triggers hang off direct and direct_helpcode");
    check(!mode_enabled(mode(field), true, true, false, false), "end triggers are greyed while direct is on");
    check(!mode_enabled(mode(field), true, false, false, true), "end triggers are greyed while direct_helpcode is on");
    check(!mode_enabled(mode(field), true, true, false, true), "end triggers are greyed while both are on");
    check(mode_enabled(mode(field), true, false, false, false), "end triggers open when direct and direct_helpcode are off");
    check(mode_enabled(mode(field), true, false, true, false), "mid_sentence does not grey the end triggers");
    check(!mode_enabled(mode(field), false, false, false, false), "end triggers still follow helpcode");
  }
  check(mode(L"direct").description == L"完整音节后直接接辅助码；打开时末尾间接触发不生效",
        "the direct row says it turns the end triggers off");
  for (auto const field : {L"mid_sentence_backtick", L"mid_sentence_semicolon", L"mid_sentence_uppercase"}) {
    check(!mode_enabled(mode(field), true, false, false, true), "mid-sentence keys follow mid_sentence");
    check(mode_enabled(mode(field), true, true, true, false), "mid-sentence keys open with mid_sentence");
  }
  for (auto const field : {L"direct_helpcode_slash", L"direct_helpcode_uppercase"}) {
    check(!mode_enabled(mode(field), true, false, true, false), "terminators follow direct_helpcode");
    check(mode_enabled(mode(field), true, true, false, true), "terminators open with direct_helpcode");
  }

  check(strict_filter_description == L"关闭时一码只把匹配项前移；打开后排除辅助码不匹配的候选。两码本来就是严格筛选。",
        "strict filter explanation matches the shared page");

  if (failures)
    return 1;
  std::puts("helpcode modes model: order, defaults, titles and grey-out rules");
  return 0;
}
