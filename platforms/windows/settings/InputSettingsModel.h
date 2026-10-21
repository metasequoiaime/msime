#pragma once
#include <array>
#include <string>
#include <string_view>

// 「输入」「快捷键」两页里与共享设置页逐字相同的列表、文案和判断：输入页的六个页内标签（`packages/ui/src/settings/input-tabs.ts`）、五笔选词键的校验（`wubi-preferences.ts` 的 validWubiCandidateKeys）、中文输出的说明（`traditional-chinese-output-section.tsx`）、逐个模式的各行（`local-mode-options.ts`、`local-mode-preferences.ts`）、模糊音的四组规则（`fuzzy-pinyin-preferences.ts`）和按键速查里的翻页键（`candidate-paging-options.ts` 的 pagingKeys）。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::input {

struct Tab {
  std::wstring_view id;
  std::wstring_view title;
};

// 输入页的页内标签，顺序按「基础 → 进阶」排，与共享页 `inputTabs` 相同。
inline constexpr std::array<Tab, 6> tabs{{
    {L"scheme", L"输入方案"},
    {L"mode", L"中英模式"},
    {L"selection", L"选词与翻页"},
    {L"candidates", L"候选与联想"},
    {L"output", L"输出设置"},
    {L"advanced", L"高级选项"},
}};

// 快捷键页的页内标签（共享页 `shortcutTabs`）。
inline constexpr std::array<Tab, 2> shortcut_tabs{{
    {L"settings", L"按键设置"},
    {L"reference", L"按键速查"},
}};

// 标点与翻译页的页内标签（共享页 `expressionTabs`）。
inline constexpr std::array<Tab, 3> expression_tabs{{
    {L"punctuation", L"标点设置"},
    {L"gloss", L"候选释义"},
    {L"providers", L"翻译服务"},
}};

// 五笔选词键的一个键：一个英文标点，即 ASCII 的 ! 到 /、: 到 @、[ 到 `、{ 到 ~（共享页的正则 `^[!-/:-@[-`{-~]$`）。
inline bool wubi_candidate_key(std::wstring_view key) {
  if (key.size() != 1)
    return false;
  const wchar_t c = key[0];
  return (c >= L'!' && c <= L'/') || (c >= L':' && c <= L'@') || (c >= L'[' && c <= L'`') ||
         (c >= L'{' && c <= L'~');
}

// 两个键各是一个英文标点并且不同；与翻页、以词定字的冲突由共享偏好库在保存时检查。
inline bool valid_wubi_candidate_keys(std::wstring_view second, std::wstring_view third) {
  return wubi_candidate_key(second) && wubi_candidate_key(third) && second != third;
}

// 「自定义选词键」一行的说明：键有效时说怎么选，无效时换成提示。
inline std::wstring_view wubi_candidate_keys_description(bool valid) {
  return valid ? L"使用两个不同的英文标点；请选择未用于翻页或以词定字的键。"
               : L"请输入两个不同的单个英文标点。";
}

// 「中文输出」一行的说明，随选项和方案变：粤拼、注音直接输出繁体，笔画按所选的字原样输出，都不受这个选项影响。Windows 不认繁体变体（`HostCapabilities.traditional_chinese_variants` 只在 Linux 为真），只有简体和标准繁体两项。
inline std::wstring chinese_output_description(bool traditional, std::wstring_view scheme) {
  std::wstring description = traditional
                                 ? L"将提交的简体中文转换为繁体中文（OpenCC 标准繁体）"
                                 : L"按简体中文提交";
  if (scheme == L"cantonese" || scheme == L"zhuyin")
    description += L"。粤拼与注音直接输出繁体，此选项不影响它们";
  else if (scheme == L"stroke")
    description += L"。笔画按所选的字原样输出，此选项不影响它";
  return description;
}

struct LocalMode {
  // `local_modes.<id>` 偏好。
  std::wstring_view id;
  std::wstring_view title;
  std::wstring_view description;
  // 文档里没有这一项时的值：V、/、@、地名和拆字反查在桌面默认关。
  bool default_on;
  // 从 Shift+字母进入：总开关关掉时这一行变灰，值保留。`/` 和 `@` 不经过它。
  bool shift_letter;
};

// 逐个模式的各行，标题和说明与共享页 `localModeRows`、`triggerModeRows`、`mentionPlacesRow` 相同，顺序也相同。Windows 是路由 V、/、@ 的宿主（`plugin_triggers`），也能编辑 @ 名单（插件页），所以三行和「@ 地名」都列出。
inline constexpr std::array<LocalMode, 13> local_modes{{
    {L"quick_phrase", L"快捷短语(K 模式)", L"中文模式下按 Shift+K，再输入编码即可调用快捷短语", true, true},
    {L"date_time", L"日期与时间快捷输入(T 模式)",
     L"中文模式下按 Shift+T，再输入 rq / riqi / date 输入日期，sj / shijian / time 输入时间，xq / xingqi / week 输入星期，nl / nongli / yinli 输入农历。打开时，拼音（含九键）直接输入 riqi / rq、shijian / sj、xingqi / xq、nongli / nl，日期、时间、星期、农历也会排在对应的词后面",
     true, true},
    {L"unicode", L"Unicode 便捷录入(U 模式)",
     L"中文模式下按 Shift+U，再输入十六进制码位（如 4e00 / +1f600）。空格上屏；Shift+数字选词", true, true},
    {L"emoji", L"Emoji 快捷输入(E 模式)",
     L"中文模式下按 Shift+E，再输入全拼 / 简拼 / 双拼 / 英文关键词。空格上屏；数字选词", true, true},
    {L"kaomoji", L"颜文字快捷输入(M 模式)",
     L"中文模式下按 Shift+M，再输入全拼 / 简拼 / 双拼 / 英文关键词。空格上屏；数字选词", true, true},
    {L"super_jianpin", L"超级简拼(J 模式)",
     L"中文模式下按 Shift+J，每个字母作为简拼；双拼按当前方案转换声母。空格上屏；数字选词", true, true},
    {L"temporary_english", L"临时英文(Y 模式)",
     L"中文模式下按 Shift+Y，之后按英文处理。空格上屏当前输入；数字选词；上屏后回到中文", true, true},
    {L"temporary_japanese", L"临时日语(R 模式)",
     L"中文模式下按 Shift+R，之后按日语罗马字处理。空格上屏首选；数字选词；上屏后回到中文", true, true},
    {L"chaizi", L"拆字反查(C 模式)",
     L"中文模式下按 Shift+C，再输入字的各个部件的拼音，例如 huohuohuo 得 焱、nvzi 得 好。部件之间可以用 ' 分隔，ü 用 v。空格上屏；数字选词",
     false, true},
    {L"expression", L"计算与数字(V 模式)",
     L"中文模式下按 Shift+V，再输入算式（如 1+2*3）、数字（123 可转为一百二十三、壹佰贰拾叁或金额）或日期（2026.10.1）。空格上屏；Shift+数字选词；上屏内容不参与学习和打字统计",
     false, true},
    {L"command", L"指令(/ 模式)",
     L"中文标点下没有输入时按 /，再输入指令字母：rq 日期、sj 时间、xq 星期、nl 农历、fy 翻译（fy 后输入英文，' 分隔单词，首选为中文译文），以及「插件」页启用的指令表。空格上屏；数字选词",
     false, false},
    {L"mention", L"@ 名字与地点(@ 模式)",
     L"中文标点下没有输入时按 @，再输入拼音或首字母，从「插件」页的 @ 名单中选择。空格上屏；数字选词",
     false, false},
    {L"mention_places", L"@ 地名",
     L"@ 模式在名单之后列出全国省、市、区县，候选旁注明所属省市。地名表内置在输入法中，不联网、不读取位置",
     false, false},
}};

// 没有选翻译服务时，「指令(/ 模式)」的说明后面补的一句（共享页 `commandTranslationNotice`）。
inline constexpr std::wstring_view command_translation_notice =
    L"fy 翻译需要先在「标点与翻译 → 候选词翻译」选择翻译服务，目前未选择，fy 不会出结果。";

// 某个模式这一行的说明：`translation_service` 为假时指令模式补上上面那句。
inline std::wstring local_mode_description(LocalMode const &mode, bool translation_service) {
  std::wstring description(mode.description);
  if (mode.id == L"command" && !translation_service)
    description += L"。" + std::wstring(command_translation_notice);
  return description;
}

// 这一行变不变灰（共享页 localModeControlState）：「@ 地名」在 @ 模式关着时变灰；Shift+字母进入的模式在总开关关着时变灰。值都不动。
inline bool local_mode_disabled(LocalMode const &mode, bool shift_letter_entry, bool mention) {
  return (mode.id == L"mention_places" && !mention) || (!shift_letter_entry && mode.shift_letter);
}

struct FuzzyRule {
  // 规则所在的小标题。
  std::wstring_view group;
  // `fuzzy_pinyin.rules` 里的值。
  std::wstring_view id;
  std::wstring_view label;
};

// 模糊音规则按四个小标题分组，顺序与共享页 `fuzzyPinyinGroups` 相同。
inline constexpr std::array<FuzzyRule, 11> fuzzy_rules{{
    {L"平翘舌", L"z-zh", L"z ↔ zh"},
    {L"平翘舌", L"c-ch", L"c ↔ ch"},
    {L"平翘舌", L"s-sh", L"s ↔ sh"},
    {L"声母", L"n-l", L"n ↔ l"},
    {L"声母", L"f-h", L"f ↔ h"},
    {L"声母", L"r-l", L"r ↔ l"},
    {L"前后鼻音", L"an-ang", L"an ↔ ang"},
    {L"前后鼻音", L"en-eng", L"en ↔ eng"},
    {L"前后鼻音", L"in-ing", L"in ↔ ing"},
    {L"其他韵母", L"ian-iang", L"ian ↔ iang"},
    {L"其他韵母", L"uan-uang", L"uan ↔ uang"},
}};

struct PagingKeys {
  // `navigation.<id>` 偏好。
  std::wstring_view id;
  std::wstring_view chord;
};

// 按键速查「向前 / 向后翻页」一行列出的几组键，顺序与输入页的翻页方式相同。Windows 上 Tab 和 PageUp / PageDown 两组固定开启（`HostCapabilities.tab_page_paging_switches` 为假），速查按实际生效的样子列出它们，不看文档里存的值（共享页 effectiveNavigation）。
inline constexpr std::array<PagingKeys, 5> paging_keys{{
    {L"minus_equal", L"- / ="},
    {L"comma_period", L", / ."},
    {L"brackets", L"[ / ]"},
    {L"tab", L"Shift+Tab / Tab"},
    {L"page_up_down", L"Page Up / Page Down"},
}};

// 这组翻页键在速查里算不算开着：Tab 和 PageUp / PageDown 总是开，其余看偏好。
inline bool paging_keys_on(PagingKeys const &keys, bool stored) {
  return keys.id == L"tab" || keys.id == L"page_up_down" || stored;
}

} // namespace msime::settings::input
