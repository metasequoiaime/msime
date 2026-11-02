#pragma once
#include "PluginPageModel.h"

// 设置页插件编辑器的规则和文案，照共享设置的 `plugin-editor-model.ts` 逐条移植：可编辑的四种类型和说明、默认许可证与版本、草稿与交给宿主的 AuthoredPack 互转、保存前的第一处问题、批量粘贴的读法，以及保存后详情顶上的提醒。上限与 `crates/client-services/src/plugins/` 的清单校验一致，宿主保存时按同一套规则再查一遍，这里只让用户在提交之前就知道该改哪一行。JSON 的读写留在各自的窗口里（现代设置用 WinRT 的 JsonObject，Win7 设置用 nlohmann::json），这里只有两边共用的结构。另有 Win7 设置用的整段文本写法（每种内容一个多行文本框），新建和编辑走同样的草稿与校验。不依赖 Windows 和 WinRT 头文件，方便单独测试。
namespace msime::settings::plugins {

// ---- 类型 ----

struct AuthoredKindInfo {
  std::wstring_view kind, title, description;
};

// 编辑器能写的类型，按「新建插件」里列出的顺序，每种带一句给不读文档的人看的说明（`AUTHORED_KINDS`）。
inline constexpr std::array<AuthoredKindInfo, 4> authored_kinds{{
    {L"phrase_table", L"短语表", L"在 K 模式里输入编码，打出一段常用文字，例如输入 dz 打出家庭地址。"},
    {L"command_table", L"指令表", L"按 / 再输入指令字母，插入签名、当天日期这类模板文字。"},
    {L"symbol_set", L"符号集", L"给符号面板加几组自己常用的符号或颜文字。"},
    {L"wordbook", L"单词本", L"做一本自己的词书，在背单词里复习。"},
}};

// 「新建插件」旁边的一句说明（`CREATE_DESCRIPTION`）。
inline constexpr std::wstring_view create_description =
    L"不用写代码，填好短语、指令、符号或单词就能做出自己的插件，做好后也可以发布到社区";

// 这个类型能不能在设置页编辑（`isAuthoredKind`）。
inline bool is_authored_kind(std::wstring_view kind) {
  return std::any_of(authored_kinds.begin(), authored_kinds.end(),
                     [kind](AuthoredKindInfo const &entry) { return entry.kind == kind; });
}

// 详情里「编辑」的条件：四种文本类型的用户包；内置和官方包由宿主拒绝，这里按 `builtin` 先不给。
inline bool pack_editable(Package const &pack) { return !pack.builtin && is_authored_kind(pack.kind); }

inline constexpr std::wstring_view default_license = L"CC0-1.0";
inline constexpr std::wstring_view default_version = L"1.0.0";
// 许可证输入框下面的说明。
inline constexpr std::wstring_view license_note =
    L"许可证决定别人能怎样使用这个插件。CC0-1.0 表示放弃全部权利，任何人都能随意使用和改编；想让别人注明出自你，可以写 CC-BY-4.0。这里只能写 SPDX 许可证代码。";
// 短语表的光标标记由共享层解释，设置页只说明写法。
inline constexpr std::wstring_view phrase_cursor_note =
    LR"(在文字中用 ${cursor} 指定上屏后的光标位置，例如 title:"${cursor}" 会输入 title:""，光标停在引号内。只定位第一个标记；要输入标记本身，请写 $${cursor}。编辑器不支持定位时光标留在末尾。)";
// 词库页编辑用户快捷短语时的说明：同一套标记，另外提醒升级前存过字面 ${cursor} 的短语要改写。
inline constexpr std::wstring_view quick_phrase_cursor_note =
    LR"(在文字中用 ${cursor} 指定上屏后的光标位置，例如 title:"${cursor}" 会输入 title:""，光标停在引号内。只定位第一个标记；要输入标记本身，请写 $${cursor}。编辑器不支持定位时光标留在末尾。以前存过、要照原样输入 ${cursor} 的短语，请改写成 $${cursor}。)";
// 保存按钮上面的说明：编辑替换本机的这一份，新建装进本机。
inline std::wstring_view save_note(bool editing) {
  return editing ? L"保存后替换本机的这一份。从社区装来的插件改过之后就是你自己的副本，原作者那边不受影响。"
                 : L"保存后装进本机，和导入的插件一样出现在「我的插件」里，也可以在「社区插件」里发布给别人。";
}

// 以下上限与 client-services 的清单校验一致。
inline constexpr std::size_t max_name_bytes = 80;
inline constexpr std::size_t max_version_bytes = 32;
inline constexpr std::size_t max_license_bytes = 64;
inline constexpr std::size_t max_author_bytes = 120;
inline constexpr std::size_t max_description_bytes = 500;
// `phrase_table::MAX_PHRASES`。
inline constexpr std::size_t max_phrases = 2000;
// `command_table::MAX_COMMANDS`。
inline constexpr std::size_t max_commands = 256;
inline constexpr std::size_t max_key_letters = 32;
// 短语文字和指令模板的 UTF-16 码元上限：Windows 候选管道的文本字段。
inline constexpr std::size_t max_text_utf16 = 199;
inline constexpr std::size_t max_command_title_bytes = 48;
// `symbol_set::MAX_GROUPS`。
inline constexpr std::size_t max_groups = 32;
inline constexpr std::size_t max_group_items = 512;
inline constexpr std::size_t max_symbol_items = 2048;
inline constexpr std::size_t max_group_title_bytes = 48;
inline constexpr std::size_t max_keywords_bytes = 256;
inline constexpr std::size_t max_item_utf16 = 64;
// `vocabulary::wordbook::MAX_ENTRIES`。
inline constexpr std::size_t max_words = 20000;
inline constexpr std::size_t max_word_chars = 64;
inline constexpr std::size_t max_phonetic_chars = 64;
inline constexpr std::size_t max_gloss_chars = 256;
inline constexpr std::size_t max_wordbook_name_chars = 64;
// 长列表一次画出的行数：单词本可以有两万行，全部画出来会让页面卡住（`ROW_PAGE`）。
inline constexpr std::size_t editor_row_page = 100;

// ---- 文本度量 ----

// UTF-8 字节数，即 `TextEncoder` 编出的长度。UTF-16 的代理对两半各算 2，合起来是一个 4 字节的码点。
inline std::size_t utf8_length(std::wstring_view value) {
  std::size_t length = 0;
  for (const wchar_t c : value) {
    const auto code = static_cast<unsigned long>(c);
    if (code < 0x80)
      length += 1;
    else if (code < 0x800)
      length += 2;
    else if (code >= 0xD800 && code <= 0xDFFF)
      length += 2;
    // 与 `utf16_length` 一样用预处理分开：16 位 wchar_t 上 `> 0xFFFF` 恒假，MinGW 的 -Werror=type-limits 会报错。
#if WCHAR_MAX <= 0xFFFF
    else
      length += 3;
#else
    else
      length += code > 0xFFFF ? 4 : 3;
#endif
  }
  return length;
}

// 码点数，即 `[...value].length`：UTF-16 的代理对算一个。
inline std::size_t code_points(std::wstring_view value) {
  return static_cast<std::size_t>(std::count_if(value.begin(), value.end(), [](wchar_t c) {
    const auto code = static_cast<unsigned long>(c);
    return code < 0xDC00 || code > 0xDFFF;
  }));
}

inline bool blank_text(std::wstring_view value) { return js_trim(value).empty(); }

inline bool has_control(std::wstring_view value) {
  return std::any_of(value.begin(), value.end(), control_character);
}

// `^[a-z]+$`。
inline bool lowercase_letters(std::wstring_view value) {
  return !value.empty() && std::all_of(value.begin(), value.end(), [](wchar_t c) { return c >= L'a' && c <= L'z'; });
}

// `^[A-Za-z0-9.\-+() ]+$`：SPDX 许可证表达式能用到的字符。
inline bool license_characters(std::wstring_view value) {
  return !value.empty() && std::all_of(value.begin(), value.end(), [](wchar_t c) {
    return (c >= L'a' && c <= L'z') || (c >= L'A' && c <= L'Z') || (c >= L'0' && c <= L'9') ||
           c == L'.' || c == L'-' || c == L'+' || c == L'(' || c == L')' || c == L' ';
  });
}

// 按 `\r?\n` 切成行，与 `text.split(/\r?\n/)` 相同：最后一个换行之后的空串也算一行。
inline std::vector<std::wstring> split_lines(std::wstring_view text) {
  std::vector<std::wstring> lines;
  std::size_t start = 0;
  for (;;) {
    const auto end = text.find(L'\n', start);
    auto line = text.substr(start, end == std::wstring_view::npos ? std::wstring_view::npos : end - start);
    if (end != std::wstring_view::npos && !line.empty() && line.back() == L'\r')
      line.remove_suffix(1);
    lines.emplace_back(line);
    if (end == std::wstring_view::npos)
      return lines;
    start = end + 1;
  }
}

// ---- 草稿与交给宿主的包 ----

// 编辑中的单词：音标留空时就是空串。
struct DraftWord {
  std::wstring word, phonetic, gloss;
};

// 编辑中的一组符号：`items` 是文本框里的原文，一行一项。
struct DraftGroup {
  std::wstring tab = L"symbols";
  std::wstring title, keywords, items;
};

// 编辑器的草稿：全部字段都是表单里的原文，保存前由 `pack_from_draft` 整理。四种内容都在，只有与 `kind` 对应的那一种会保存。`id` 在编辑已安装的包时是它的 id，新建时为空，由宿主生成。
struct AuthoringDraft {
  std::wstring kind;
  std::optional<std::wstring> id;
  std::wstring name, version, license, author, description;
  std::vector<Phrase> phrases;
  std::vector<Command> commands;
  std::vector<DraftGroup> groups;
  std::vector<DraftWord> words;

  // 有没有改动（共享页比较两份草稿的 JSON）。
  bool operator==(AuthoringDraft const &other) const {
    const auto same_phrases = std::equal(phrases.begin(), phrases.end(), other.phrases.begin(), other.phrases.end(),
                                         [](Phrase const &a, Phrase const &b) { return a.key == b.key && a.text == b.text; });
    const auto same_commands =
        std::equal(commands.begin(), commands.end(), other.commands.begin(), other.commands.end(),
                   [](Command const &a, Command const &b) {
                     return a.trigger == b.trigger && a.title == b.title && a.template_text == b.template_text;
                   });
    const auto same_groups =
        std::equal(groups.begin(), groups.end(), other.groups.begin(), other.groups.end(),
                   [](DraftGroup const &a, DraftGroup const &b) {
                     return a.tab == b.tab && a.title == b.title && a.keywords == b.keywords && a.items == b.items;
                   });
    const auto same_words =
        std::equal(words.begin(), words.end(), other.words.begin(), other.words.end(),
                   [](DraftWord const &a, DraftWord const &b) {
                     return a.word == b.word && a.phonetic == b.phonetic && a.gloss == b.gloss;
                   });
    return kind == other.kind && id == other.id && name == other.name && version == other.version &&
           license == other.license && author == other.author && description == other.description && same_phrases &&
           same_commands && same_groups && same_words;
  }
  bool operator!=(AuthoringDraft const &other) const { return !(*this == other); }
};

// `client-services::plugins::authoring` 的 AuthoredWord、AuthoredGroup 和 AuthoredPack：可选的字段为空时在 JSON 里是 null；四个内容字段只发与 `kind` 对应的那一个。
struct AuthoredWord {
  std::wstring word;
  std::optional<std::wstring> phonetic;
  std::wstring gloss;
};

struct AuthoredGroup {
  std::wstring tab, title;
  std::optional<std::wstring> keywords;
  std::vector<std::wstring> items;
};

struct AuthoredPack {
  std::wstring kind;
  std::optional<std::wstring> id;
  std::wstring name, version, license;
  std::optional<std::wstring> author, description;
  std::vector<Phrase> phrases;
  std::vector<Command> commands;
  std::vector<AuthoredGroup> groups;
  std::vector<AuthoredWord> words;
};

inline bool blank_phrase(Phrase const &row) { return blank_text(row.key) && blank_text(row.text); }
inline bool blank_command(Command const &row) {
  return blank_text(row.trigger) && blank_text(row.title) && blank_text(row.template_text);
}
inline bool blank_group(DraftGroup const &group) {
  return blank_text(group.title) && blank_text(group.keywords) && blank_text(group.items);
}
inline bool blank_word(DraftWord const &row) { return blank_text(row.word) && blank_text(row.phonetic) && blank_text(row.gloss); }

// 新建一种插件时的草稿：默认许可证和版本，内容先给一个空行（`emptyDraft`）。
inline AuthoringDraft empty_draft(std::wstring_view kind) {
  AuthoringDraft draft;
  draft.kind = std::wstring(kind);
  draft.version = std::wstring(default_version);
  draft.license = std::wstring(default_license);
  if (kind == L"phrase_table")
    draft.phrases.push_back({});
  else if (kind == L"command_table")
    draft.commands.push_back({});
  else if (kind == L"symbol_set")
    draft.groups.push_back({});
  else if (kind == L"wordbook")
    draft.words.push_back({});
  return draft;
}

inline std::wstring joined_lines(std::vector<std::wstring> const &lines, std::wstring_view separator) {
  std::wstring text;
  for (std::size_t i = 0; i < lines.size(); ++i) {
    if (i > 0)
      text += separator;
    text += lines[i];
  }
  return text;
}

// 宿主读出的已安装包，变成可以编辑的草稿（`draftFromPack`）：符号一行一项。
inline AuthoringDraft draft_from_pack(AuthoredPack const &pack) {
  AuthoringDraft draft;
  draft.kind = pack.kind;
  draft.id = pack.id;
  draft.name = pack.name;
  draft.version = pack.version;
  draft.license = pack.license;
  draft.author = pack.author.value_or(L"");
  draft.description = pack.description.value_or(L"");
  draft.phrases = pack.phrases;
  draft.commands = pack.commands;
  for (auto const &group : pack.groups)
    draft.groups.push_back({group.tab, group.title, group.keywords.value_or(L""), joined_lines(group.items, L"\n")});
  for (auto const &entry : pack.words)
    draft.words.push_back({entry.word, entry.phonetic.value_or(L""), entry.gloss});
  return draft;
}

// 文本框里的符号：一行一项，去掉首尾空白，跳过空行（`groupItems`）。
inline std::vector<std::wstring> group_items(std::wstring_view text) {
  std::vector<std::wstring> items;
  for (auto const &line : split_lines(text)) {
    auto item = js_trim(line);
    if (!item.empty())
      items.push_back(std::move(item));
  }
  return items;
}

inline std::optional<std::wstring> optional_text(std::wstring_view value) {
  auto trimmed = js_trim(value);
  return trimmed.empty() ? std::nullopt : std::make_optional(std::move(trimmed));
}

// 草稿整理成交给宿主的包：各字段去掉首尾空白，可选字段留空时为空，整行都空着的行不保存（`packFromDraft`）。
inline AuthoredPack pack_from_draft(AuthoringDraft const &draft) {
  AuthoredPack pack;
  pack.kind = draft.kind;
  pack.id = draft.id;
  pack.name = js_trim(draft.name);
  pack.version = js_trim(draft.version);
  pack.license = js_trim(draft.license);
  pack.author = optional_text(draft.author);
  pack.description = optional_text(draft.description);
  if (draft.kind == L"phrase_table") {
    for (auto const &row : draft.phrases)
      if (!blank_phrase(row))
        pack.phrases.push_back({js_trim(row.key), js_trim(row.text)});
  } else if (draft.kind == L"command_table") {
    for (auto const &row : draft.commands)
      if (!blank_command(row))
        pack.commands.push_back({js_trim(row.trigger), js_trim(row.title), js_trim(row.template_text)});
  } else if (draft.kind == L"symbol_set") {
    for (auto const &group : draft.groups)
      if (!blank_group(group))
        pack.groups.push_back({group.tab, js_trim(group.title), optional_text(group.keywords), group_items(group.items)});
  } else if (draft.kind == L"wordbook") {
    for (auto const &row : draft.words)
      if (!blank_word(row))
        pack.words.push_back({js_trim(row.word), optional_text(row.phonetic), js_trim(row.gloss)});
  }
  return pack;
}

// ---- 保存前的检查 ----

// 指令模板里的占位符是否都认得：`{date}`、`{time}`（都可以带 `:格式`）和 `{weekday}`，花括号成对且不嵌套。格式本身对不对、展开后会不会太长由宿主检查。
inline bool template_placeholders_known(std::wstring_view rest) {
  for (;;) {
    const auto open = rest.find_first_of(L"{}");
    if (open == std::wstring_view::npos)
      return true;
    if (rest[open] == L'}')
      return false;
    const auto after = rest.substr(open + 1);
    const auto close = after.find_first_of(L"{}");
    if (close == std::wstring_view::npos || after[close] == L'{')
      return false;
    const auto placeholder = after.substr(0, close);
    const bool dated = (placeholder.substr(0, 4) == L"date" || placeholder.substr(0, 4) == L"time") &&
                       (placeholder.size() == 4 || placeholder[4] == L':');
    if (placeholder != L"weekday" && !dated)
      return false;
    rest = after.substr(close + 1);
  }
}

inline std::optional<std::wstring> phrases_issue(std::vector<Phrase> const &rows) {
  std::set<std::wstring> seen;
  std::size_t count = 0;
  for (std::size_t index = 0; index < rows.size(); ++index) {
    auto const &row = rows[index];
    if (blank_phrase(row))
      continue;
    ++count;
    const auto at = L"第 " + std::to_wstring(index + 1) + L" 条短语";
    const auto key = js_trim(row.key);
    const auto text = js_trim(row.text);
    if (key.empty())
      return at + L"还没有填写编码。";
    if (utf16_length(key) > max_key_letters || !lowercase_letters(key))
      return at + L"的编码只能是 1 到 " + std::to_wstring(max_key_letters) + L" 个小写字母。";
    if (text.empty())
      return at + L"还没有填写文字。";
    if (utf16_length(text) > max_text_utf16)
      return at + L"的文字太长了。";
    if (has_control(text))
      return at + L"的文字里不能有换行或制表符。";
    if (!seen.insert(key + L"\n" + text).second)
      return at + L"和前面的一条重复了。";
  }
  if (count == 0)
    return std::wstring(L"至少要有一条短语。");
  if (count > max_phrases)
    return L"一个短语表最多 " + std::to_wstring(max_phrases) + L" 条。";
  return std::nullopt;
}

inline std::optional<std::wstring> commands_issue(std::vector<Command> const &rows) {
  std::set<std::wstring> triggers;
  std::size_t count = 0;
  for (std::size_t index = 0; index < rows.size(); ++index) {
    auto const &row = rows[index];
    if (blank_command(row))
      continue;
    ++count;
    const auto at = L"第 " + std::to_wstring(index + 1) + L" 条指令";
    const auto trigger = js_trim(row.trigger);
    const auto title = js_trim(row.title);
    const auto template_text = js_trim(row.template_text);
    if (trigger.empty())
      return at + L"还没有填写指令字母。";
    if (utf16_length(trigger) > max_key_letters || !lowercase_letters(trigger))
      return at + L"的指令只能是 1 到 " + std::to_wstring(max_key_letters) + L" 个小写字母。";
    if (!triggers.insert(trigger).second)
      return L"指令 /" + trigger + L" 重复了。";
    if (title.empty())
      return at + L"还没有填写标题。";
    if (utf8_length(title) > max_command_title_bytes || has_control(title))
      return at + L"的标题太长了。";
    if (template_text.empty())
      return at + L"还没有填写要插入的文字。";
    if (utf16_length(template_text) > max_text_utf16)
      return at + L"要插入的文字太长了。";
    if (has_control(template_text))
      return at + L"要插入的文字里不能有换行或制表符。";
    if (!template_placeholders_known(template_text))
      return at + L"的花括号里只能写 {date}、{time} 或 {weekday}。";
  }
  if (count == 0)
    return std::wstring(L"至少要有一条指令。");
  if (count > max_commands)
    return L"一个指令表最多 " + std::to_wstring(max_commands) + L" 条。";
  return std::nullopt;
}

inline std::optional<std::wstring> groups_issue(std::vector<DraftGroup> const &groups) {
  std::set<std::wstring> titles;
  std::size_t count = 0;
  std::size_t total = 0;
  for (std::size_t index = 0; index < groups.size(); ++index) {
    auto const &group = groups[index];
    if (blank_group(group))
      continue;
    ++count;
    const auto at = L"第 " + std::to_wstring(index + 1) + L" 组";
    const auto title = js_trim(group.title);
    if (title.empty())
      return at + L"还没有填写标题。";
    if (utf8_length(title) > max_group_title_bytes || has_control(title))
      return at + L"的标题太长了。";
    if (!titles.insert(group.tab + L"\n" + title).second)
      return at + L"的标题和同一页的另一组重复了。";
    if (utf8_length(js_trim(group.keywords)) > max_keywords_bytes || has_control(group.keywords))
      return at + L"的搜索关键词太长了。";
    const auto items = group_items(group.items);
    if (items.empty())
      return at + L"还没有符号。";
    if (items.size() > max_group_items)
      return at + L"最多 " + std::to_wstring(max_group_items) + L" 项。";
    total += items.size();
    std::set<std::wstring> seen;
    for (auto const &item : items) {
      if (utf16_length(item) > max_item_utf16 || has_control(item))
        return at + L"的「" + item + L"」太长了。";
      if (!seen.insert(item).second)
        return at + L"里「" + item + L"」重复了。";
    }
  }
  if (count == 0)
    return std::wstring(L"至少要有一组符号。");
  if (count > max_groups)
    return L"一个符号集最多 " + std::to_wstring(max_groups) + L" 组。";
  if (total > max_symbol_items)
    return L"一个符号集合计最多 " + std::to_wstring(max_symbol_items) + L" 项。";
  return std::nullopt;
}

inline std::optional<std::wstring> words_issue(std::vector<DraftWord> const &rows) {
  std::set<std::wstring> seen;
  std::size_t count = 0;
  for (std::size_t index = 0; index < rows.size(); ++index) {
    auto const &row = rows[index];
    if (blank_word(row))
      continue;
    ++count;
    const auto at = L"第 " + std::to_wstring(index + 1) + L" 个单词";
    const auto word = js_trim(row.word);
    const auto phonetic = js_trim(row.phonetic);
    const auto gloss = js_trim(row.gloss);
    if (word.empty())
      return at + L"还没有填写单词。";
    if (code_points(word) > max_word_chars || has_control(word))
      return at + L"太长了。";
    if (word.front() == L'#')
      return at + L"不能以 # 开头。";
    if (code_points(phonetic) > max_phonetic_chars || has_control(phonetic))
      return at + L"的音标太长了。";
    if (gloss.empty())
      return at + L"还没有填写释义。";
    if (code_points(gloss) > max_gloss_chars || has_control(gloss))
      return at + L"的释义太长了。";
    if (!seen.insert(word).second)
      return L"「" + word + L"」出现了不止一次。";
  }
  if (count == 0)
    return std::wstring(L"至少要有一个单词。");
  if (count > max_words)
    return L"一本单词本最多 " + std::to_wstring(max_words) + L" 个单词。";
  return std::nullopt;
}

// 草稿现在不能保存的第一个原因，用页面上的说法；能保存时为空（`draftIssue`）。规则与宿主的清单校验一致，行号按表单里的行数（整行都空着的行不算问题）。
inline std::optional<std::wstring> draft_issue(AuthoringDraft const &draft) {
  const auto name = js_trim(draft.name);
  if (name.empty())
    return std::wstring(L"请给插件起个名字。");
  if (utf8_length(name) > max_name_bytes || has_control(name))
    return std::wstring(L"名字太长了。");
  if (draft.kind == L"wordbook" && code_points(name) > max_wordbook_name_chars)
    return L"单词本的名字不能超过 " + std::to_wstring(max_wordbook_name_chars) + L" 个字。";
  const auto version = js_trim(draft.version);
  if (version.empty())
    return std::wstring(L"请填写版本，例如 1.0.0。");
  if (utf8_length(version) > max_version_bytes || has_control(version))
    return std::wstring(L"版本太长了。");
  const auto license = js_trim(draft.license);
  if (license.empty())
    return L"请填写许可证，不确定时写 " + std::wstring(default_license) + L"。";
  if (utf8_length(license) > max_license_bytes || !license_characters(license))
    return std::wstring(L"许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。");
  if (utf8_length(js_trim(draft.author)) > max_author_bytes || has_control(draft.author))
    return std::wstring(L"作者太长了。");
  if (utf8_length(js_trim(draft.description)) > max_description_bytes || has_control(draft.description))
    return std::wstring(L"介绍太长了。");
  if (draft.kind == L"phrase_table")
    return phrases_issue(draft.phrases);
  if (draft.kind == L"command_table")
    return commands_issue(draft.commands);
  if (draft.kind == L"symbol_set")
    return groups_issue(draft.groups);
  if (draft.kind == L"wordbook")
    return words_issue(draft.words);
  return std::nullopt;
}

// ---- 批量粘贴 ----

// 批量粘贴的结果：能读出的行，以及第一行读不出的原因（`BulkRows`）。
template <typename T> struct BulkRows {
  std::vector<T> rows;
  std::optional<std::wstring> issue;
};

// JavaScript 正则里 `.` 不匹配的行终止符。
inline bool line_terminator(wchar_t c) { return c == L'\n' || c == L'\r' || c == 0x2028 || c == 0x2029; }

// 一行短语按 `^\s*(\S+)\s+(.+?)\s*$` 读成编码和文字；读不出时为空。
inline std::optional<Phrase> phrase_line(std::wstring_view line) {
  std::size_t at = 0;
  while (at < line.size() && js_whitespace(line[at]))
    ++at;
  const auto key_start = at;
  while (at < line.size() && !js_whitespace(line[at]))
    ++at;
  if (at == key_start || at == line.size())
    return std::nullopt;
  const auto key = line.substr(key_start, at - key_start);
  const auto rest = line.substr(at);
  std::size_t start = 0;
  while (start < rest.size() && js_whitespace(rest[start]))
    ++start;
  auto end = rest.size();
  while (end > start && js_whitespace(rest[end - 1]))
    --end;
  if (end > start) {
    const auto text = rest.substr(start, end - start);
    if (std::any_of(text.begin(), text.end(), line_terminator))
      return std::nullopt;
    return Phrase{std::wstring(key), std::wstring(text)};
  }
  // 编码后面只有空白时，`\s+` 至少留一个，`(.+?)` 取它后面最靠后的一个不是行终止符的空白字符。
  for (auto j = rest.size(); j-- > 1;)
    if (!line_terminator(rest[j]))
      return Phrase{std::wstring(key), std::wstring(1, rest[j])};
  return std::nullopt;
}

// 批量添加短语：每行「编码 文字」，编码和文字之间用空格或 Tab 隔开，文字里可以再有空格；空行跳过（`parsePhraseLines`）。
inline BulkRows<Phrase> parse_phrase_lines(std::wstring_view text) {
  BulkRows<Phrase> result;
  const auto lines = split_lines(text);
  for (std::size_t index = 0; index < lines.size(); ++index) {
    if (blank_text(lines[index]))
      continue;
    auto row = phrase_line(lines[index]);
    if (!row) {
      result.issue = L"第 " + std::to_wstring(index + 1) + L" 行要写成「编码 文字」，中间用空格隔开。";
      return result;
    }
    result.rows.push_back(std::move(*row));
  }
  return result;
}

inline std::vector<std::wstring> tab_columns(std::wstring_view line) {
  std::vector<std::wstring> columns;
  std::size_t start = 0;
  for (;;) {
    const auto end = line.find(L'\t', start);
    columns.emplace_back(line.substr(start, end == std::wstring_view::npos ? std::wstring_view::npos : end - start));
    if (end == std::wstring_view::npos)
      return columns;
    start = end + 1;
  }
}

// 批量添加单词：每行「单词 Tab 释义」或「单词 Tab 音标 Tab 释义」，从表格复制过来就是这样；空行和 # 开头的行跳过，与单词本的词表文件同一种写法（`parseWordLines`）。
inline BulkRows<DraftWord> parse_word_lines(std::wstring_view text) {
  BulkRows<DraftWord> result;
  const auto lines = split_lines(text);
  for (std::size_t index = 0; index < lines.size(); ++index) {
    auto const &line = lines[index];
    if (blank_text(line) || line.front() == L'#')
      continue;
    const auto columns = tab_columns(line);
    if (columns.size() == 2) {
      result.rows.push_back({columns[0], L"", columns[1]});
    } else if (columns.size() == 3) {
      result.rows.push_back({columns[0], columns[1], columns[2]});
    } else {
      result.issue = L"第 " + std::to_wstring(index + 1) +
                     L" 行要写成「单词、释义」两列或「单词、音标、释义」三列，列之间用 Tab 隔开。";
      return result;
    }
  }
  return result;
}

// 把批量读出的行接到已有的行后面；已有的只剩一个空行时直接替换它（`appendRows`）。
template <typename T, typename Blank>
std::vector<T> append_rows(std::vector<T> const &current, std::vector<T> const &added, Blank is_blank) {
  std::vector<T> rows;
  if (!(current.size() == 1 && is_blank(current.front())))
    rows = current;
  rows.insert(rows.end(), added.begin(), added.end());
  return rows;
}

// ---- 保存之后 ----

// 刚保存的包详情页顶上的一句话：短语表和指令表还要打开「启用」，K 模式或 / 指令关着时也要打开，别的类型装上就能用（`authoredSaveNote`）。`enabled` 是这个表已经启用，`mode_on` 是对应的快捷模式开着。
inline std::wstring authored_save_note(std::wstring_view kind, bool enabled, bool mode_on) {
  if (kind == L"phrase_table") {
    if (enabled)
      return mode_on ? L"已保存，修改已经生效。"
                     : L"已保存。快捷短语（K 模式）现在关着，在「输入 → 快捷模式」打开后才会出候选。";
    return mode_on ? L"已保存。打开下面的「启用」后，按 Shift+K 再输入编码就能用到这些短语。"
                   : L"已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开快捷短语（K 模式），这些短语才会出候选。";
  }
  if (kind == L"command_table") {
    if (enabled)
      return mode_on ? L"已保存，修改已经生效。" : L"已保存。/ 指令现在关着，在「输入 → 快捷模式」打开后才能用。";
    return mode_on ? L"已保存。打开下面的「启用」后，按 / 再输入指令字母就能用。"
                   : L"已保存。打开下面的「启用」，再在「输入 → 快捷模式」打开 / 指令，这些指令才能用。";
  }
  if (kind == L"symbol_set")
    return L"已保存，符号面板里会显示这些符号。";
  if (kind == L"wordbook")
    return L"已保存，这本书出现在背单词的词书里。";
  return L"已保存。";
}

// 保存成功后的一句（`saveAuthored` 的 `setNotice`）。
inline std::wstring saved_notice(Package const &pack) {
  return L"已保存" + pack_kind_label(pack) + L"「" + pack.name + L"」。";
}

// ---- Win7 设置的整段文本 ----

// Win7 设置没有逐行的表单，每种内容是一个多行文本框：短语和单词用上面批量粘贴的写法，指令是 Tab 隔开的三列，符号集用「[符号] 标题」或「[颜文字] 标题」开始一组（标题后可以按 Tab 写搜索关键词），下面每行一个符号。读出的行放进草稿，再按 `draft_issue` 检查，与现代设置同一套规则。写回文本框时用 `\r\n` 分行，Win32 的多行编辑框只认它。

inline constexpr std::wstring_view symbols_header = L"[符号]";
inline constexpr std::wstring_view kaomoji_header = L"[颜文字]";

// 这种内容在文本框里怎么写。
inline std::wstring_view authored_text_format(std::wstring_view kind) {
  if (kind == L"phrase_table")
    return L"每行一条短语：编码在前，文字在后，中间用空格隔开，例如 dz 北京市海淀区。";
  if (kind == L"command_table")
    return L"每行一条指令：指令字母、标题、要插入的文字三列，列之间用 Tab 隔开，可以从表格复制过来。要插入的文字里可以写 {date}、{time} 和 {weekday}。";
  if (kind == L"symbol_set")
    return L"一行「[符号] 标题」或「[颜文字] 标题」开始一组，标题后面可以按 Tab 再写搜索关键词；下面每行一个符号。";
  if (kind == L"wordbook")
    return L"每行一个单词：单词、释义两列，或单词、音标、释义三列，列之间用 Tab 隔开，可以从表格复制过来。";
  return {};
}

inline std::wstring phrase_text(std::vector<Phrase> const &rows) {
  std::vector<std::wstring> lines;
  for (auto const &row : rows)
    if (!blank_phrase(row))
      lines.push_back(row.key + L" " + row.text);
  return joined_lines(lines, L"\r\n");
}

inline std::wstring command_text(std::vector<Command> const &rows) {
  std::vector<std::wstring> lines;
  for (auto const &row : rows)
    if (!blank_command(row))
      lines.push_back(row.trigger + L"\t" + row.title + L"\t" + row.template_text);
  return joined_lines(lines, L"\r\n");
}

inline std::wstring group_text(std::vector<DraftGroup> const &groups) {
  std::vector<std::wstring> lines;
  for (auto const &group : groups) {
    if (blank_group(group))
      continue;
    auto header = std::wstring(group.tab == L"kaomoji" ? kaomoji_header : symbols_header) + L" " + group.title;
    if (!blank_text(group.keywords))
      header += L"\t" + group.keywords;
    lines.push_back(std::move(header));
    for (auto const &item : group_items(group.items))
      lines.push_back(item);
  }
  return joined_lines(lines, L"\r\n");
}

inline std::wstring word_text(std::vector<DraftWord> const &rows) {
  std::vector<std::wstring> lines;
  for (auto const &row : rows)
    if (!blank_word(row))
      lines.push_back(row.phonetic.empty() ? row.word + L"\t" + row.gloss
                                           : row.word + L"\t" + row.phonetic + L"\t" + row.gloss);
  return joined_lines(lines, L"\r\n");
}

// 草稿的内容写成文本框里的样子。
inline std::wstring authored_content_text(AuthoringDraft const &draft) {
  if (draft.kind == L"phrase_table")
    return phrase_text(draft.phrases);
  if (draft.kind == L"command_table")
    return command_text(draft.commands);
  if (draft.kind == L"symbol_set")
    return group_text(draft.groups);
  if (draft.kind == L"wordbook")
    return word_text(draft.words);
  return {};
}

// 每行「指令 Tab 标题 Tab 要插入的文字」；空行跳过。
inline BulkRows<Command> parse_command_lines(std::wstring_view text) {
  BulkRows<Command> result;
  const auto lines = split_lines(text);
  for (std::size_t index = 0; index < lines.size(); ++index) {
    if (blank_text(lines[index]))
      continue;
    const auto columns = tab_columns(lines[index]);
    if (columns.size() != 3) {
      result.issue = L"第 " + std::to_wstring(index + 1) + L" 行要写成「指令、标题、要插入的文字」三列，列之间用 Tab 隔开。";
      return result;
    }
    result.rows.push_back({columns[0], columns[1], columns[2]});
  }
  return result;
}

// 「[符号] 标题」「[颜文字] 标题」开始一组，标题后 Tab 隔开的是搜索关键词；其余非空行是这一组的符号。空行跳过。
inline BulkRows<DraftGroup> parse_group_lines(std::wstring_view text) {
  BulkRows<DraftGroup> result;
  const auto lines = split_lines(text);
  for (std::size_t index = 0; index < lines.size(); ++index) {
    const auto line = js_trim(lines[index]);
    if (line.empty())
      continue;
    const bool symbols = line.substr(0, symbols_header.size()) == symbols_header;
    const bool kaomoji = line.substr(0, kaomoji_header.size()) == kaomoji_header;
    if (symbols || kaomoji) {
      const auto rest = std::wstring_view(line).substr((symbols ? symbols_header : kaomoji_header).size());
      const auto tab = rest.find(L'\t');
      DraftGroup group;
      group.tab = symbols ? L"symbols" : L"kaomoji";
      group.title = js_trim(rest.substr(0, tab));
      if (tab != std::wstring_view::npos)
        group.keywords = js_trim(rest.substr(tab + 1));
      result.rows.push_back(std::move(group));
      continue;
    }
    if (result.rows.empty()) {
      result.issue = L"第 " + std::to_wstring(index + 1) + L" 行之前还没有组标题，先写一行「[符号] 标题」或「[颜文字] 标题」。";
      return result;
    }
    auto &items = result.rows.back().items;
    items += (items.empty() ? L"" : L"\n") + line;
  }
  return result;
}

// 文本框里的内容读进草稿，替换这种内容原来的行；读不出时是第一行的原因，草稿不动。
inline std::optional<std::wstring> read_authored_content(AuthoringDraft &draft, std::wstring_view text) {
  if (draft.kind == L"phrase_table") {
    auto parsed = parse_phrase_lines(text);
    if (!parsed.issue)
      draft.phrases = std::move(parsed.rows);
    return parsed.issue;
  }
  if (draft.kind == L"command_table") {
    auto parsed = parse_command_lines(text);
    if (!parsed.issue)
      draft.commands = std::move(parsed.rows);
    return parsed.issue;
  }
  if (draft.kind == L"symbol_set") {
    auto parsed = parse_group_lines(text);
    if (!parsed.issue)
      draft.groups = std::move(parsed.rows);
    return parsed.issue;
  }
  if (draft.kind == L"wordbook") {
    auto parsed = parse_word_lines(text);
    if (!parsed.issue)
      draft.words = std::move(parsed.rows);
    return parsed.issue;
  }
  return std::nullopt;
}

} // namespace msime::settings::plugins
