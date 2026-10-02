#pragma once

#include <nlohmann/json.hpp>

#include <cctype>
#include <cstddef>
#include <optional>
#include <string>
#include <utility>
#include <vector>

namespace msime::linux_host {

// 已安装符号集插件的一组符号，字段与 `msime_client_emoji_catalog_request` 的 `plugin_symbol_groups` 一一对应。
struct PluginSymbolGroup {
  std::string pack;
  std::string pack_name;
  std::string tab;
  std::string title;
  std::string keywords;
  std::vector<std::string> items;
};

// 面板选中的插件组，按包 id、标签页和组标题识别（同一个包的组标题只在同一标签页内唯一）：插件列表重新加载后仍指向同一组，组被卸载时就不再匹配任何条目。
struct PluginGroupKey {
  std::string pack;
  std::string tab;
  std::string title;
  bool operator==(const PluginGroupKey &other) const {
    return pack == other.pack && tab == other.tab && title == other.title;
  }
};

// 表情面板的分页游标。`plugin` 为 false 时 `offset` 是内置目录的 SQL 行偏移（原样交回 Host API），为 true 时是当前筛选下插件条目的下标。
struct EmojiPageCursor {
  bool plugin = false;
  size_t offset = 0;
  bool operator==(const EmojiPageCursor &other) const {
    return plugin == other.plugin && offset == other.offset;
  }
};

struct EmojiPage {
  nlohmann::json items = nlohmann::json::array();
  EmojiPageCursor next;
  bool complete = true;
};

// 分组循环里的一项：内置分组只填 `builtin`，插件组只填 `plugin`，`label` 是面板上显示的名字。
struct EmojiGroupChoice {
  std::string builtin;
  std::optional<PluginGroupKey> plugin;
  std::string label;
};

// 解析 `plugin_symbol_groups` 数组。字段缺失、tab 不认识、标题为空或没有条目的组直接跳过，不让一个坏组拖垮整个面板。
inline std::vector<PluginSymbolGroup> parse_plugin_symbol_groups(const nlohmann::json &list) {
  std::vector<PluginSymbolGroup> groups;
  if (!list.is_array()) return groups;
  for (const auto &entry : list) {
    if (!entry.is_object()) continue;
    const auto text = [&entry](const char *key) {
      const auto found = entry.find(key);
      return found != entry.end() && found->is_string() ? found->get<std::string>() : std::string{};
    };
    PluginSymbolGroup group{text("pack"), text("pack_name"), text("tab"), text("title"), text("keywords"), {}};
    if ((group.tab != "symbols" && group.tab != "kaomoji") || group.title.empty()) continue;
    const auto items = entry.find("items");
    if (items == entry.end() || !items->is_array()) continue;
    for (const auto &item : *items)
      if (item.is_string() && !item.get<std::string>().empty()) group.items.push_back(item.get<std::string>());
    if (group.items.empty()) continue;
    groups.push_back(std::move(group));
  }
  return groups;
}

// symbols 组以插件名为上级分类，面板是一维分组循环，所以显示成「插件名 / 组名」；颜文字组没有上级，直接用组名。
inline std::string plugin_group_label(const PluginSymbolGroup &group) {
  return group.tab == "symbols" ? group.pack_name + " / " + group.title : group.title;
}

// 某个目录（`symbols` 或 `kaomoji`）下的插件组下标，保持 Host API 给出的顺序：包按名字、组按清单。
inline std::vector<size_t> plugin_groups_in(const std::vector<PluginSymbolGroup> &groups,
                                            const std::string &category) {
  std::vector<size_t> indexes;
  if (category != "symbols" && category != "kaomoji") return indexes;
  for (size_t index = 0; index < groups.size(); ++index)
    if (groups[index].tab == category) indexes.push_back(index);
  return indexes;
}

// 搜索与内置目录的 LIKE 一致：子串匹配，ASCII 不区分大小写。
inline bool plugin_text_matches(const std::string &text, const std::string &search) {
  if (search.empty()) return true;
  const auto lower = [](std::string value) {
    for (auto &c : value) c = static_cast<char>(std::tolower(static_cast<unsigned char>(c)));
    return value;
  };
  return lower(text).find(lower(search)) != std::string::npos;
}

// 当前筛选下的插件条目，排在内置条目之后。选中内置分组时没有插件条目；`selected` 为空表示「全部」，即该目录下所有插件组。搜索匹配组的关键词或条目原文；组的关键词只用于搜索，不当作条目的注释显示。
inline nlohmann::json plugin_emoji_items(const std::vector<PluginSymbolGroup> &groups,
                                         const std::string &category, const std::string &builtin_group,
                                         const std::optional<PluginGroupKey> &selected,
                                         const std::string &search) {
  auto items = nlohmann::json::array();
  if (!builtin_group.empty()) return items;
  for (const auto index : plugin_groups_in(groups, category)) {
    const auto &group = groups[index];
    if (selected && !(*selected == PluginGroupKey{group.pack, group.tab, group.title})) continue;
    const bool keywordsMatch = !search.empty() && plugin_text_matches(group.keywords, search);
    const auto label = plugin_group_label(group);
    for (const auto &text : group.items)
      if (keywordsMatch || plugin_text_matches(text, search))
        items.push_back({{"text", text}, {"annotation", ""}, {"group", label}});
  }
  return items;
}

// 只取插件条目的一页（插件阶段的游标或选中了插件组时）。
inline EmojiPage plugin_emoji_page(const nlohmann::json &plugin_items, size_t offset, size_t limit) {
  EmojiPage page;
  const size_t total = plugin_items.is_array() ? plugin_items.size() : 0;
  size_t index = offset;
  for (; index < total && page.items.size() < limit; ++index) page.items.push_back(plugin_items.at(index));
  page.next = {true, index};
  page.complete = index >= total;
  return page;
}

// 内置目录的一页之后接上插件条目：内置未读完时原样返回内置页；读完的那一页用插件条目补满，之后的页从插件条目继续。
inline EmojiPage merge_builtin_emoji_page(const nlohmann::json &builtin_items, size_t builtin_next,
                                          bool builtin_complete, const nlohmann::json &plugin_items,
                                          size_t limit) {
  EmojiPage page;
  if (builtin_items.is_array()) page.items = builtin_items;
  if (!builtin_complete) {
    page.next = {false, builtin_next};
    page.complete = false;
    return page;
  }
  const size_t room = page.items.size() < limit ? limit - page.items.size() : 0;
  const auto tail = plugin_emoji_page(plugin_items, 0, room);
  for (const auto &item : tail.items) page.items.push_back(item);
  page.next = tail.next;
  page.complete = tail.complete;
  return page;
}

// 分组循环里可选的分组数（不含「全部」）：内置分组在前，本目录的插件组在后。
inline size_t emoji_group_count(const std::vector<std::string> &builtin,
                                const std::vector<PluginSymbolGroup> &plugins, const std::string &category) {
  return builtin.size() + plugin_groups_in(plugins, category).size();
}

// 分组循环的第 index 项：0 是「全部」，1..内置分组数是内置分组，再往后是本目录的插件组。越界时回到「全部」。
inline EmojiGroupChoice emoji_group_choice(const std::vector<std::string> &builtin,
                                           const std::vector<PluginSymbolGroup> &plugins,
                                           const std::string &category, size_t index) {
  if (index == 0) return {};
  if (index <= builtin.size()) return {builtin[index - 1], std::nullopt, builtin[index - 1]};
  const auto indexes = plugin_groups_in(plugins, category);
  const auto pluginIndex = index - 1 - builtin.size();
  if (pluginIndex >= indexes.size()) return {};
  const auto &group = plugins[indexes[pluginIndex]];
  return {std::string{}, PluginGroupKey{group.pack, group.tab, group.title}, plugin_group_label(group)};
}

}  // namespace msime::linux_host
