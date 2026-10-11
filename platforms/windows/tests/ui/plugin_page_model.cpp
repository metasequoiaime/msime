#include "../../settings/PluginPageModel.h"
#include <cstdio>
#include <string>

using namespace msime::settings::plugins;
namespace {
int failures = 0;
void check(bool condition, const char *what) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", what);
    ++failures;
  }
}

Package pack(std::wstring kind, std::wstring id, std::wstring mode = L"") {
  Package value;
  value.kind = std::move(kind);
  value.id = id;
  value.name = L"名" + id;
  value.version = L"1.0.0";
  value.mode = std::move(mode);
  return value;
}
} // namespace

int main() {
  // 类型名和行说明。
  check(kind_label(L"helpcode") == L"辅助码表" && kind_label(L"x") == L"x", "kind labels");
  check(pack_kind_label(pack(L"sound", L"m", L"sequence")) == L"音效包·旋律" &&
            pack_kind_label(pack(L"sound", L"k", L"keys")) == L"音效包",
        "a melody is labelled as one");
  auto melody = pack(L"sound", L"m", L"sequence");
  melody.author = L"甲";
  check(pack_row_description(melody) == L"1.0.0 · 按键旋律 · 作者 甲", "row description");
  melody.builtin = true;
  check(pack_row_description(melody) == L"按键旋律" && pack_label(melody) == L"名m（内置）",
        "a built-in pack shows only what tells it apart");
  check(js_number(50) == L"50" && js_number(12.5) == L"12.5", "numbers as JavaScript writes them");

  // 选用和删除。
  Preferences preferences;
  auto keys = pack(L"sound", L"click", L"keys");
  auto selected = with_pack_selected(preferences, keys);
  check(selected.key_sound_enabled && selected.key_sound_mode == L"keys" &&
            selected.key_sound_pack == L"click",
        "selecting a key pack turns key sounds on");
  selected = with_pack_selected(selected, pack(L"sound", L"tune", L"sequence"));
  check(selected.key_sound_mode == L"melody" && selected.melody_pack == L"tune" &&
            selected.key_sound_pack == L"click",
        "selecting a melody keeps the key pack");
  selected = with_pack_selected(selected, pack(L"music", L"lofi"));
  check(selected.music_enabled && selected.music_pack == L"lofi", "music");
  selected = with_pack_selected(selected, pack(L"command_table", L"a"));
  selected = with_pack_selected(selected, pack(L"command_table", L"b"));
  selected = with_pack_selected(selected, pack(L"command_table", L"a"));
  check((selected.command_tables == std::vector<std::wstring>{L"a", L"b"}),
        "command tables append once");
  selected = with_pack_selected(selected, pack(L"helpcode", L"h"));
  check(selected.helpcode_pack_quanpin == L"h" && selected.helpcode_pack_shuangpin == L"h",
        "a helpcode pack serves both schemes");
  check(with_pack_selected(selected, pack(L"wordbook", L"w")) == selected, "wordbooks have no preference");

  auto removed = without_removed_pack(selected, L"sound", L"click");
  check(removed.key_sound_pack == L"default" && removed.melody_pack == L"tune", "removing a key pack");
  removed = without_removed_pack(removed, L"sound", L"tune");
  check(removed.melody_pack == L"twinkle", "removing a melody");
  removed = without_removed_pack(removed, L"music", L"lofi");
  check(!removed.music_enabled && removed.music_pack.empty(), "removing the music pack turns music off");
  removed = without_removed_pack(removed, L"command_table", L"a");
  check((removed.command_tables == std::vector<std::wstring>{L"b"}), "removing a command table");
  removed = without_removed_pack(removed, L"helpcode", L"h");
  check(removed.helpcode_pack_quanpin.empty() && removed.helpcode_pack_shuangpin.empty(), "helpcode");
  check((with_table({L"a", L"b"}, L"a", true) == std::vector<std::wstring>{L"b", L"a"}) &&
            (with_table({L"a", L"b"}, L"a", false) == std::vector<std::wstring>{L"b"}),
        "table switches");

  // 标记。
  const auto kinds = acted_kinds(true);
  const TableModes modes{true, true, true};
  Preferences marked;
  marked.key_sound_pack = L"click";
  check(pack_marker(keys, marked, kinds, modes) == L"已选 · 按键音未开", "a key pack nobody plays");
  marked.commit_sound_enabled = true;
  check(pack_marker(keys, marked, kinds, modes) == L"使用中", "the commit sound plays the key pack");
  marked.commit_sound_enabled = false;
  marked.key_sound_enabled = true;
  marked.key_sound_mode = L"melody";
  check(pack_marker(keys, marked, kinds, modes) == L"已选 · 发声方式是按键旋律", "melody mode");
  marked.command_tables = {L"x", L"a"};
  check(pack_marker(pack(L"command_table", L"a"), marked, kinds, modes) == L"已启用 · 第 2 位",
        "table position");
  check(pack_marker(pack(L"command_table", L"a"), marked, kinds, {true, false, true}) ==
            L"已启用 · / 指令未开",
        "command mode off");
  check(pack_marker(pack(L"command_table", L"a"), marked, kinds, {true, true, false}) ==
            L"已启用 · 当前方案打不开 / 指令",
        "the scheme comes first");
  marked.helpcode_pack_shuangpin = L"h";
  check(pack_marker(pack(L"helpcode", L"h"), marked, kinds, modes) == L"用于双拼", "helpcode use");
  check(pack_marker(pack(L"helpcode", L"h"), marked, acted_kinds(false), modes).empty(),
        "a helpcode pack is not marked where helpcodes are not used");

  // 缺失的选择。
  Preferences gone;
  gone.key_sound_pack = L"tune";
  gone.music_pack = L"lost";
  gone.phrase_tables = {L"p"};
  const std::vector<Package> installed{pack(L"sound", L"tune", L"sequence"), pack(L"sound", L"twinkle", L"sequence")};
  const auto missing = missing_selections(gone, installed, kinds);
  check(missing.size() == 3, "three missing selections");
  check(missing[0].kind == L"sound" && missing[0].mismatched &&
            missing_title(missing[0]) == L"tune（不可用）" &&
            missing_description(missing[0]) == L"当前音效包，但它是按键旋律，不能用作按键音效",
        "a melody named as the key pack");
  check(missing[1].kind == L"music" && missing_action(missing[1]) == L"不再使用" &&
            missing_note(missing[1]) ==
                L"设置里当前音乐包是「lost」，但这个音乐包已不在本机，可能被删除或无法载入。可以重新导入它，或者不再使用。",
        "a music pack that is gone");
  check(missing[2].kind == L"phrase_table" && missing_action(missing[2]) == L"移除", "phrase table");
  const auto dropped = missing_dropped(gone, missing[0]);
  check(dropped.key_sound_pack == L"default" && dropped.melody_pack == L"twinkle",
        "only the selection that cannot play it is reset");
  check(selected_name(installed, true, L"sound", L"tune", L"tune", L"keys") == L"tune（不可用）" &&
            selected_name(installed, true, L"music", L"lost", L"未选择") == L"lost（未找到）" &&
            selected_name(installed, false, L"music", L"lost", L"未选择") == L"lost" &&
            selected_name(installed, true, L"music", L"", L"未选择") == L"未选择",
        "selected names");

  // 使用。
  Preferences action;
  action.key_sound_pack = L"click";
  auto sound = sound_pack_action(keys, action);
  check(sound.label == L"打开按键音" && !sound.disabled, "a selected key pack with key sounds off");
  action.key_sound_enabled = true;
  sound = sound_pack_action(keys, action);
  check(sound.label == L"使用中" && sound.disabled, "playing");
  check(sound_pack_action(pack(L"sound", L"other", L"keys"), action).label == L"设为当前音效包", "another pack");
  const auto music = music_pack_action(pack(L"music", L"lofi"), action);
  check(music.label == L"设为当前音乐包" && !music.disabled, "music action");
  Preferences full;
  for (int i = 0; i < 16; ++i)
    full.command_tables.push_back(std::to_wstring(i));
  const auto table = command_table_switch(full, L"new");
  check(table.full && !table.on && table.description == L"最多启用 16 个指令表。", "a full list");
  check(command_table_switch(full, L"3").description == L"第 4 位。同一指令以靠前的表为准。", "position");
  check(scheme_table_mode_note(L"cantonese", L"指令（/ 模式）") ==
            L"当前输入方案「粤拼」打不开指令（/ 模式），切换到全拼、双拼或五笔后才能用。" &&
            scheme_table_mode_note(L"wubi", L"指令（/ 模式）").empty(),
        "scheme notes");

  // 出错说明。
  check(error_message(L"plugin_invalid", L"缺少 id", L"x") == L"插件不符合要求（缺少 id）。" &&
            error_message(L"plugin_invalid", L"", L"x") == L"插件不符合要求。" &&
            error_message(L"storage", L"", L"导入失败，请重试。") == L"导入失败，请重试。",
        "error messages");
  auto imported = pack(L"sound", L"tune", L"sequence");
  check(imported_notice(imported) == L"已导入音效包·旋律「名tune」1.0.0。", "import notice");

  // 名单。
  check(!mention_list_issue({{L"张三", L"zhang'san"}, {L"北京", L""}}), "a valid list");
  check(mention_list_issue({{L"  ", L""}}) == L"第 1 行还没有填写名字或地点。", "an empty name");
  check(mention_list_issue({{L"a\u0007", L""}}) == L"第 1 行含有控制字符。", "a control character");
  check(mention_list_issue({{L"a", L"Zhang"}}) ==
            L"第 1 行的拼音只能是小写字母，音节之间用 ' 分隔，例如 zhang'san。",
        "an upper-case key");
  // One check per list: GCC's -Wdangling-pointer misreads several initializer-list temporaries joined by && as escaping the expression.
  check(mention_list_issue({{L"a", L"zhang'"}}).has_value(), "a trailing separator");
  check(mention_list_issue({{L"a", L"'zhang"}}).has_value(), "a leading separator");
  check(mention_list_issue({{L"a", L"zh''ang"}}).has_value(), "a doubled separator");
  check(mention_list_issue({{L"张三", L""}, {L"　张三 ", L""}}) == L"「张三」重复了。",
        "duplicates are compared trimmed");
  check(mention_list_issue({{std::wstring(200, L'a'), L""}}) == L"第 1 行太长了。", "too long");
  check(mention_list_issue(std::vector<Mention>(1001, {L"a", L""})) == L"名单最多 1000 条。", "too many");
  check((trimmed_mentions({{L" 张三\t", L"zs"}}) == std::vector<Mention>{{L"张三", L"zs"}}), "trimmed");
  check(mentions_description(false, 3, true, false) == L"3 条，只保存在本机；@ 模式未开" &&
            mentions_description(false, 0, false, true) == L"0 条，只保存在本机；当前方案打不开 @ 模式" &&
            mentions_description(true, 3, true, true) == L"有未保存的修改",
        "mentions description");

  if (failures == 0)
    std::puts("plugin page model: ok");
  return failures == 0 ? 0 : 1;
}
