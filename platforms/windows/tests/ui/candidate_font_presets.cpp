#include "../../settings/CandidateFontPresets.h"
#include <cstdio>
#include <string>
#include <vector>

using namespace msime::settings;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}
using List = std::vector<std::wstring>;
const List defaults{L"Noto Sans SC", L"Microsoft YaHei"};
bool by_code_unit(std::wstring const &a, std::wstring const &b) { return a < b; }
} // namespace

int main() {
  check(candidate_font_presets.size() == 5 && candidate_font_presets[0].label == L"默认" &&
            candidate_font_presets[4].label == L"圆体",
        "presets in the shared order");
  check(!candidate_font_preset_available(L"yuan"), "圆体 is disabled on Windows");
  check(candidate_font_preset_available(L"kai"), "楷体 is available on Windows");

  const auto song = candidate_font_preset_patch(L"song", defaults);
  check(song.family == L"SimSun", "宋体 takes the Windows face as the main font");
  check(song.fallback == List{L"SimSun", L"Songti SC", L"Noto Serif CJK SC", L"Noto Serif SC",
                              L"Noto Sans SC", L"Microsoft YaHei"},
        "宋体 leads the fallback list, the user's list follows");
  const auto hei = candidate_font_preset_patch(L"hei", defaults);
  check(hei.fallback == List{L"Microsoft YaHei", L"PingFang SC", L"Noto Sans CJK SC",
                             L"Noto Sans SC"},
        "黑体 drops the families it already names");
  const auto restored = candidate_font_preset_patch(L"default", song.fallback);
  check(restored.family == L"Noto Sans SC" && restored.fallback == defaults,
        "默认 restores the core defaults");
  List long_list;
  for (int i = 0; i < 40; ++i)
    long_list.push_back(L"Font " + std::to_wstring(i));
  check(candidate_font_preset_patch(L"kai", long_list).fallback.size() == 32,
        "the fallback list keeps at most 32 families");

  check(current_candidate_font_preset(L"Noto Sans SC", defaults) == std::wstring_view(L"default"),
        "the defaults read as 默认");
  check(current_candidate_font_preset(song.family, song.fallback) == std::wstring_view(L"song"),
        "a preset reads back as itself");
  check(current_candidate_font_preset(hei.family, hei.fallback) == std::wstring_view(L"hei"),
        "黑体 reads back as 黑体");
  check(current_candidate_font_preset(L"Noto Sans SC",
                                      candidate_font_preset_patch(L"yuan", defaults).fallback) ==
            std::wstring_view(L"default"),
        "圆体's list on Windows does not lead with Yuanti SC, so its main font reads as 默认");
  check(current_candidate_font_preset(L"Yuanti SC", defaults) == std::wstring_view(L"yuan"),
        "a font only one preset names reads as that preset");
  check(!current_candidate_font_preset(L"Consolas", defaults), "a font of the user's own");

  const auto main = candidate_main_font_patch(L"Consolas", L"Noto Sans SC", defaults);
  check(main.family == L"Consolas" && main.fallback == List{L"Consolas", L"Microsoft YaHei"},
        "the main font replaces the previous one at the head of the fallback list");
  const auto typed = candidate_main_font_patch(L"Consol", L"Conso", List{L"Conso", L"Microsoft YaHei"});
  check(typed.fallback == List{L"Consol", L"Microsoft YaHei"},
        "partial names typed on the way do not pile up");
  check(candidate_main_font_patch(L"A", L"B", long_list).fallback.size() == 32,
        "the main font keeps the 32-family bound");

  check(valid_font_family(L"微软雅黑"), "a localized family is valid");
  check(!valid_font_family(L""), "an empty name is invalid");
  check(!valid_font_family(L"a\tb") && !valid_font_family(std::wstring(L"a\x85")),
        "control characters are invalid");
  check(valid_font_family(std::wstring(128, L'a')) && !valid_font_family(std::wstring(129, L'a')),
        "128 ASCII bytes fit");
  check(valid_font_family(std::wstring(42, L'字')) && !valid_font_family(std::wstring(43, L'字')),
        "CJK counts three bytes a character");
  check(font_name_utf8_bytes(L"\xD83D\xDE00") == 4 && font_name_utf8_bytes(L"\xD83D") == 3,
        "surrogate pairs and lone surrogates count as the browser does");

  const auto catalog = normalize_font_catalog({L"b", L"a", L"b"}, by_code_unit);
  check(catalog && *catalog == List{L"a", L"b"}, "the catalog is sorted without repeats");
  check(!normalize_font_catalog({L"ok", L""}, by_code_unit), "an invalid name voids the catalog");
  check(!normalize_font_catalog(List(font_catalog_limit + 1, L"x"), by_code_unit),
        "an oversized catalog is refused");

  List fonts;
  for (int i = 0; i < 150; ++i)
    fonts.push_back(L"Font " + std::to_wstring(i));
  fonts.push_back(L"KaiTi");
  const auto all = font_menu(fonts, L"KaiTi", false);
  check(all.options.size() == 100 && all.matches == 151, "before typing every font is listed, 100 shown");
  const auto searched = font_menu(fonts, L"kait", true);
  check(searched.options == List{L"KaiTi"} && searched.matches == 1, "typing filters case-insensitively");
  check(font_menu(fonts, L"zzz", true).matches == 0, "no match");
  // The window passes a locale-aware lowercase; the menu matches through whatever it is given.
  const auto folded = font_menu(List{L"École", L"Arial"}, L"éc", true, [](std::wstring_view value) {
    std::wstring result(value);
    for (auto &c : result)
      if (c == L'É')
        c = L'é';
    return result;
  });
  check(folded.options == List{L"École"}, "the menu filters through the given lowercase");

  if (failures == 0)
    std::puts("candidate font presets: ok");
  return failures == 0 ? 0 : 1;
}
