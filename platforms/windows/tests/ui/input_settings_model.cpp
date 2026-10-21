#include "../../settings/InputSettingsModel.h"
#include <cstdio>
#include <set>
#include <string>

using namespace msime::settings::input;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

LocalMode const &mode(std::wstring_view id) {
  for (auto const &entry : local_modes)
    if (entry.id == id)
      return entry;
  std::fprintf(stderr, "FAIL: missing local mode\n");
  ++failures;
  return local_modes.front();
}
} // namespace

int main() {
  // 共享页 inputTabs、shortcutTabs、expressionTabs 的顺序和文字。
  check(tabs[0].id == L"scheme" && tabs[0].title == L"输入方案" && tabs[1].id == L"mode" &&
            tabs[1].title == L"中英模式" && tabs[2].id == L"selection" &&
            tabs[2].title == L"选词与翻页" && tabs[3].id == L"candidates" &&
            tabs[3].title == L"候选与联想" && tabs[4].id == L"output" &&
            tabs[4].title == L"输出设置" && tabs[5].id == L"advanced" &&
            tabs[5].title == L"高级选项",
        "the six input tabs of the shared page");
  check(shortcut_tabs[0].title == L"按键设置" && shortcut_tabs[1].title == L"按键速查",
        "the two shortcut tabs");
  check(expression_tabs[0].title == L"标点设置" && expression_tabs[1].title == L"候选释义" &&
            expression_tabs[2].title == L"翻译服务",
        "the three punctuation and translation tabs");

  // 五笔选词键：各是一个英文标点，并且不同。
  check(valid_wubi_candidate_keys(L";", L"'"), "the default pair is valid");
  check(valid_wubi_candidate_keys(L"'", L";"), "a swapped pair is valid");
  check(valid_wubi_candidate_keys(L"[", L"~"), "brackets and tilde are punctuation");
  check(!valid_wubi_candidate_keys(L";", L";"), "the same key twice is invalid");
  check(!valid_wubi_candidate_keys(L"", L"'"), "an empty key is invalid");
  check(!valid_wubi_candidate_keys(L"a", L"'"), "a letter is invalid");
  check(!valid_wubi_candidate_keys(L"1", L"'"), "a digit is invalid");
  check(!valid_wubi_candidate_keys(L" ", L"'"), "a space is invalid");
  check(!valid_wubi_candidate_keys(L"；", L"'"), "a full-width punctuation mark is invalid");
  check(!valid_wubi_candidate_keys(L";;", L"'"), "two characters are invalid");
  check(wubi_candidate_keys_description(true) ==
            L"使用两个不同的英文标点；请选择未用于翻页或以词定字的键。",
        "the description of a valid pair");
  check(wubi_candidate_keys_description(false) == L"请输入两个不同的单个英文标点。",
        "the description of an invalid pair");

  // 中文输出的说明随选项和方案变。
  check(chinese_output_description(false, L"quanpin") == L"按简体中文提交", "simplified");
  check(chinese_output_description(true, L"quanpin") ==
            L"将提交的简体中文转换为繁体中文（OpenCC 标准繁体）",
        "traditional");
  check(chinese_output_description(true, L"cantonese") ==
            L"将提交的简体中文转换为繁体中文（OpenCC 标准繁体）。粤拼与注音直接输出繁体，此选项不影响它们",
        "Cantonese is not converted");
  check(chinese_output_description(false, L"zhuyin") ==
            L"按简体中文提交。粤拼与注音直接输出繁体，此选项不影响它们",
        "Zhuyin is not converted");
  check(chinese_output_description(false, L"stroke") ==
            L"按简体中文提交。笔画按所选的字原样输出，此选项不影响它",
        "stroke is not converted");

  // 逐个模式：顺序、标题和默认值与共享页相同，ID 不重复。
  check(local_modes.front().id == L"quick_phrase" && local_modes.back().id == L"mention_places",
        "local modes start with K and end with places");
  std::set<std::wstring_view> ids;
  for (auto const &entry : local_modes)
    ids.insert(entry.id);
  check(ids.size() == local_modes.size(), "local mode ids are unique");
  check(mode(L"quick_phrase").title == L"快捷短语(K 模式)" && mode(L"quick_phrase").default_on,
        "K mode");
  check(!mode(L"chaizi").default_on && !mode(L"expression").default_on &&
            !mode(L"command").default_on && !mode(L"mention").default_on &&
            !mode(L"mention_places").default_on,
        "C, V, / and @ default to off on the desktop");
  check(mode(L"mention_places").title == L"@ 地名", "the places row");

  // 总开关关着时 Shift+字母进入的各行变灰，/ 和 @ 不受影响；地名跟着 @ 模式。
  check(local_mode_disabled(mode(L"emoji"), false, true), "E mode greys without Shift+letter");
  check(local_mode_disabled(mode(L"expression"), false, true), "V mode greys without Shift+letter");
  check(!local_mode_disabled(mode(L"emoji"), true, false), "E mode is available with Shift+letter");
  check(!local_mode_disabled(mode(L"command"), false, false), "/ mode ignores Shift+letter");
  check(!local_mode_disabled(mode(L"mention"), false, false), "@ mode ignores Shift+letter");
  check(local_mode_disabled(mode(L"mention_places"), true, false), "places grey without @");
  check(!local_mode_disabled(mode(L"mention_places"), false, true), "places follow @ only");

  // 没有翻译服务时指令模式补一句。
  check(local_mode_description(mode(L"command"), true) == mode(L"command").description,
        "the command row with a translation service");
  check(local_mode_description(mode(L"command"), false) ==
            std::wstring(mode(L"command").description) + L"。" +
                std::wstring(command_translation_notice),
        "the command row without a translation service");
  check(local_mode_description(mode(L"emoji"), false) == mode(L"emoji").description,
        "other rows do not get the notice");

  // 模糊音：四组十一条，组按顺序连续排列。
  check(fuzzy_rules.front().group == L"平翘舌" && fuzzy_rules.back().group == L"其他韵母",
        "fuzzy groups run from 平翘舌 to 其他韵母");
  std::set<std::wstring_view> groups;
  std::wstring_view last;
  for (auto const &rule : fuzzy_rules) {
    if (rule.group != last) {
      check(groups.insert(rule.group).second, "a fuzzy group appears in one run");
      last = rule.group;
    }
  }
  check(groups.size() == 4, "four fuzzy groups");

  // 按键速查：Tab 和 PageUp / PageDown 总算开着，其余看偏好。
  check(paging_keys_on(paging_keys[3], false) && paging_keys_on(paging_keys[4], false),
        "Tab and PageUp / PageDown are always on");
  check(!paging_keys_on(paging_keys[2], false) && paging_keys_on(paging_keys[2], true),
        "brackets follow the preference");

  if (failures)
    return 1;
  std::puts("input settings model: tabs, Wubi keys, Chinese output, local modes, fuzzy groups and paging keys");
  return 0;
}
