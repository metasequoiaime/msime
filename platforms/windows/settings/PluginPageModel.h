#pragma once
#include <algorithm>
#include <array>
#include <charconv>
#include <cstddef>
#include <cwchar>
#include <cwctype>
#include <optional>
#include <set>
#include <string>
#include <string_view>
#include <vector>

// 「插件」页「我的插件」的规则和文案，照共享设置在 Windows 上的样子逐条移植：`plugin-preferences.ts`（选用和删除后的偏好）、`plugin-catalog-helpers.ts`（类型名、标记、缺失的选择、出错说明）、`plugin-library-view.tsx`（macOS 的插件库）的卡片说明、搜索与类型筛选、`plugin-sound-effects-view.tsx` 的包名、`plugin-detail-view.tsx` 的按钮与说明，`plugin-mentions-view.tsx` 的名单校验，以及 `plugin-script-model.ts` 的脚本展示规则、`script-grant-dialog.tsx` 与 `plugin-script-view.tsx` 的脚本插件授权页。Windows 宿主的能力（`host_surface.rs`）全部为真：播放音效和音乐、路由 / 与 @、画特效样式和特效包、背单词列单词本、符号面板列符号集；特效不是只闪光的那种（那是 HarmonyOS）。只有辅助码随版本：调用方传入。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::plugins {

// ---- 包 ----

// 我的插件列表给已安装的包分组的顺序（`kindOrder`）。
inline constexpr std::array<std::wstring_view, 8> kind_order{
    L"sound", L"effect", L"music", L"command_table",
    L"phrase_table", L"helpcode", L"wordbook", L"symbol_set"};

// 各类插件统一的显示名（`kindLabels`）；不认识的类型原样显示。
inline std::wstring kind_label(std::wstring_view kind) {
  if (kind == L"sound")
    return L"音效包";
  if (kind == L"music")
    return L"音乐包";
  if (kind == L"command_table")
    return L"指令表";
  if (kind == L"effect")
    return L"特效包";
  if (kind == L"phrase_table")
    return L"短语表";
  if (kind == L"helpcode")
    return L"辅助码表";
  if (kind == L"wordbook")
    return L"单词本";
  if (kind == L"symbol_set")
    return L"符号集";
  if (kind == L"script")
    return L"脚本插件";
  return std::wstring(kind);
}

struct Command {
  std::wstring trigger, title, template_text;
};

struct Phrase {
  std::wstring key, text;
};

struct HelpcodeEntry {
  std::wstring character, code;
};

struct SymbolGroup {
  std::wstring tab, title, keywords;
  std::vector<std::wstring> items;
};

// `client-services::plugins::scan` 列出的一个包：清单的字段，加上它那一类的内容（`PluginPackage`）。没有的数字是 nullopt，没有的文字是空串。
struct Package {
  std::wstring id, name, version, license, author, description;
  bool builtin = false;
  std::wstring kind;
  // 音效包："keys" 或 "sequence"；其他类型为空。
  std::wstring mode;
  std::vector<std::wstring> tracks;
  std::vector<Command> commands;
  std::vector<Phrase> phrases;
  std::wstring table;
  std::optional<double> entries;
  std::vector<HelpcodeEntry> preview;
  std::wstring file;
  std::optional<double> word_count;
  std::vector<std::wstring> first_words;
  std::vector<SymbolGroup> groups;
  std::wstring style;
  std::optional<double> intensity;
  std::vector<std::wstring> colors;
  std::optional<double> duration_ms;
  std::optional<double> particles;
  // 脚本插件（`script_pack::ScriptPack`）：声明的钩子、申请的能力和内容哈希。授权绑定在内容哈希上。
  std::vector<std::wstring> hooks;
  std::vector<std::wstring> permissions;
  std::wstring content_hash;
};

// 插件目录下不能载入的文件夹和原因（`PluginIssue`）。
struct Issue {
  std::wstring kind, folder, reason;
};

struct Catalog {
  std::vector<Package> packages;
  std::vector<Issue> issues;
};

// 按键旋律（`mode = "sequence"` 的音效包）显示为「音效包·旋律」，其余用类型名（`packKindLabel`）。
inline std::wstring pack_kind_label(Package const &pack) {
  return pack.kind == L"sound" && pack.mode == L"sequence" ? L"音效包·旋律" : kind_label(pack.kind);
}

inline std::wstring pack_label(Package const &pack) {
  return pack.builtin ? pack.name + L"（内置）" : pack.name;
}

// JavaScript 把数字写成字符串的样子：整数不带小数点，其他取最短的十进制写法。
inline std::wstring js_number(double value) {
  if (value == 0)
    return L"0";
  char buffer[64];
  const auto result = std::to_chars(buffer, buffer + sizeof(buffer), value);
  return std::wstring(buffer, result.ptr);
}

// ---- 偏好 ----

// `client-core::preferences::PluginPreferences`，每一项都已用默认值补齐（`pluginPreferences`）。
struct Preferences {
  bool key_sound_enabled = false;
  std::wstring key_sound_mode = L"keys";
  std::wstring key_sound_pack = L"default";
  double key_sound_volume = 50;
  bool commit_sound_enabled = false;
  std::wstring melody_pack = L"twinkle";
  bool music_enabled = false;
  std::wstring music_pack;
  double music_volume = 30;
  bool achievements_enabled = false;
  std::vector<std::wstring> command_tables;
  std::wstring effect_style = L"off";
  double effect_intensity = 50;
  std::wstring effect_pack;
  bool combo_counter = false;
  bool combo_tier_sound = false;
  std::vector<std::wstring> phrase_tables;
  std::wstring helpcode_pack_quanpin;
  std::wstring helpcode_pack_shuangpin;

  bool operator==(Preferences const &other) const {
    return key_sound_enabled == other.key_sound_enabled && key_sound_mode == other.key_sound_mode &&
           key_sound_pack == other.key_sound_pack && key_sound_volume == other.key_sound_volume &&
           commit_sound_enabled == other.commit_sound_enabled && melody_pack == other.melody_pack &&
           music_enabled == other.music_enabled && music_pack == other.music_pack &&
           music_volume == other.music_volume && achievements_enabled == other.achievements_enabled &&
           command_tables == other.command_tables && effect_style == other.effect_style &&
           effect_intensity == other.effect_intensity && effect_pack == other.effect_pack &&
           combo_counter == other.combo_counter && combo_tier_sound == other.combo_tier_sound &&
           phrase_tables == other.phrase_tables && helpcode_pack_quanpin == other.helpcode_pack_quanpin &&
           helpcode_pack_shuangpin == other.helpcode_pack_shuangpin;
  }
  bool operator!=(Preferences const &other) const { return !(*this == other); }
};

inline constexpr std::wstring_view default_sound_pack = L"default";
inline constexpr std::wstring_view default_melody_pack = L"twinkle";
// `client-core::plugins::BUILTIN_MUSIC_PACKS`：安装包随带的音乐包，按这个顺序挑默认的那一个。
inline constexpr std::array<std::wstring_view, 2> builtin_music_packs{L"msime-music-lofi",
                                                                      L"msime-music-ambient"};

// 打开「背景音乐」开关时还没选音乐包，就替用户选的那个包（`musicPackToSelect`）：宿主只在开关打开并且选了包时播放，只打开开关什么也听不到。优先安装包随带的音乐包（按 `builtin_music_packs` 的顺序），其次目录里的第一个音乐包；目录还没读到（`listed` 为假）时选随带的第一个。目录读到了却一个音乐包都没有时为空，这时开关不能打开。
inline std::wstring music_pack_to_select(std::vector<Package> const &packages, bool listed) {
  if (!listed)
    return std::wstring(builtin_music_packs[0]);
  for (auto const id : builtin_music_packs)
    for (auto const &pack : packages)
      if (pack.kind == L"music" && pack.id == id && pack.builtin)
        return pack.id;
  for (auto const &pack : packages)
    if (pack.kind == L"music")
      return pack.id;
  return {};
}

// 「成就音效」不能打开：成就按打字统计计数，统计关着时打开它什么也不会响。已经打开的仍然可以关掉，不把用户锁在改不回来的状态里；还不知道统计开没开（没读到）时按打开处理。
inline bool achievements_blocked(std::optional<bool> statistics_enabled, bool achievements_enabled) {
  return statistics_enabled.has_value() && !*statistics_enabled && !achievements_enabled;
}
inline constexpr std::size_t max_command_tables = 16;
inline constexpr std::size_t max_phrase_tables = 16;

inline bool contains(std::vector<std::wstring> const &list, std::wstring_view id) {
  return std::find(list.begin(), list.end(), id) != list.end();
}

inline std::vector<std::wstring> without(std::vector<std::wstring> list, std::wstring_view id) {
  list.erase(std::remove(list.begin(), list.end(), id), list.end());
  return list;
}

// 包从磁盘删除之后的设置：选中它的选择回到默认值（特效包回到无），启用它的指令表或短语表被移除（`withoutRemovedPack`）。
inline Preferences without_removed_pack(Preferences preferences, std::wstring_view kind,
                                        std::wstring_view id) {
  if (kind == L"phrase_table") {
    preferences.phrase_tables = without(preferences.phrase_tables, id);
    return preferences;
  }
  // 单词本、符号集和脚本插件没有偏好键；单词本的复习进度留在背单词里，脚本插件的授权条目由宿主在删包时一起去掉。
  if (kind == L"wordbook" || kind == L"symbol_set" || kind == L"script")
    return preferences;
  if (kind == L"helpcode") {
    if (preferences.helpcode_pack_quanpin == id)
      preferences.helpcode_pack_quanpin.clear();
    if (preferences.helpcode_pack_shuangpin == id)
      preferences.helpcode_pack_shuangpin.clear();
    return preferences;
  }
  if (kind == L"effect") {
    if (preferences.effect_pack == id)
      preferences.effect_pack.clear();
    return preferences;
  }
  if (kind == L"command_table") {
    preferences.command_tables = without(preferences.command_tables, id);
    return preferences;
  }
  if (kind == L"music") {
    if (preferences.music_pack == id) {
      preferences.music_enabled = false;
      preferences.music_pack.clear();
    }
    return preferences;
  }
  if (preferences.key_sound_pack == id)
    preferences.key_sound_pack = default_sound_pack;
  if (preferences.melody_pack == id)
    preferences.melody_pack = default_melody_pack;
  return preferences;
}

// 把一个包设为它所属类型的当前选择（`withPackSelected`）：选用声音或音乐包就同时打开对应的播放开关，声音包还切到能播放它的发声方式；特效包成为特效，指令表和短语表启用在已启用的之后，辅助码表两个方案都用它。
inline Preferences with_pack_selected(Preferences preferences, Package const &pack) {
  if (pack.kind == L"sound") {
    preferences.key_sound_enabled = true;
    if (pack.mode == L"sequence") {
      preferences.key_sound_mode = L"melody";
      preferences.melody_pack = pack.id;
    } else {
      preferences.key_sound_mode = L"keys";
      preferences.key_sound_pack = pack.id;
    }
  } else if (pack.kind == L"effect") {
    preferences.effect_pack = pack.id;
  } else if (pack.kind == L"music") {
    preferences.music_enabled = true;
    preferences.music_pack = pack.id;
  } else if (pack.kind == L"command_table") {
    if (!contains(preferences.command_tables, pack.id))
      preferences.command_tables.push_back(pack.id);
  } else if (pack.kind == L"phrase_table") {
    if (!contains(preferences.phrase_tables, pack.id))
      preferences.phrase_tables.push_back(pack.id);
  } else if (pack.kind == L"helpcode") {
    preferences.helpcode_pack_quanpin = pack.id;
    preferences.helpcode_pack_shuangpin = pack.id;
  }
  return preferences;
}

// 详情里「启用」开关拨动后的表（`setCommandTable` / `setPhraseTable`）：打开时排到最后，关闭时移除。
inline std::vector<std::wstring> with_table(std::vector<std::wstring> const &tables,
                                            std::wstring const &id, bool enabled) {
  auto next = without(tables, id);
  if (enabled)
    next.push_back(id);
  return next;
}

// ---- 选择与标记 ----

// 声音选择能播放的方式：`key_sound.pack` 指名按键采样包，`melody.pack` 指名旋律；没写方式的包按按键包算（`soundPackUse`）。
inline std::wstring_view sound_use(Package const &pack) {
  return pack.mode == L"sequence" ? std::wstring_view(L"sequence") : std::wstring_view(L"keys");
}

// 一个选择指名的已安装包（`selectedPack`）：类型和 id 相同，声音选择还要播放方式相同。
inline Package const *selected_pack(std::vector<Package> const &packages, std::wstring_view kind,
                                    std::wstring_view id,
                                    std::optional<std::wstring_view> use = std::nullopt) {
  for (auto const &pack : packages)
    if (pack.kind == kind && pack.id == id &&
        (kind != L"sound" || !use || sound_use(pack) == *use))
      return &pack;
  return nullptr;
}

// 有没有声音从按键音效包里放出来：按键采样、上屏音（含连击升档）或成就音效（`keySoundPackPlays`）。
inline bool key_sound_pack_plays(Preferences const &preferences) {
  return (preferences.key_sound_enabled && preferences.key_sound_mode == L"keys") ||
         preferences.commit_sound_enabled || preferences.achievements_enabled ||
         (preferences.combo_counter && preferences.combo_tier_sound);
}

// 按键是否在弹旋律：按键音开着、发声方式是按键旋律（`melodyPlays`）。
inline bool melody_plays(Preferences const &preferences) {
  return preferences.key_sound_enabled && preferences.key_sound_mode == L"melody";
}

// 输入方案的显示名（`schemeTitle`）。
inline std::wstring scheme_title(std::wstring_view scheme) {
  if (scheme == L"quanpin")
    return L"全拼";
  if (scheme == L"shuangpin")
    return L"双拼";
  if (scheme == L"wubi")
    return L"五笔";
  if (scheme == L"japanese")
    return L"日语";
  if (scheme == L"korean")
    return L"韩语";
  if (scheme == L"cantonese")
    return L"粤拼";
  if (scheme == L"zhuyin")
    return L"注音";
  if (scheme == L"vietnamese")
    return L"越南语";
  if (scheme == L"tibetan")
    return L"藏文";
  if (scheme == L"stroke")
    return L"笔画";
  return std::wstring(scheme);
}

// 能打开 K、/ 和 @ 模式的方案：全拼、双拼和五笔（`schemeOpensTableModes`）。
inline bool scheme_opens_table_modes(std::wstring_view scheme) {
  return scheme == L"quanpin" || scheme == L"shuangpin" || scheme == L"wubi";
}

// 当前方案打不开某个模式时详情里的说明；能打开时为空（`schemeTableModeNote`）。
inline std::wstring scheme_table_mode_note(std::wstring_view scheme, std::wstring_view mode) {
  if (scheme_opens_table_modes(scheme))
    return {};
  return L"当前输入方案「" + scheme_title(scheme) + L"」打不开" + std::wstring(mode) +
         L"，切换到全拼、双拼或五笔后才能用。";
}

// K 模式、/ 指令是否打开，以及当前方案能否打开它们（`PluginTableModes`）。
struct TableModes {
  bool quick_phrase = true;
  bool command = false;
  bool scheme_opens = true;
};

// 宿主会用到的包类型（`actedKinds`）：Windows 上声音、音乐、指令表、短语表和特效包都用；辅助码表只在显示辅助码的版本里用。
inline std::set<std::wstring> acted_kinds(bool helpcode) {
  std::set<std::wstring> kinds{L"sound", L"music", L"command_table", L"phrase_table", L"effect"};
  if (helpcode)
    kinds.insert(L"helpcode");
  return kinds;
}

// 辅助码表包用在哪个方案上；都没用时为空（`helpcodePackUses`）。
inline std::wstring helpcode_pack_uses(Preferences const &preferences, std::wstring_view id) {
  std::vector<std::wstring> schemes;
  if (preferences.helpcode_pack_quanpin == id)
    schemes.push_back(L"全拼");
  if (preferences.helpcode_pack_shuangpin == id)
    schemes.push_back(L"双拼");
  if (schemes.empty())
    return {};
  std::wstring uses = L"用于" + schemes[0];
  for (std::size_t i = 1; i < schemes.size(); ++i)
    uses += L"和" + schemes[i];
  return uses;
}

// 列表行尾的标记（`packMarker`）：使用中、当前旋律、已启用和排位；选中了但播放它的开关关着时写明哪个开关，方案打不开模式时先说方案。不在用或宿主不用这一类时为空。
inline std::wstring pack_marker(Package const &pack, Preferences const &preferences,
                                std::set<std::wstring> const &kinds, TableModes const &modes) {
  if (!kinds.count(pack.kind))
    return {};
  if (pack.kind == L"sound") {
    if (sound_use(pack) == L"sequence") {
      if (preferences.melody_pack != pack.id)
        return {};
      return melody_plays(preferences) ? L"当前旋律" : L"已选 · 按键旋律未开";
    }
    if (preferences.key_sound_pack != pack.id)
      return {};
    if (key_sound_pack_plays(preferences))
      return L"使用中";
    return preferences.key_sound_enabled ? L"已选 · 发声方式是按键旋律" : L"已选 · 按键音未开";
  }
  if (pack.kind == L"effect")
    return preferences.effect_pack == pack.id ? L"使用中" : L"";
  if (pack.kind == L"music") {
    if (preferences.music_pack != pack.id)
      return {};
    return preferences.music_enabled ? L"使用中" : L"已选 · 背景音乐未开";
  }
  auto table = [&](std::vector<std::wstring> const &tables, bool on, wchar_t const *closed,
                   wchar_t const *scheme) -> std::wstring {
    const auto found = std::find(tables.begin(), tables.end(), pack.id);
    if (found == tables.end())
      return {};
    if (!modes.scheme_opens)
      return scheme;
    return on ? L"已启用 · 第 " + std::to_wstring(found - tables.begin() + 1) + L" 位"
              : std::wstring(closed);
  };
  if (pack.kind == L"command_table")
    return table(preferences.command_tables, modes.command, L"已启用 · / 指令未开",
                 L"已启用 · 当前方案打不开 / 指令");
  if (pack.kind == L"phrase_table")
    return table(preferences.phrase_tables, modes.quick_phrase, L"已启用 · K 模式未开",
                 L"已启用 · 当前方案打不开 K 模式");
  if (pack.kind == L"helpcode")
    return helpcode_pack_uses(preferences, pack.id);
  // 单词本没有偏好：选中哪本书在背单词里；符号集装上就显示。
  return {};
}

// 插件库卡片上一个包的元信息（macOS 的 `PluginLibraryView` 卡片说明）：内置包写「内置」，其余写版本，再接「按键旋律」和非内置包的作者，用「 · 」连起来，空的项略去。脚本插件也用这一句。
inline std::wstring pack_row_description(Package const &pack) {
  std::vector<std::wstring> parts;
  parts.push_back(pack.builtin ? std::wstring(L"内置") : pack.version);
  if (pack.kind == L"sound" && pack.mode == L"sequence")
    parts.push_back(L"按键旋律");
  if (!pack.builtin)
    parts.push_back(pack.author);
  std::wstring description;
  for (auto const &part : parts) {
    if (part.empty())
      continue;
    if (!description.empty())
      description += L" · ";
    description += part;
  }
  return description;
}

// 指名的包已不在本机、或者是另一种播放方式的声音包的选择（`MissingSelection`）。
struct MissingSelection {
  std::wstring kind, id;
  std::vector<std::wstring> uses;
  bool mismatched = false;
};

// 指名的包不在目录里（或声音包的方式不对）的选择，每个包一项，按列表顺序；只在读到目录之后有意义（`missingSelections`）。
inline std::vector<MissingSelection> missing_selections(Preferences const &preferences,
                                                       std::vector<Package> const &packages,
                                                       std::set<std::wstring> const &kinds) {
  std::vector<MissingSelection> missing;
  auto add = [&](std::wstring const &kind, std::wstring const &id, std::wstring const &use,
                 std::optional<std::wstring_view> sound = std::nullopt) {
    if (id.empty() || !kinds.count(kind))
      return;
    if (selected_pack(packages, kind, id, sound))
      return;
    const bool mismatched = std::any_of(packages.begin(), packages.end(), [&](Package const &pack) {
      return pack.kind == kind && pack.id == id;
    });
    for (auto &entry : missing)
      if (entry.kind == kind && entry.id == id) {
        entry.uses.push_back(use);
        return;
      }
    missing.push_back({kind, id, {use}, mismatched});
  };
  add(L"sound", preferences.key_sound_pack, L"当前音效包", L"keys");
  add(L"sound", preferences.melody_pack, L"当前旋律", L"sequence");
  add(L"effect", preferences.effect_pack, L"当前特效包");
  add(L"music", preferences.music_pack, L"当前音乐包");
  for (auto const &id : preferences.command_tables)
    add(L"command_table", id, L"已启用的指令表");
  for (auto const &id : preferences.phrase_tables)
    add(L"phrase_table", id, L"已启用的短语表");
  add(L"helpcode", preferences.helpcode_pack_quanpin, L"全拼辅助码");
  add(L"helpcode", preferences.helpcode_pack_shuangpin, L"双拼辅助码");
  return missing;
}

inline std::wstring joined(std::vector<std::wstring> const &parts, std::wstring_view separator) {
  std::wstring result;
  for (std::size_t i = 0; i < parts.size(); ++i) {
    if (i)
      result += separator;
    result += parts[i];
  }
  return result;
}

// 缺失的选择在列表和标题上的名字（`missingTitle`）。
inline std::wstring missing_title(MissingSelection const &entry) {
  return entry.id + (entry.mismatched ? L"（不可用）" : L"（未找到）");
}

// 为什么用不了，接在它被选作什么之后（`missingReason`）。
inline std::wstring missing_reason(MissingSelection const &entry) {
  if (!entry.mismatched)
    return L"已不在本机";
  return contains(entry.uses, L"当前音效包") ? L"但它是按键旋律，不能用作按键音效"
                                             : L"但它不是按键旋律";
}

// 列表里缺失选择那一行的说明。
inline std::wstring missing_description(MissingSelection const &entry) {
  return joined(entry.uses, L"、") + L"，" + missing_reason(entry);
}

// 缺失选择详情里唯一的按钮（`MissingPluginView` 的 action）。
inline std::wstring missing_action(MissingSelection const &entry) {
  if (entry.kind == L"sound")
    return L"改回默认";
  if (entry.kind == L"command_table" || entry.kind == L"phrase_table")
    return L"移除";
  if (entry.kind == L"helpcode")
    return L"改回原来的方案";
  return L"不再使用";
}

// 缺失选择详情的说明（`MissingPluginView` 的 note）。
inline std::wstring missing_note(MissingSelection const &entry) {
  const auto uses = joined(entry.uses, L"、");
  const auto label = kind_label(entry.kind);
  const auto action = missing_action(entry);
  if (entry.mismatched)
    return L"设置里" + uses + L"是「" + entry.id + L"」，" + missing_reason(entry) + L"。可以" +
           action + L"，或者在「我的插件」里打开另一个" + label + L"。";
  return L"设置里" + uses + L"是「" + entry.id + L"」，但这个" + label +
         L"已不在本机，可能被删除或无法载入。可以重新导入它，或者" + action + L"。";
}

// 按下缺失选择的按钮之后的设置：包不在了就像删除它一样；方式不对的声音包还装着、可能在它自己的方式下选得没错，所以只重置用不了它的那个选择。
inline Preferences missing_dropped(Preferences preferences, MissingSelection const &entry) {
  if (!entry.mismatched)
    return without_removed_pack(std::move(preferences), entry.kind, entry.id);
  if (contains(entry.uses, L"当前音效包"))
    preferences.key_sound_pack = default_sound_pack;
  else
    preferences.melody_pack = default_melody_pack;
  return preferences;
}

// ---- 声音与效果 ----

// 「声音与效果」入口的说明：Windows 上三样都有。
inline std::wstring sound_effects_description() {
  return L"按键音效、打字效果、背景音乐的开关与音量";
}

// 「声音与效果」里各个选择显示的名字（`selectedName`）：读到的包用它的名字，没读到目录时原样显示，读到了却没有时标未找到或不可用。
inline std::wstring selected_name(std::vector<Package> const &packages, bool listed,
                                  std::wstring_view kind, std::wstring const &id,
                                  std::wstring const &none,
                                  std::optional<std::wstring_view> use = std::nullopt) {
  if (id.empty())
    return none;
  if (auto const *pack = selected_pack(packages, kind, id, use))
    return pack_label(*pack);
  if (!listed)
    return id;
  const bool installed = std::any_of(packages.begin(), packages.end(), [&](Package const &pack) {
    return pack.kind == kind && pack.id == id;
  });
  return id + (installed ? L"（不可用）" : L"（未找到）");
}

// 选择旁边的说明：Windows 有插件目录，所以总是这一句（`choose`）。
inline constexpr std::wstring_view choose_note = L"在「我的插件」里打开一个包即可更换。";

// ---- 详情的「使用」 ----

// 一组说明和一个按钮。
struct PackAction {
  std::wstring note;
  std::wstring label;
  bool disabled = false;
};

// 音效包（按键采样或旋律）的「使用」：选用即使用，包已选中但开关没开时按钮仍可点，点了会把开关重新打开。
inline PackAction sound_pack_action(Package const &pack, Preferences const &preferences) {
  if (pack.mode == L"sequence") {
    const bool selected = preferences.melody_pack == pack.id;
    const bool playing = melody_plays(preferences);
    return {playing ? L"按键旋律每按一键弹出旋律的下一个音，停顿 3 秒后从头开始。"
                    : L"按键旋律现在没有打开。设为按键旋律会同时打开按键音，并把发声方式改为按键旋律。",
            selected ? (playing ? L"使用中" : L"打开按键旋律") : L"设为按键旋律",
            selected && playing};
  }
  const bool selected = preferences.key_sound_pack == pack.id;
  const bool keys_enabled = preferences.key_sound_enabled;
  const bool playing = keys_enabled && preferences.key_sound_mode == L"keys";
  const std::wstring resume = keys_enabled ? L"改用按键音效" : L"打开按键音";
  return {playing ? L"按键、上屏和成就音效都取自当前音效包；开关和音量在「声音与效果」里。"
          : keys_enabled
              ? L"发声方式现在是按键旋律。设为当前音效包会改回按键音效；上屏和成就音效也取自当前音效包，开关在「声音与效果」里。"
              : L"按键音现在没有打开。设为当前音效包会同时打开按键音；上屏和成就音效也取自当前音效包，开关在「声音与效果」里。",
          selected ? (playing ? std::wstring(L"使用中") : resume) : L"设为当前音效包",
          selected && playing};
}

// 音乐包的「使用」；选中时另有「不再使用」。
inline PackAction music_pack_action(Package const &pack, Preferences const &preferences) {
  const bool selected = preferences.music_pack == pack.id;
  const bool playing = preferences.music_enabled;
  return {playing ? L"背景音乐只在输入法处于活动状态时播放。"
                  : L"背景音乐现在没有打开。设为当前音乐包会同时打开它，只在输入法处于活动状态时播放。",
          selected ? (playing ? L"使用中" : L"打开背景音乐") : L"设为当前音乐包",
          selected && playing};
}

// 指令表或短语表详情里「启用」开关的说明和是否可拨（满了且自己没启用时不可拨）。
struct TableSwitch {
  bool on = false;
  bool full = false;
  std::wstring description;
};

inline TableSwitch command_table_switch(Preferences const &preferences, std::wstring const &id) {
  const auto found = std::find(preferences.command_tables.begin(), preferences.command_tables.end(), id);
  const bool on = found != preferences.command_tables.end();
  const bool full = !on && preferences.command_tables.size() >= max_command_tables;
  return {on, full,
          on ? L"第 " + std::to_wstring(found - preferences.command_tables.begin() + 1) +
                   L" 位。同一指令以靠前的表为准。"
          : full ? L"最多启用 " + std::to_wstring(max_command_tables) + L" 个指令表。"
                 : std::wstring(L"启用后排在已启用的指令表之后。")};
}

inline TableSwitch phrase_table_switch(Preferences const &preferences, std::wstring const &id) {
  const auto found = std::find(preferences.phrase_tables.begin(), preferences.phrase_tables.end(), id);
  const bool on = found != preferences.phrase_tables.end();
  const bool full = !on && preferences.phrase_tables.size() >= max_phrase_tables;
  return {on, full,
          on ? L"第 " + std::to_wstring(found - preferences.phrase_tables.begin() + 1) +
                   L" 位。同一编码下靠前的表先列出。"
          : full ? L"最多启用 " + std::to_wstring(max_phrase_tables) + L" 个短语表。"
                 : std::wstring(L"启用后排在已启用的短语表之后。")};
}

// 特效样式的选项（`effectStyleOptions`）。
struct StyleOption {
  std::wstring_view value, label;
};
inline constexpr std::array<StyleOption, 4> effect_style_options{{
    {L"off", L"关闭"}, {L"flash", L"闪光"}, {L"sparks", L"火花"}, {L"power_mode", L"Power Mode"}}};

inline std::wstring effect_style_label(std::wstring_view style) {
  for (auto const &option : effect_style_options)
    if (option.value == style)
      return std::wstring(option.label);
  return std::wstring(style);
}

// 各类详情的预览行数（`PHRASE_PREVIEW_ROWS`、`WORDBOOK_PREVIEW_WORDS`、`SYMBOL_PREVIEW_ITEMS`）。
inline constexpr std::size_t phrase_preview_rows = 20;
inline constexpr std::size_t wordbook_preview_words = 5;
inline constexpr std::size_t symbol_preview_items = 16;

// 单词本插件在背单词里的词书 id（`wordbookPackBookId`）。
inline std::wstring wordbook_book_id(std::wstring const &id) { return L"pack-" + id; }

// 符号集一组的标题和前几项。
inline std::wstring symbol_group_title(SymbolGroup const &group) {
  return std::wstring(group.tab == L"kaomoji" ? L"颜文字" : L"符号") + L" · " + group.title + L"（" +
         std::to_wstring(group.items.size()) + L"）";
}

inline std::wstring symbol_group_preview(SymbolGroup const &group) {
  std::wstring text;
  const auto shown = std::min(group.items.size(), symbol_preview_items);
  for (std::size_t i = 0; i < shown; ++i) {
    if (i)
      text += L" ";
    text += group.items[i];
  }
  if (group.items.size() > symbol_preview_items)
    text += L" …";
  return text;
}

// ---- 出错说明 ----

// 宿主调用失败时的句子（`pluginErrorMessage`）：code 是 client-services 的 `PluginFailure`，detail 是它报告的规则。
inline std::wstring error_message(std::wstring_view code, std::wstring const &detail,
                                  std::wstring const &fallback) {
  const std::wstring reason = detail.empty() ? std::wstring() : L"（" + detail + L"）";
  if (code == L"plugin_invalid")
    return L"插件不符合要求" + reason + L"。";
  if (code == L"plugin_archive")
    return L"压缩包无法读取" + reason + L"。";
  if (code == L"plugin_unsupported_source")
    return L"只能导入插件文件夹或 .zip 文件。";
  if (code == L"plugin_reserved")
    return L"这个 id 属于内置插件，不能覆盖或删除。";
  if (code == L"plugin_storage")
    return L"插件目录不可用，请检查数据目录的权限。";
  if (code == L"plugin_script_memory")
    return L"启用后会超出输入法留给脚本插件的内存（64 MiB），先停用别的脚本插件再试。";
  if (code == L"plugin_script_limit")
    return L"最多同时启用 8 个脚本插件，先停用一个再试。";
  if (code == L"plugin_script_changed")
    return L"这个插件的内容在你打开之后变了，页面已经重新读取，请看过现在的内容再试。";
  if (code == L"plugin_script_grants_format")
    return L"授权文件是更新版本的水杉输入法写的，这个版本不能修改它。";
  if (code == L"plugin_script_grant_invalid")
    return L"授权不符合要求" + reason + L"。";
  if (code == L"plugin_script_missing")
    return L"找不到这个脚本插件" + reason + L"。";
  if (code == L"mention_invalid")
    return L"名单有误" + reason + L"。";
  if (code == L"mention_format")
    return L"名单文件无法读取；保存会用新的名单替换它。";
  if (code == L"mention_storage")
    return L"名单文件不可用，请检查数据目录的权限。";
  return fallback;
}

// 导入成功后的一句（`importPack`）。
inline std::wstring imported_notice(Package const &pack) {
  return L"已导入" + pack_kind_label(pack) + L"「" + pack.name + L"」" + pack.version + L"。";
}

// ---- 脚本插件（实验） ----

// 「脚本插件（实验）」分组、详情和授权对话框，照 `plugin-script-model.ts`、`script-grant-dialog.tsx`、`plugin-script-view.tsx` 和 `plugin-library-view.tsx` 的「脚本插件（实验）」节移植。规则（依赖、互斥、哈希绑定、准入）的权威在 client-services 的 `script_grants`，这里只是让页面先按同样的规则勾选和说明，宿主还会再查一遍。

// 脚本插件的能力，按 `msime_script_runtime::Capability::ALL` 的顺序（`SCRIPT_CAPABILITIES`）。
inline constexpr std::array<std::wstring_view, 8> script_capabilities{
    L"read_composition",  L"read_commit",         L"read_candidates",   L"rewrite_commit",
    L"rerank_candidates", L"annotate_candidates", L"filter_candidates", L"provide_commands"};

// 输入法同时运行的脚本插件个数上限（`limits::MAX_ENABLED_SCRIPTS`）。
inline constexpr std::size_t max_enabled_scripts = 8;

// 授予一项之前必须一起授予的能力（`CAPABILITY_REQUIRES`，即 `Capability::requires`）；没有时为空。
inline std::wstring_view capability_requires(std::wstring_view capability) {
  if (capability == L"rewrite_commit")
    return L"read_commit";
  if (capability == L"rerank_candidates" || capability == L"annotate_candidates" ||
      capability == L"filter_candidates")
    return L"read_candidates";
  return {};
}

// 和这一项互斥的那一项（`exclusivePartner`）：改写上屏的插件读不到组字过程，免得它把放弃的组字夹带进后面的上屏。不互斥时为空。
inline std::wstring_view exclusive_partner(std::wstring_view capability) {
  if (capability == L"rewrite_commit")
    return L"read_composition";
  if (capability == L"read_composition")
    return L"rewrite_commit";
  return {};
}

// 一项能力的白话说明（`capabilityLabels`）：标题说能做什么，说明说这意味着插件能看到或改动什么。不认识的能力原样作标题。
struct CapabilityLabel {
  std::wstring title, description;
};

inline CapabilityLabel capability_label(std::wstring_view capability) {
  if (capability == L"read_composition")
    return {L"读取正在输入的内容", L"能看到你正在打的拼音或编码、组字时显示的文字，以及每次按下的字符。"};
  if (capability == L"read_commit")
    return {L"读取输入的文字", L"能看到你选中的候选和最终输入到应用里的文字。"};
  if (capability == L"read_candidates")
    return {L"读取候选词", L"能看到候选窗口里当前一页的候选，最多 20 个。"};
  if (capability == L"rewrite_commit")
    return {L"修改输入的文字", L"能在文字输入到应用之前改写它，例如在中英文之间加空格。"};
  if (capability == L"rerank_candidates")
    return {L"调整候选顺序", L"能把候选词往前或往后排。"};
  if (capability == L"annotate_candidates")
    return {L"给候选加注释", L"能在候选旁边显示一小段说明。"};
  if (capability == L"filter_candidates")
    return {L"隐藏候选", L"能把某些候选移到末尾并隐藏起来。"};
  // 和共享页的 `capabilityLabels`、`docs/plugins.md` 一字不差。
  if (capability == L"provide_commands")
    return {L"提供 / 指令",
            L"你在 / 指令里输入它声明的触发词（例如 /uuid）之后打的字母会交给这个插件，它据此给出候选；给出的候选只取决于这些字母和随机数，带不走你之前打过的内容。你按序号、点选、按空格选中某一行，或者打标点结束输入时，高亮的那一行会输入；在有的系统上，按快捷键或 Tab 等键、用菜单或工具栏结束输入时也会输入这一行。按回车时只输入你打的原文（例如 /uuid），点到别的窗口时插件给出的行不会输入。它看不到你平时打的字、其他指令和别的插件的内容；它给出的文字不进入词库、不计入打字统计、不会作为上文发给 AI 候选，也不会拿去查释义或翻译。"};
  return {std::wstring(capability), {}};
}

// 清单里钩子的白话名字：插件在什么时候运行（`hookLabels`）。不认识的名字原样显示。
inline std::wstring hook_label(std::wstring_view hook) {
  if (hook == L"focus_in")
    return L"进入输入框时";
  if (hook == L"focus_out")
    return L"离开输入框时";
  if (hook == L"input")
    return L"每次打字时";
  if (hook == L"selection_retracted")
    return L"撤回选词时";
  if (hook == L"candidate_selected")
    return L"选中候选时";
  if (hook == L"before_commit")
    return L"文字输入到应用之前（可以改写）";
  if (hook == L"commit")
    return L"文字输入到应用之后";
  if (hook == L"composition_start")
    return L"开始组字时";
  if (hook == L"composition_update")
    return L"组字变化时";
  if (hook == L"composition_end")
    return L"组字结束时";
  if (hook == L"candidates_shown")
    return L"候选窗口出现时";
  if (hook == L"candidates_hidden")
    return L"候选窗口消失时";
  if (hook == L"candidates")
    return L"生成候选时（可以调整候选）";
  if (hook == L"command")
    return L"你在 / 指令里输入它的触发词时（给出候选）";
  return std::wstring(hook);
}

// 输入法停用一个插件的原因：`script-health.json` 的四个类别（`healthLabels`）。
inline std::wstring health_label(std::wstring_view disabled) {
  if (disabled == L"invalid")
    return L"模块不符合要求，输入法已停用它";
  if (disabled == L"init")
    return L"初始化失败，输入法已停用它";
  if (disabled == L"failed")
    return L"运行时出错太多次，输入法已停用它";
  if (disabled == L"slow")
    return L"运行太慢，输入法已停用它";
  return std::wstring(disabled);
}

// 内容哈希的前 12 位，授权页用它让人分辨两个版本（`shortHash`）。
inline std::wstring short_hash(std::wstring_view hash) { return std::wstring(hash.substr(0, 12)); }

// 认识的能力，去重并按 `script_capabilities` 排序（`sortCapabilities`）。
inline std::vector<std::wstring> sort_capabilities(std::vector<std::wstring> const &capabilities) {
  std::vector<std::wstring> sorted;
  for (auto const capability : script_capabilities)
    if (contains(capabilities, capability))
      sorted.emplace_back(capability);
  return sorted;
}

// 勾选或取消一项之后的授权，只在清单申请过的能力（`requested`）里变动：勾选时把它依赖的那一项一起勾上，并去掉和它互斥的那一项（以及依赖那一项的）；取消时把依赖它的各项一起取消。结果按 `script_capabilities` 排序，所以总能通过宿主的依赖和互斥检查（`toggleCapability`）。
inline std::vector<std::wstring> toggle_capability(std::vector<std::wstring> const &current,
                                                   std::wstring_view capability, bool checked,
                                                   std::vector<std::wstring> const &requested) {
  std::set<std::wstring> next;
  for (auto const &item : current)
    if (contains(requested, item))
      next.insert(item);
  auto remove_with_dependents = [&next](std::wstring_view target) {
    next.erase(std::wstring(target));
    for (auto const item : script_capabilities)
      if (capability_requires(item) == target)
        next.erase(std::wstring(item));
  };
  auto sorted = [&next] {
    return sort_capabilities(std::vector<std::wstring>(next.begin(), next.end()));
  };
  if (!checked) {
    remove_with_dependents(capability);
    return sorted();
  }
  if (!contains(requested, capability))
    return sorted();
  next.insert(std::wstring(capability));
  const auto required = capability_requires(capability);
  if (!required.empty() && contains(requested, required))
    next.insert(std::wstring(required));
  if (const auto partner = exclusive_partner(capability); !partner.empty())
    remove_with_dependents(partner);
  // 勾上依赖项之后，它的互斥项也要让开（例如勾选「修改输入的文字」带上「读取输入的文字」）。
  if (!required.empty())
    if (const auto partner = exclusive_partner(required); !partner.empty())
      remove_with_dependents(partner);
  return sorted();
}

// `script-grants.json` 的一条（client-services 的 `ScriptGrantView`）。
struct ScriptGrant {
  std::wstring id;
  // 授权时包的内容哈希；与包现在的 `content_hash` 不同时授权已经作废。
  std::wstring hash;
  bool enabled = false;
  std::vector<std::wstring> granted;
};

// 输入法停用了的一个脚本插件（`<user_data>/script-health.json`）。
struct ScriptHealth {
  std::wstring id, hash, disabled;
};

// 一条插件 `/` 指令在输入法里的状态（client-services 的 `script_commands::ResolvedCommand`，`ScriptCommandState`）。`status` 是 `active`、`shadowed_by_user_table`（和用户指令表里的 `/conflict` 冲突）或 `shadowed_by_plugin`（和执行顺序在前的插件 `conflict_id` 的 `/conflict` 冲突）；用不到的字段是空串。
struct ScriptCommandState {
  std::wstring id, trigger, title, status, conflict, conflict_id;
};

// 授权页读到的全部（`ScriptGrantsView`）。`plugins` 的顺序就是执行顺序；`commands` 是启用、授予了「提供 / 指令」、清单能载入的插件声明的指令和状态。
struct ScriptGrants {
  std::vector<ScriptGrant> plugins;
  std::vector<ScriptHealth> health;
  std::vector<ScriptCommandState> commands;
};

inline ScriptGrant const *script_grant(ScriptGrants const *grants, std::wstring_view id) {
  if (!grants)
    return nullptr;
  for (auto const &grant : grants->plugins)
    if (grant.id == id)
      return &grant;
  return nullptr;
}

// 一个脚本插件的授权状态，按页面要说的先后：没授权过、内容变了要重新授权、启用、停用（`scriptGrantState`）。
enum class ScriptGrantState { ungranted, stale, enabled, disabled };

inline ScriptGrantState script_grant_state(Package const &pack, ScriptGrant const *grant) {
  if (!grant)
    return ScriptGrantState::ungranted;
  if (grant->hash != pack.content_hash)
    return ScriptGrantState::stale;
  return grant->enabled ? ScriptGrantState::enabled : ScriptGrantState::disabled;
}

// 输入法会运行的插件 id，按执行顺序：启用的前 8 个条目里内容哈希和已安装的包对得上的（`runningScriptIds`，即 `ScriptGrants::enabled` 之后再核对哈希）。授权还没读到（`grants` 为空）时为空。
inline std::vector<std::wstring> running_script_ids(ScriptGrants const *grants,
                                                    std::vector<Package> const &packages) {
  std::vector<std::wstring> running;
  if (!grants)
    return running;
  std::size_t enabled = 0;
  for (auto const &grant : grants->plugins) {
    if (!grant.enabled)
      continue;
    if (enabled++ >= max_enabled_scripts)
      break;
    const bool installed = std::any_of(packages.begin(), packages.end(), [&](Package const &pack) {
      return pack.kind == L"script" && pack.id == grant.id && pack.content_hash == grant.hash;
    });
    if (installed)
      running.push_back(grant.id);
  }
  return running;
}

// 输入法记下的停用，只算这个版本（哈希相同）的（`scriptHealth`）。
inline ScriptHealth const *script_health(ScriptGrants const *grants, Package const &pack) {
  if (!grants)
    return nullptr;
  for (auto const &entry : grants->health)
    if (entry.id == pack.id && entry.hash == pack.content_hash)
      return &entry;
  return nullptr;
}

inline std::ptrdiff_t running_position(std::vector<std::wstring> const &running, std::wstring_view id) {
  const auto found = std::find(running.begin(), running.end(), id);
  return found == running.end() ? -1 : found - running.begin();
}

// 随输入法分发的官方脚本插件（`builtin` 为真）详情里「来源」一行，与共享页的 `SCRIPT_BUILTIN_NOTE` 一字不差：它不能删除，和导入的插件一样默认不运行。
inline constexpr std::wstring_view script_builtin_note =
    L"随输入法安装和更新，不能删除；需要你授权后才会运行。";

// 授权之后包的内容变了时的标记（`scriptStaleLabel`）。内置插件的内容只会随输入法更新而变，说法跟着改。
inline std::wstring script_stale_label(Package const &pack) {
  return pack.builtin ? L"已随输入法更新，需要重新授权" : L"已更新，需要重新授权";
}

// 列表里这一行的标记（`scriptMarker`）。
inline std::wstring script_marker(Package const &pack, ScriptGrants const *grants,
                                  std::vector<std::wstring> const &running) {
  switch (script_grant_state(pack, script_grant(grants, pack.id))) {
  case ScriptGrantState::ungranted:
    return L"未授权";
  case ScriptGrantState::stale:
    return script_stale_label(pack);
  case ScriptGrantState::disabled:
    return L"未启用";
  case ScriptGrantState::enabled:
    break;
  }
  if (script_health(grants, pack))
    return L"已被输入法停用";
  const auto position = running_position(running, pack.id);
  return position < 0 ? std::wstring(L"已启用 · 超过 8 个，不运行")
                      : L"已启用 · 第 " + std::to_wstring(position + 1) + L" 位";
}

// 一个脚本插件的说明：版本和作者，用「 · 」连起来。设置页的插件库卡片改用 pack_row_description，不再调用这个函数；它留在这里，由单测覆盖。
inline std::wstring script_row_description(Package const &pack) {
  std::vector<std::wstring> parts;
  if (!pack.version.empty())
    parts.push_back(pack.version);
  if (!pack.author.empty())
    parts.push_back(L"作者 " + pack.author);
  return joined(parts, L" · ");
}

// / 指令关着时，提供指令的插件详情里的说明，后面跟着「打开 / 指令」按钮（`SCRIPT_COMMAND_MODE_NOTE`）。授权本身从不改这个开关。
inline constexpr std::wstring_view script_command_mode_note =
    L"需要先打开 / 指令，并使用中文标点；在全拼、双拼、五笔下没有输入时按 / 再输入触发词。";

// 包里冲突说明要用的名字：已安装的脚本插件 `id` 的名字，找不到时用 id。
inline std::wstring script_name(std::vector<Package> const &packages, std::wstring_view id) {
  for (auto const &pack : packages)
    if (pack.kind == L"script" && pack.id == id)
      return pack.name;
  return std::wstring(id);
}

// 一条插件指令现在的状态，白话（`scriptCommandStatus`）：没启用或没授予「提供 / 指令」时说要怎样才生效；输入法停用了插件时照实说；否则按宿主算出的状态说生效，或者和谁冲突、没生效。
inline std::wstring script_command_status(Package const &pack, std::wstring_view trigger,
                                          ScriptGrants const *grants,
                                          std::vector<Package> const &packages) {
  auto const *grant = script_grant(grants, pack.id);
  if (script_grant_state(pack, grant) != ScriptGrantState::enabled)
    return L"授权并启用后生效";
  if (!contains(grant->granted, L"provide_commands"))
    return L"没有授予「提供 / 指令」，不会生效";
  if (script_health(grants, pack))
    return L"插件已被输入法停用，不会给出候选";
  for (auto const &command : grants->commands) {
    if (command.id != pack.id || command.trigger != trigger)
      continue;
    if (command.status == L"active")
      return L"生效";
    if (command.status == L"shadowed_by_user_table")
      return L"与你的指令 /" + command.conflict + L" 冲突，未生效";
    if (command.status == L"shadowed_by_plugin")
      return L"与插件 " + script_name(packages, command.conflict_id) + L" 的 /" + command.conflict +
             L" 冲突，未生效";
    break;
  }
  return L"未生效";
}

// 授权对话框里列出的一条指令（`ScriptGrantDialog` 的列表）：「/uuid UUID」，调整授权时后面接着「：」和状态。
inline std::wstring script_command_line(Command const &command, std::wstring const &status) {
  auto line = L"/" + command.trigger + L" " + command.title;
  return status.empty() ? line : line + L"：" + status;
}

// 授权页上的平台说明（`scriptResidualRisk` 的 Windows 分支）。授权文件不签名，以同一用户身份运行的程序都能绕过授权页自己写授权，这是设计接受的残余风险，要写在用户做决定的地方：列表分组、详情和授权对话框。
inline constexpr std::wstring_view script_residual_risk =
    L"在 Windows 上，以你的身份运行的其他程序可以不经过这个页面，直接写入授权，让脚本插件读取或改写你输入的文字。只安装你信任的脚本插件。以管理员身份运行的窗口里不会运行脚本插件。";

// 「脚本插件（实验）」分组的说明，后面接着残余风险。
inline std::wstring script_group_note() {
  return L"脚本插件会在输入法里运行第三方代码，只做你在授权时允许的事；导入后默认不运行，打开它的详情授权后才启用。 " +
         std::wstring(script_residual_risk);
}

// 插件库底部折叠的「插件导入说明」：Windows 运行脚本插件，所以说清两种插件的区别。
inline constexpr std::wstring_view import_description =
    L"数据插件只含音频、指令模板等数据；脚本插件（实验）带可执行代码，导入后要经授权才会运行。每个包须声明许可证";

// 详情「状态」里的一句：授权状态，启用时再接输入法停用它的原因（停用原因只在启用时显示）。
inline std::wstring script_status(Package const &pack, ScriptGrantState state,
                                  ScriptHealth const *health) {
  switch (state) {
  case ScriptGrantState::ungranted:
    return L"还没有授权，不会运行。";
  case ScriptGrantState::stale:
    return script_stale_label(pack) +
           L"：插件的内容和授权时不同，之前的授权已经作废，不会运行。";
  case ScriptGrantState::disabled:
    return L"已停用，不会运行。";
  case ScriptGrantState::enabled:
    break;
  }
  return health ? L"已启用。" + health_label(health->disabled) + L"，重新授权或重启输入法后恢复。"
                : std::wstring(L"已启用。");
}

// 详情是否给出「执行顺序」一组（启用时）：只有一个脚本在运行时两个按钮都不能按，顺序也无从谈起，所以至少两个在运行时才给；排在第 8 个之后（`position` 为 -1）不会运行时仍要说明。
inline bool script_order_shown(std::ptrdiff_t position, std::size_t count) {
  return count >= 2 || position < 0;
}

// 详情「执行顺序」的说明（只在 `script_order_shown` 时显示）。`position` 为 -1 时排在第 8 个之后。
inline std::wstring script_order_note(std::ptrdiff_t position, std::size_t count) {
  if (position < 0)
    return L"同时启用的脚本插件超过 8 个，这个插件排在第 8 个之后，不会运行。";
  return L"第 " + std::to_wstring(position + 1) + L" 位，共 " + std::to_wstring(count) +
         L" 个。多个插件按这个顺序依次处理同一件事。";
}

// 在会运行的插件里把 `id` 往前（-1）或往后（1）挪一位之后交给宿主的顺序（`moveScript`）；挪不动时为空。
inline std::optional<std::vector<std::wstring>> moved_order(std::vector<std::wstring> running,
                                                            std::wstring_view id, int offset) {
  const auto position = running_position(running, id);
  const auto target = position + offset;
  if (position < 0 || target < 0 || target >= static_cast<std::ptrdiff_t>(running.size()))
    return std::nullopt;
  std::swap(running[static_cast<std::size_t>(position)], running[static_cast<std::size_t>(target)]);
  return running;
}

// 授权对话框里的一个复选框（`ScriptGrantDialog` 的 items）：说明后面接着依赖和互斥的提示；互斥的那一项勾着时禁用。
struct GrantItem {
  std::wstring capability, title, description;
  bool checked = false;
  bool disabled = false;
};

// 对话框现在列着、勾着的那几项。包在对话框打开之后被换过（`plugin_script_changed`，页面重读后换成新版本）时，`granted` 里可能还留着新版本不再申请的一项：它不显示，不能算作勾着去禁用互斥的另一项，也不能被交出去、被宿主以「清单没有申请」拒绝。
inline std::vector<std::wstring> selected_capabilities(std::vector<std::wstring> const &granted,
                                                       std::vector<std::wstring> const &requested) {
  std::vector<std::wstring> selected;
  for (auto const &item : granted)
    if (contains(requested, item))
      selected.push_back(item);
  return selected;
}

inline std::vector<GrantItem> grant_items(std::vector<std::wstring> const &requested,
                                          std::vector<std::wstring> const &granted, bool busy) {
  const auto selected = selected_capabilities(granted, requested);
  std::vector<GrantItem> items;
  for (auto const &capability : sort_capabilities(requested)) {
    const auto partner = exclusive_partner(capability);
    const bool blocked = !partner.empty() && contains(selected, partner);
    const auto required = capability_requires(capability);
    std::wstring notes;
    if (!required.empty())
      notes += L"需要同时授予「" + capability_label(required).title + L"」。";
    if (blocked)
      notes += L"不能和「" + capability_label(partner).title + L"」同时授予。";
    const auto label = capability_label(capability);
    items.push_back({capability, label.title,
                     notes.empty() ? label.description : label.description + L" " + notes,
                     contains(selected, capability), busy || blocked});
  }
  return items;
}

// 打开授权对话框时勾着的能力：调整授权时是现在授予的（只算清单还申请的），新授权和重新授权时一项都不预先勾选。
inline std::vector<std::wstring> grant_dialog_initial(Package const &pack, ScriptGrant const *grant) {
  if (script_grant_state(pack, grant) != ScriptGrantState::enabled)
    return {};
  return sort_capabilities(selected_capabilities(grant->granted, pack.permissions));
}

inline std::wstring grant_dialog_title(Package const &pack) { return L"授权「" + pack.name + L"」"; }

// 授权对话框开头的提醒：这次授权给的是变过的内容。
inline std::wstring grant_dialog_stale(Package const &pack) {
  return script_stale_label(pack) + L"：这个插件的内容和你上次授权时不同，之前的授权已经作废。";
}
inline constexpr std::wstring_view grant_dialog_intro =
    L"脚本插件是第三方代码，会在输入法里运行。只勾选你愿意让它做的事；没有勾选的，它一律做不到。在密码框和隐私输入框里它不会运行。";
inline constexpr std::wstring_view grant_dialog_no_permissions =
    L"这个插件没有申请任何权限：它只能知道发生了什么事件，读不到也改不了任何文字。";
inline constexpr std::wstring_view grant_dialog_legend = L"允许这个插件";
inline constexpr std::wstring_view grant_dialog_confirm = L"授权并启用";
inline constexpr std::wstring_view grant_dialog_failed = L"授权没有写入，请重试。";

// ---- @ 名单 ----

// `client-services::plugins::mentions::MentionEntry`。
struct Mention {
  std::wstring text, key;
  bool operator==(Mention const &other) const { return text == other.text && key == other.key; }
  bool operator!=(Mention const &other) const { return !(*this == other); }
};

inline constexpr std::size_t max_mentions = 1000;
// `mentions::MAX_TEXT_UTF16`：Windows 候选管道的文字字段。
inline constexpr std::size_t max_mention_text_utf16 = 199;
// `mentions::MAX_KEY_BYTES`。
inline constexpr std::size_t max_mention_key_bytes = 64;

// UTF-16 码元数，即 JavaScript 的 String.length。
inline std::size_t utf16_length(std::wstring_view value) {
  // Windows 的 wchar_t 本来就是 UTF-16 码元；别处的 wchar_t 是码点，基本平面以外的一个算两个。用预处理而不是 if constexpr：非模板函数里被丢弃的分支照样编译，16 位 wchar_t 上 `> 0xFFFF` 恒假，MinGW 的 -Werror=type-limits 会报错。
#if WCHAR_MAX <= 0xFFFF
  return value.size();
#else
  std::size_t length = 0;
  for (const wchar_t c : value)
    length += static_cast<unsigned long>(c) > 0xFFFF ? 2 : 1;
  return length;
#endif
}

// JavaScript 的 String.prototype.trim 去掉的字符：空白和行终止符。
inline bool js_whitespace(wchar_t c) {
  return (c >= 0x09 && c <= 0x0D) || c == 0x20 || c == 0xA0 || c == 0x1680 ||
         (c >= 0x2000 && c <= 0x200A) || c == 0x2028 || c == 0x2029 || c == 0x202F ||
         c == 0x205F || c == 0x3000 || c == 0xFEFF;
}

inline std::wstring js_trim(std::wstring_view value) {
  std::size_t begin = 0;
  std::size_t end = value.size();
  while (begin < end && js_whitespace(value[begin]))
    ++begin;
  while (end > begin && js_whitespace(value[end - 1]))
    --end;
  return std::wstring(value.substr(begin, end - begin));
}

// Unicode 的 Cc 类：C0、DEL 和 C1 控制字符。
inline bool control_character(wchar_t c) { return c <= 0x1F || (c >= 0x7F && c <= 0x9F); }

// `^[a-z]+(?:'[a-z]+)*$`：小写字母的音节，用 ' 隔开。
inline bool mention_key_valid(std::wstring_view key) {
  if (key.empty())
    return false;
  bool letter_before = false;
  for (const wchar_t c : key) {
    if (c >= L'a' && c <= L'z') {
      letter_before = true;
    } else if (c == L'\'' && letter_before) {
      letter_before = false;
    } else {
      return false;
    }
  }
  return letter_before;
}

// 名单按现在的样子能不能保存，不能时是页面上的那句话（`mentionListIssue`）。规则与 `mentions::validate_entry` 相同，宿主还会再查一遍。
inline std::optional<std::wstring> mention_list_issue(std::vector<Mention> const &entries) {
  if (entries.size() > max_mentions)
    return L"名单最多 " + std::to_wstring(max_mentions) + L" 条。";
  std::set<std::wstring> seen;
  for (std::size_t index = 0; index < entries.size(); ++index) {
    auto const &entry = entries[index];
    const auto row = L"第 " + std::to_wstring(index + 1) + L" 行";
    const auto text = js_trim(entry.text);
    if (text.empty())
      return row + L"还没有填写名字或地点。";
    if (utf16_length(entry.text) > max_mention_text_utf16)
      return row + L"太长了。";
    if (std::any_of(entry.text.begin(), entry.text.end(), control_character))
      return row + L"含有控制字符。";
    if (!entry.key.empty() &&
        (utf16_length(entry.key) > max_mention_key_bytes || !mention_key_valid(entry.key)))
      return row + L"的拼音只能是小写字母，音节之间用 ' 分隔，例如 zhang'san。";
    // 按保存时的样子比较：保存前每个名字都去掉首尾空白，宿主再查重复。
    if (!seen.insert(text).second)
      return L"「" + text + L"」重复了。";
  }
  return std::nullopt;
}

// 保存时发出去的名单：名字去掉首尾空白，拼音原样。
inline std::vector<Mention> trimmed_mentions(std::vector<Mention> const &entries) {
  std::vector<Mention> trimmed;
  trimmed.reserve(entries.size());
  for (auto const &entry : entries)
    trimmed.push_back({js_trim(entry.text), entry.key});
  return trimmed;
}

// 列表里「@ 名单」入口的说明：有改动没存时说出来，否则是条数，以及 @ 模式没开或方案打不开它。
inline std::wstring mentions_description(bool dirty, std::size_t saved, bool scheme_opens,
                                         bool mention_mode) {
  if (dirty)
    return L"有未保存的修改";
  const std::wstring gap = !scheme_opens ? L"；当前方案打不开 @ 模式" : mention_mode ? L"" : L"；@ 模式未开";
  return std::to_wstring(saved) + L" 条，只保存在本机" + gap;
}

// ---- 插件库 ----

// 「我的插件」照 macOS 的插件库（`plugin-library-view.tsx` 的 `PluginLibraryView`）：已安装计数、搜索、带计数的类型筛选和按类型分节的卡片。Windows 宿主路由 / 指令（`triggers` 为真）也运行脚本插件，所以指令表一类总在，脚本插件排在最后。

// 插件库给类型排的顺序：`kindOrder` 后面接脚本插件。
inline constexpr std::array<std::wstring_view, 9> library_kind_order{
    L"sound", L"effect", L"music", L"command_table", L"phrase_table",
    L"helpcode", L"wordbook", L"symbol_set", L"script"};

// 筛选「全部」的值。
inline constexpr std::wstring_view library_all = L"all";

// 搜索框的查询：按 JavaScript 的 trim 去掉两头空白再转小写（`filter.query.trim().toLocaleLowerCase()`）；为空表示没在搜索。
inline std::wstring library_query(std::wstring_view query) {
  auto result = js_trim(query);
  for (auto &c : result)
    c = static_cast<wchar_t>(std::towlower(c));
  return result;
}

inline bool library_text_has(std::wstring_view text, std::wstring const &query) {
  return library_query(text).find(query) != std::wstring::npos;
}

// 一个已安装的包是否匹配查询：名称、id、作者、描述或类型名里含有它（`matches`）。`query` 是 `library_query` 的结果。
inline bool library_matches(Package const &pack, std::wstring const &query) {
  if (query.empty())
    return true;
  for (auto const *text : {&pack.name, &pack.id, &pack.author, &pack.description})
    if (library_text_has(*text, query))
      return true;
  return library_text_has(kind_label(pack.kind), query);
}

// 缺失的选择是否匹配查询：标题加 id 里含有它。
inline bool library_missing_matches(MissingSelection const &entry, std::wstring const &query) {
  return query.empty() || library_text_has(missing_title(entry) + L" " + entry.id, query);
}

// 某一类已安装的包有几个；`kind` 为 `library_all` 时是全部已安装的包（筛选按钮上的计数，与查询无关）。
inline std::size_t library_count(std::vector<Package> const &packages, std::wstring_view kind) {
  if (kind == library_all)
    return packages.size();
  return static_cast<std::size_t>(std::count_if(packages.begin(), packages.end(),
                                                [kind](Package const &pack) { return pack.kind == kind; }));
}

// 筛选按钮和分节用到的类型（`categories`）：有已安装的包或缺失的选择的类型，加上总在的指令表，按 `library_kind_order` 排。
inline std::vector<std::wstring> library_categories(std::vector<Package> const &packages,
                                                    std::vector<MissingSelection> const &missing) {
  std::vector<std::wstring> categories;
  for (auto const kind : library_kind_order) {
    const bool present =
        kind == L"command_table" ||
        std::any_of(packages.begin(), packages.end(), [kind](Package const &pack) { return pack.kind == kind; }) ||
        std::any_of(missing.begin(), missing.end(),
                    [kind](MissingSelection const &entry) { return entry.kind == kind; });
    if (present)
      categories.emplace_back(kind);
  }
  return categories;
}

// 实际生效的筛选：选中的类型在目录变化后消失了就回到「全部」，不留下没有按钮可取消的筛选（`selectedKind`）。
inline std::wstring library_selected_kind(std::wstring const &kind, std::vector<std::wstring> const &categories) {
  return contains(categories, kind) ? kind : std::wstring(library_all);
}

struct LibraryGroup {
  std::wstring kind;
  std::vector<Package const *> packs;
  std::vector<MissingSelection const *> gone;
};

// 按筛选和查询列出的分节（`groups`）：选中的类型或全部类型里，匹配查询的包和缺失的选择；空的分节略去，只有指令表在没在搜索时仍然列出（它带着 / 指令的说明）。
inline std::vector<LibraryGroup> library_groups(std::vector<Package> const &packages,
                                                std::vector<MissingSelection> const &missing,
                                                std::vector<std::wstring> const &categories,
                                                std::wstring const &selected, std::wstring const &query) {
  std::vector<LibraryGroup> groups;
  for (auto const &kind : categories) {
    if (selected != library_all && selected != kind)
      continue;
    LibraryGroup group{kind, {}, {}};
    for (auto const &pack : packages)
      if (pack.kind == kind && library_matches(pack, query))
        group.packs.push_back(&pack);
    for (auto const &entry : missing)
      if (entry.kind == kind && library_missing_matches(entry, query))
        group.gone.push_back(&entry);
    if (!group.packs.empty() || !group.gone.empty() || (kind == L"command_table" && query.empty()))
      groups.push_back(std::move(group));
  }
  return groups;
}

// 分节标题：脚本插件写「脚本插件（实验）」，其余是类型名。
inline std::wstring library_section_title(std::wstring_view kind) {
  return kind == L"script" ? std::wstring(L"脚本插件（实验）") : kind_label(kind);
}

// 读到目录却没有可列的分节时的一句：在搜索时请用户换个关键词。
inline std::wstring library_empty_message(std::wstring const &query) {
  return query.empty() ? L"没有插件" : L"没有匹配的插件，试试其他关键词。";
}

// 指令表分节的说明（/ 指令怎么用、启用顺序和上限）。
inline std::wstring command_table_note() {
  return L"在「输入 → 快捷模式」打开 / 指令后，按 / 再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用 " +
         std::to_wstring(max_command_tables) + L" 个。";
}

} // namespace msime::settings::plugins
