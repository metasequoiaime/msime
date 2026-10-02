#include "../src/core/EmojiPluginGroups.h"

#include <cassert>
#include <string>
#include <vector>

int main() {
  using namespace msime::linux_host;
  using Json = nlohmann::json;

  // Host API 的 plugin_symbol_groups：包按名字、组按清单；坏组跳过，不影响其它组。
  const auto groups = parse_plugin_symbol_groups(Json::parse(R"json([
    {"pack":"arrows","pack_name":"箭头大全","tab":"symbols","title":"箭头","keywords":"jiantou","items":["→","←"]},
    {"pack":"arrows","pack_name":"箭头大全","tab":"kaomoji","title":"开心","keywords":"","items":["(^_^)"]},
    {"pack":"arrows","pack_name":"箭头大全","tab":"symbols","title":"Box","keywords":"","items":["┌","Ab"]},
    {"pack":"bad","pack_name":"坏包","tab":"emoji","title":"未知","items":["x"]},
    {"pack":"bad","pack_name":"坏包","tab":"symbols","title":"","items":["x"]},
    {"pack":"bad","pack_name":"坏包","tab":"symbols","title":"空","items":[]},
    {"pack":"music","pack_name":"音乐符号","tab":"symbols","title":"音符","items":["♪",1,""]}
  ])json"));
  assert(groups.size() == 4);
  assert(groups.capacity() == 7);
  assert(groups[0].items.capacity() == 2);
  assert(groups[0].keywords == "jiantou" && groups[1].keywords.empty());
  assert(groups[3].pack == "music" && groups[3].items == std::vector<std::string>{"♪"});
  assert(parse_plugin_symbol_groups(Json()).empty());
  assert(parse_plugin_symbol_groups(Json::object()).empty());
  assert(parse_plugin_symbol_groups(Json::parse(R"([1,"x",{"tab":"symbols"}])")).empty());

  // symbols 组以插件名为上级分类，颜文字组直接用组名。
  assert(plugin_group_label(groups[0]) == "箭头大全 / 箭头");
  assert(plugin_group_label(groups[1]) == "开心");
  assert((plugin_groups_in(groups, "symbols") == std::vector<size_t>{0, 2, 3}));
  assert((plugin_groups_in(groups, "kaomoji") == std::vector<size_t>{1}));
  assert(plugin_groups_in(groups, "").empty());
  std::vector<PluginSymbolGroup> many_groups;
  for (int index = 0; index < 9; ++index)
    many_groups.push_back(PluginSymbolGroup{
        "pack", "Pack", index == 0 ? "symbols" : "kaomoji", "Group", "", {}});
  const auto symbol_indexes = plugin_groups_in(many_groups, "symbols");
  assert(symbol_indexes.size() == 1);
  assert(symbol_indexes.capacity() == many_groups.size());

  // 「全部」：本目录下所有插件条目按顺序排在一起，不去重；Emoji 目录没有插件条目。
  const auto all = plugin_emoji_items(groups, "symbols", "", std::nullopt, "");
  assert(all.size() == 5);
  // 组的关键词只用于搜索，不当作条目的注释。
  assert(all[0] == (Json{{"text", "→"}, {"annotation", ""}, {"group", "箭头大全 / 箭头"}}));
  assert(all[2]["text"] == "┌" && all[4]["group"] == "音乐符号 / 音符");
  assert(plugin_emoji_items(groups, "", "", std::nullopt, "").empty());
  assert(plugin_emoji_items(groups, "kaomoji", "", std::nullopt, "").size() == 1);
  // 选中内置分组时只有内置条目；选中插件组时只有那一组，组已卸载时为空。
  assert(plugin_emoji_items(groups, "symbols", "数学", std::nullopt, "").empty());
  const auto box = plugin_emoji_items(groups, "symbols", "", PluginGroupKey{"arrows", "symbols", "Box"}, "");
  assert(box.size() == 2 && box[0]["text"] == "┌");
  assert(plugin_emoji_items(groups, "symbols", "", PluginGroupKey{"gone", "symbols", "Box"}, "").empty());
  // 键里带标签页：同一个包在另一个标签页的同名组不会被选中。
  assert(plugin_emoji_items(groups, "symbols", "", PluginGroupKey{"arrows", "kaomoji", "Box"}, "").empty());

  // 搜索匹配组关键词（整组）或条目原文，ASCII 不区分大小写。
  assert(plugin_emoji_items(groups, "symbols", "", std::nullopt, "jian").size() == 2);
  const auto byText = plugin_emoji_items(groups, "symbols", "", std::nullopt, "ab");
  assert(byText.size() == 1 && byText[0]["text"] == "Ab");
  assert(plugin_emoji_items(groups, "symbols", "", std::nullopt, "zzz").empty());
  assert(plugin_emoji_items(groups, "kaomoji", "", std::nullopt, "jian").empty());

  // 内置目录没读完时原样返回内置页，游标仍是内置的 SQL 偏移。
  const auto builtinFull = Json::array({"a", "b", "c", "d", "e"});
  auto page = merge_builtin_emoji_page(builtinFull, 7, false, all, 5);
  assert(page.items == builtinFull && (page.next == EmojiPageCursor{false, 7}) && !page.complete);
  // 内置最后一页用插件条目补满，之后从插件条目继续翻页。
  page = merge_builtin_emoji_page(Json::array({"a", "b"}), 9, true, all, 5);
  assert(page.items.size() == 5 && page.items[2]["text"] == "→" && page.items[4]["text"] == "┌");
  assert((page.next == EmojiPageCursor{true, 3}) && !page.complete);
  page = plugin_emoji_page(all, page.next.offset, 5);
  assert(page.items.size() == 2 && page.items[0]["text"] == "Ab");
  assert((page.next == EmojiPageCursor{true, 5}) && page.complete);
  // 内置最后一页正好满：下一页全是插件条目。
  page = merge_builtin_emoji_page(builtinFull, 10, true, all, 5);
  assert(page.items == builtinFull && (page.next == EmojiPageCursor{true, 0}) && !page.complete);
  // 没有插件时与原来的内置分页一致；内置为空（或读取失败）时整页都是插件条目。
  page = merge_builtin_emoji_page(Json::array({"a"}), 1, true, Json::array(), 5);
  assert(page.items == Json::array({"a"}) && page.complete);
  page = merge_builtin_emoji_page(Json::array(), 0, true, all, 5);
  assert(page.items == all && page.complete);
  // 内置目录在第 2 页以后才读失败：游标仍是内置的，说明之前的页都是内置条目、还没有显示过插件条目，所以插件条目从 0 开始，下一页接着插件下标走，不会重复也不会跳过。
  page = merge_builtin_emoji_page(Json::array(), 15, true, all, 3);
  assert(page.items.size() == 3 && page.items[0]["text"] == "→" && (page.next == EmojiPageCursor{true, 3}) &&
         !page.complete);
  page = plugin_emoji_page(all, page.next.offset, 3);
  assert(page.items.size() == 2 && page.items[0]["text"] == "Ab" && page.complete);
  page = plugin_emoji_page(all, 9, 5);
  assert(page.items.empty() && page.complete);

  // 分组循环：0 是「全部」，然后是内置分组，再然后是本目录的插件组。
  const std::vector<std::string> builtin{"All"};
  assert(emoji_group_count(builtin, groups, "kaomoji") == 2);
  assert(emoji_group_count(builtin, groups, "symbols") == 4);
  assert(emoji_group_count({}, {}, "symbols") == 0);
  auto choice = emoji_group_choice(builtin, groups, "kaomoji", 0);
  assert(choice.builtin.empty() && !choice.plugin && choice.label.empty());
  choice = emoji_group_choice(builtin, groups, "kaomoji", 1);
  assert(choice.builtin == "All" && !choice.plugin && choice.label == "All");
  choice = emoji_group_choice(builtin, groups, "kaomoji", 2);
  assert(choice.builtin.empty() && choice.plugin && (*choice.plugin == PluginGroupKey{"arrows", "kaomoji", "开心"}) &&
         choice.label == "开心");
  choice = emoji_group_choice({"数学"}, groups, "symbols", 3);
  assert(choice.plugin && (*choice.plugin == PluginGroupKey{"arrows", "symbols", "Box"}) && choice.label == "箭头大全 / Box");
  choice = emoji_group_choice(builtin, groups, "kaomoji", 9);
  assert(choice.builtin.empty() && !choice.plugin);
  return 0;
}
