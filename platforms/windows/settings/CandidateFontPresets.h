#pragma once
#include <algorithm>
#include <array>
#include <cstddef>
#include <cwctype>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

// 候选字体的预设、主字体与字体名的规则，逐条照搬共享设置的 `candidate/candidate-font-presets.ts`、`candidate-font-family.ts`、`font-catalog.ts` 和 `font-family-input.tsx`，平台固定为 Windows（共享代码里 `platform === "windows"`、`windows = true` 的那一支）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings {

// 偏好库的默认值（`preferences.rs` 的 default_candidate_font_family / default_candidate_fallback_fonts），以及 Windows 上英文字体未设置时显示的 Segoe UI。
inline constexpr std::wstring_view candidate_default_font_family = L"Noto Sans SC";
inline constexpr std::array<std::wstring_view, 2> candidate_default_fallback_fonts{
    {L"Noto Sans SC", L"Microsoft YaHei"}};
inline constexpr std::wstring_view candidate_default_english_font = L"Segoe UI";
// 回退字体最多 32 个，与偏好库的校验相同。
inline constexpr std::size_t candidate_max_fallback_fonts = 32;
// 字体列表最多 16384 项（`normalizeFontCatalog`），输入框的下拉最多列 100 项。
inline constexpr std::size_t font_catalog_limit = 16384;
inline constexpr std::size_t font_menu_limit = 100;

struct CandidateFontFamilies {
  std::wstring family;
  std::vector<std::wstring> fallback;
};

struct CandidateFontPresetFamily {
  std::wstring_view family;
  // Windows 自带这个字体。共享预设按平台标注，这里只需要知道是不是 Windows 的。
  bool windows = false;
};

struct CandidateFontPreset {
  std::wstring_view id;
  std::wstring_view label;
  std::array<CandidateFontPresetFamily, 4> families;
  std::size_t count = 0;
  // Windows 没有这种字体，预设在这里不可选。
  bool unavailable = false;
};

// 预设的顺序和每个预设里字体的顺序都与共享的 `candidateFontPresets` 相同。
inline constexpr std::array<CandidateFontPreset, 5> candidate_font_presets{{
    {L"default", L"默认", {{{candidate_default_font_family, false}}}, 1, false},
    {L"song",
     L"宋体",
     {{{L"Songti SC", false}, {L"SimSun", true}, {L"Noto Serif CJK SC", false}, {L"Noto Serif SC", false}}},
     4,
     false},
    {L"hei",
     L"黑体",
     {{{L"PingFang SC", false},
       {L"Microsoft YaHei", true},
       {L"Noto Sans CJK SC", false},
       {L"Noto Sans SC", false}}},
     4,
     false},
    {L"kai",
     L"楷体",
     {{{L"Kaiti SC", false}, {L"KaiTi", true}, {L"STKaiti", false}, {L"AR PL UKai CN", false}}},
     4,
     false},
    // Windows 没有自带圆体，预设在这里禁用，而不是悄悄画成黑体。
    {L"yuan", L"圆体", {{{L"Yuanti SC", false}, {L"Noto Sans SC", false}}}, 2, true},
}};

inline CandidateFontPreset const &candidate_font_preset(std::wstring_view id) {
  for (auto const &entry : candidate_font_presets)
    if (entry.id == id)
      return entry;
  return candidate_font_presets[0];
}

// 预设的字体，Windows 自带的排在前面，其余保持预设里的顺序（`orderedFamilies`）。
inline std::vector<std::wstring> candidate_font_preset_families(CandidateFontPreset const &entry) {
  std::vector<std::wstring> result;
  for (std::size_t i = 0; i < entry.count; ++i)
    if (entry.families[i].windows)
      result.emplace_back(entry.families[i].family);
  for (std::size_t i = 0; i < entry.count; ++i)
    if (!entry.families[i].windows)
      result.emplace_back(entry.families[i].family);
  return result;
}

inline bool candidate_font_preset_available(std::wstring_view id) {
  return !candidate_font_preset(id).unavailable;
}

inline std::vector<std::wstring> candidate_default_fallback_list() {
  return {candidate_default_fallback_fonts.begin(), candidate_default_fallback_fonts.end()};
}

// 选中预设后要写的两项（`candidateFontPresetPatch`）：Windows 自带的那个字体成为主字体，预设的字体排在回退列表最前，后面接用户原来的列表并去掉重复；「默认」恢复偏好库的默认值。
inline CandidateFontFamilies candidate_font_preset_patch(std::wstring_view id,
                                                         std::vector<std::wstring> const &current_fallback) {
  if (id == L"default")
    return {std::wstring(candidate_default_font_family), candidate_default_fallback_list()};
  auto families = candidate_font_preset_families(candidate_font_preset(id));
  CandidateFontFamilies patch{families.front(), families};
  for (auto const &name : current_fallback)
    if (std::find(families.begin(), families.end(), name) == families.end())
      patch.fallback.push_back(name);
  if (patch.fallback.size() > candidate_max_fallback_fonts)
    patch.fallback.resize(candidate_max_fallback_fonts);
  return patch;
}

// 选用主字体后要写的两项（`candidateMainFontPatch`，Windows 一支）：Windows 的候选窗不读 candidate_font_family，汉字从回退列表取，所以主字体同时排到回退列表最前。先去掉原来的主字体，因为输入框每敲一个字都会写一次，半截的名字不能留在回退列表里。
inline CandidateFontFamilies candidate_main_font_patch(std::wstring_view family,
                                                       std::wstring_view current_family,
                                                       std::vector<std::wstring> const &current_fallback) {
  CandidateFontFamilies patch{std::wstring(family), {std::wstring(family)}};
  for (auto const &name : current_fallback)
    if (name != current_family && name != family)
      patch.fallback.push_back(name);
  if (patch.fallback.size() > candidate_max_fallback_fonts)
    patch.fallback.resize(candidate_max_fallback_fonts);
  return patch;
}

// 当前偏好对应的预设，用户自己选的字体返回空（`currentCandidateFontPreset`）。主字体决定预设；两个预设都含这个字体时，回退列表以该预设的字体开头的那个胜出，否则是「默认」或者先列出的那个。
inline std::optional<std::wstring_view> current_candidate_font_preset(
    std::wstring_view family, std::vector<std::wstring> const &fallback) {
  std::vector<CandidateFontPreset const *> matches;
  for (auto const &entry : candidate_font_presets)
    for (std::size_t i = 0; i < entry.count; ++i)
      if (entry.families[i].family == family) {
        matches.push_back(&entry);
        break;
      }
  if (matches.empty())
    return std::nullopt;
  if (matches.size() == 1)
    return matches.front()->id;
  for (auto const *entry : matches) {
    const auto families = entry->id == L"default" ? candidate_default_fallback_list()
                                                  : candidate_font_preset_families(*entry);
    bool leading = families.front() == family;
    for (std::size_t i = 0; leading && i < families.size(); ++i)
      leading = i < fallback.size() && fallback[i] == families[i];
    if (leading)
      return entry->id;
  }
  return family == candidate_default_font_family ? std::wstring_view(L"default") : matches.front()->id;
}

// 字体名按 UTF-8 计的字节数，与浏览器的 TextEncoder 相同：落单的代理项按 U+FFFD 计 3 字节。
inline std::size_t font_name_utf8_bytes(std::wstring_view value) {
  std::size_t bytes = 0;
  for (std::size_t i = 0; i < value.size(); ++i) {
    const auto unit = static_cast<char32_t>(value[i]);
    if (unit < 0x80)
      bytes += 1;
    else if (unit < 0x800)
      bytes += 2;
    else if (unit >= 0xD800 && unit <= 0xDBFF && i + 1 < value.size() &&
             value[i + 1] >= 0xDC00 && value[i + 1] <= 0xDFFF) {
      bytes += 4;
      ++i;
    } else
      bytes += 3;
  }
  return bytes;
}

// 合法的字体名（`validFontFamily`）：非空，UTF-8 不超过 128 字节，不含 U+0000–U+001F、U+007F–U+009F 的控制字符。偏好库保存时按同样的规则拒绝。
inline bool valid_font_family(std::wstring_view value) {
  if (value.empty() || font_name_utf8_bytes(value) > 128)
    return false;
  return std::none_of(value.begin(), value.end(), [](wchar_t c) {
    return c <= 0x1F || (c >= 0x7F && c <= 0x9F);
  });
}

// 宿主返回的字体列表（`normalizeFontCatalog`）：超过上限或含不合法的名字时整份作废，否则去重后用 `before` 排序（共享页用 localeCompare）。
template <class Before>
std::optional<std::vector<std::wstring>> normalize_font_catalog(std::vector<std::wstring> fonts,
                                                                Before before) {
  if (fonts.size() > font_catalog_limit ||
      !std::all_of(fonts.begin(), fonts.end(),
                   [](std::wstring const &name) { return valid_font_family(name); }))
    return std::nullopt;
  std::sort(fonts.begin(), fonts.end());
  fonts.erase(std::unique(fonts.begin(), fonts.end()), fonts.end());
  std::stable_sort(fonts.begin(), fonts.end(), before);
  return fonts;
}

inline std::wstring font_name_lowercase(std::wstring_view value) {
  std::wstring result(value);
  for (auto &c : result)
    c = static_cast<wchar_t>(std::towlower(static_cast<wint_t>(c)));
  return result;
}

struct FontMenu {
  std::vector<std::wstring> options;
  // 匹配的总数，可能多于列出的 100 项。
  std::size_t matches = 0;
};

// 输入框下拉列出的字体（`FontFamilyInput`）：用户还没打字时列出全部，打字后只列名字里含有输入内容（不分大小写）的，最多 100 项。`lower` 转小写；共享页用 toLocaleLowerCase，设置窗口传入按用户区域设置转换的函数，不传时用 C 运行库的 towlower，默认区域下只转 ASCII 字母。
template <class Lower>
FontMenu font_menu(std::vector<std::wstring> const &fonts, std::wstring_view value, bool search,
                   Lower lower) {
  FontMenu menu;
  const auto needle = search ? lower(value) : std::wstring();
  for (auto const &font : fonts) {
    if (search && lower(font).find(needle) == std::wstring::npos)
      continue;
    ++menu.matches;
    if (menu.options.size() < font_menu_limit)
      menu.options.push_back(font);
  }
  return menu;
}

inline FontMenu font_menu(std::vector<std::wstring> const &fonts, std::wstring_view value, bool search) {
  return font_menu(fonts, value, search, font_name_lowercase);
}

} // namespace msime::settings
