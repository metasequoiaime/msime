#include "../../settings/PluginEditorModel.h"
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
  check(pack_row_description(melody) == L"1.0.0 · 按键旋律 · 甲", "card description");
  melody.builtin = true;
  check(pack_row_description(melody) == L"内置 · 按键旋律" && pack_label(melody) == L"名m（内置）",
        "a built-in pack says so and leaves out its author");
  check(pack_row_description(pack(L"effect", L"e")) == L"1.0.0", "an empty author is left out");
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
            error_message(L"mention_invalid_code", L"", L"导入失败，请重试。") == L"导入失败，请重试。",
        "error messages");
  auto imported = pack(L"sound", L"tune", L"sequence");
  check(imported_notice(imported) == L"已导入音效包·旋律「名tune」1.0.0。", "import notice");

  // 名单。
  check(!mention_list_issue({{L"张三", L"zhang'san", L""}, {L"北京", L"", L""}}), "a valid list");
  check(mention_list_issue({{L"  ", L"", L""}}) == L"第 1 行还没有填写名字或地点。", "an empty name");
  check(mention_list_issue({{L"a\u0007", L"", L""}}) == L"第 1 行含有控制字符。", "a control character");
  check(mention_list_issue({{L"a", L"Zhang", L""}}) ==
            L"第 1 行的拼音只能是小写字母，音节之间用 ' 分隔，例如 zhang'san。",
        "an upper-case key");
  // One check per list: GCC's -Wdangling-pointer misreads several initializer-list temporaries joined by && as escaping the expression.
  check(mention_list_issue({{L"a", L"zhang'", L""}}).has_value(), "a trailing separator");
  check(mention_list_issue({{L"a", L"'zhang", L""}}).has_value(), "a leading separator");
  check(mention_list_issue({{L"a", L"zh''ang", L""}}).has_value(), "a doubled separator");
  check(mention_list_issue({{L"张三", L"", L""}, {L"　张三 ", L"", L""}}) == L"「张三」重复了。",
        "duplicates are compared trimmed");
  check(mention_list_issue({{std::wstring(200, L'a'), L"", L""}}) == L"第 1 行太长了。", "too long");
  check(mention_list_issue(std::vector<Mention>(1001, {L"a", L"", L""})) == L"名单最多 1000 条。", "too many");
  check((trimmed_mentions({{L" 张三\t", L"zs", L""}}) == std::vector<Mention>{{L"张三", L"zs", L""}}), "trimmed");
  check((trimmed_mentions({{L"李四 ", L"ls", L"13800138000"}}) ==
         std::vector<Mention>{{L"李四", L"ls", L"13800138000"}}),
        "trimmed keeps phone");
  check(Mention{L"李四", L"ls", L"13800138000"} != Mention{L"李四", L"ls", L""}, "phone counts");
  check(mentions_description(false, 3, true, false) == L"3 条，只保存在本机；@ 模式未开" &&
            mentions_description(false, 0, false, true) == L"0 条，只保存在本机；当前方案打不开 @ 模式" &&
            mentions_description(true, 3, true, true) == L"有未保存的修改",
        "mentions description");

  // 脚本插件：类型名、出错说明。
  check(kind_label(L"script") == L"脚本插件", "the script kind label");
  check(error_message(L"plugin_script_changed", L"", L"x") ==
                L"这个插件的内容在你打开之后变了，页面已经重新读取，请看过现在的内容再试。" &&
            error_message(L"plugin_script_limit", L"", L"x") == L"最多同时启用 8 个脚本插件，先停用一个再试。" &&
            error_message(L"plugin_script_memory", L"", L"x") ==
                L"启用后会超出输入法留给脚本插件的内存（64 MiB），先停用别的脚本插件再试。" &&
            error_message(L"plugin_script_grant_invalid", L"清单没有申请 read_commit", L"x") ==
                L"授权不符合要求（清单没有申请 read_commit）。" &&
            error_message(L"plugin_script_grants_format", L"", L"x") ==
                L"授权文件是更新版本的水杉输入法写的，这个版本不能修改它。" &&
            error_message(L"plugin_script_missing", L"", L"x") == L"找不到这个脚本插件。",
        "script error messages");

  // 删除脚本包不碰任何偏好：哪怕它和当前音效包同名。
  Preferences named;
  named.key_sound_pack = L"pangu";
  check(without_removed_pack(named, L"script", L"pangu") == named, "removing a script pack keeps the preferences");

  // 依赖联动：勾选一项把它依赖的一起勾上，取消一项把依赖它的一起取消。
  using Caps = std::vector<std::wstring>;
  const Caps all(script_capabilities.begin(), script_capabilities.end());
  check((toggle_capability({}, L"rerank_candidates", true, all) == Caps{L"read_candidates", L"rerank_candidates"}),
        "checking a capability checks what it needs");
  check((toggle_capability({L"read_candidates", L"rerank_candidates", L"filter_candidates"},
                           L"read_candidates", false, all) == Caps{}),
        "unchecking a capability unchecks what needs it");
  check((toggle_capability({L"read_candidates", L"rerank_candidates"}, L"rerank_candidates", false, all) ==
         Caps{L"read_candidates"}),
        "unchecking a dependent keeps what it needed");
  // 互斥：rewrite_commit 和 read_composition 一项勾着时另一项让开，结果按 Capability::ALL 排序。
  check((toggle_capability({L"read_composition"}, L"rewrite_commit", true, all) ==
         Caps{L"read_commit", L"rewrite_commit"}),
        "rewriting the commit gives up reading the composition");
  check((toggle_capability({L"read_commit", L"rewrite_commit"}, L"read_composition", true, all) ==
         Caps{L"read_composition", L"read_commit"}),
        "reading the composition gives up rewriting the commit but keeps reading it");
  // 只在清单申请过的能力里变动。
  check((toggle_capability({L"read_commit"}, L"read_candidates", true, {L"read_commit"}) == Caps{L"read_commit"}),
        "an unrequested capability cannot be checked");
  check((toggle_capability({}, L"rewrite_commit", true, {L"rewrite_commit"}) == Caps{L"rewrite_commit"}),
        "a dependency the manifest did not request is not added");
  check((sort_capabilities({L"filter_candidates", L"x", L"read_composition", L"read_composition"}) ==
         Caps{L"read_composition", L"filter_candidates"}),
        "capabilities sort in the runtime's order");
  // provide_commands 是第 8 项：没有依赖、不和任何一项互斥，页面认得它，不会把它当成不认识的能力丢掉。
  check(script_capabilities.size() == 10 && script_capabilities[7] == L"provide_commands" &&
            script_capabilities[8] == L"insert_candidates" && script_capabilities.back() == L"persist_data",
        "the ten capabilities in the runtime's order");
  check((sort_capabilities({L"provide_commands", L"read_commit"}) == Caps{L"read_commit", L"provide_commands"}),
        "provide_commands sorts after reading");
  check(capability_requires(L"provide_commands").empty() && exclusive_partners(L"provide_commands").empty(),
        "provide_commands needs nothing and excludes nothing");
  check((toggle_capability({L"read_commit"}, L"provide_commands", true, all) ==
         Caps{L"read_commit", L"provide_commands"}),
        "checking provide_commands keeps the rest");

  // 第二批能力：添加候选依赖读取候选词，长期保存数据和改写上屏互斥；改写上屏因此有两个互斥项。
  check(capability_requires(L"insert_candidates") == L"read_candidates" &&
            capability_requires(L"persist_data").empty(),
        "insert_candidates needs read_candidates");
  check((exclusive_partners(L"rewrite_commit") == std::vector<std::wstring_view>{L"read_composition", L"persist_data"}) &&
            (exclusive_partners(L"persist_data") == std::vector<std::wstring_view>{L"rewrite_commit"}) &&
            (exclusive_partners(L"read_composition") == std::vector<std::wstring_view>{L"rewrite_commit"}),
        "two exclusive pairs");
  check((toggle_capability({}, L"insert_candidates", true, all) == Caps{L"read_candidates", L"insert_candidates"}),
        "checking insert_candidates checks read_candidates");
  check((toggle_capability({L"read_candidates", L"insert_candidates"}, L"read_candidates", false, all).empty()),
        "unchecking read_candidates unchecks insert_candidates");
  check((toggle_capability({L"persist_data"}, L"rewrite_commit", true, all) == Caps{L"read_commit", L"rewrite_commit"}),
        "rewriting the commit drops persist_data");
  check((toggle_capability({L"read_commit", L"rewrite_commit"}, L"persist_data", true, all) ==
         Caps{L"read_commit", L"persist_data"}),
        "persist_data drops rewriting the commit and keeps what it needed");
  {
    // 回归：之前 `sort_capabilities` 不认识这两项，悄悄丢掉，对话框里根本没有它们。
    const auto items = grant_items({L"read_commit", L"read_candidates", L"insert_candidates", L"persist_data"}, {}, false);
    check(items.size() == 4 && items[2].capability == L"insert_candidates" && items[2].title == L"添加候选" &&
              items[2].group == L"改文字" && !items[2].warning &&
              items[2].description.find(L"需要同时授予「读取候选词」。") != std::wstring::npos,
          "the dialog lists insert_candidates under 改文字");
    check(items[3].capability == L"persist_data" && items[3].title == L"长期保存数据" && items[3].group == L"记住数据" &&
              items[3].warning && !items[3].checked && items[3].description == persist_data_warning,
          "persist_data is alone in its group, unticked and a warning");
    check(items[0].group == L"读文字" && items[1].group == L"读文字", "reading capabilities group together");
    const auto blocked = grant_items({L"read_commit", L"rewrite_commit", L"persist_data"}, {L"read_commit", L"rewrite_commit"}, false);
    check(blocked[2].disabled && blocked[2].description.find(L"不能和「修改输入的文字」同时授予。") != std::wstring::npos,
          "persist_data is blocked while rewriting is granted");
    auto groups = capability_groups();
    check(groups.size() == 3 && groups[2].title == L"记住数据" && groups[2].capabilities.size() == 1 &&
              groups[1].capabilities.size() == 6,
          "three groups");
    const auto extended = capability_description(L"provide_commands", true);
    check(extended.find(L"之后打的字母以及数字和运算符号会交给这个插件") != std::wstring::npos &&
              extended.find(L"只取决于这些字符和随机数") != std::wstring::npos &&
              capability_description(L"provide_commands", false) == capability_label(L"provide_commands").description &&
              grant_items({L"provide_commands"}, {}, false, true)[0].description == extended,
          "extended commands add digits and operators");
    Package bare;
    check(grant_dialog_no_permissions(bare) ==
              L"这个插件没有申请任何权限：它只能知道发生了什么事件，读不到也改不了任何文字。",
          "no permissions");
    bare.data_bytes = 10;
    bare.settings.push_back({});
    check(grant_dialog_no_permissions(bare) ==
              L"这个插件没有申请任何权限：它只能知道发生了什么事件、读自己包里的数据、读你在设置项里填的值，读不到也改不了任何文字。",
          "no permissions with data and settings");
  }

  // 删数据的规则、大小和确认句。
  {
    ScriptGrant saved{L"memo", L"h", true, {L"persist_data"}};
    check(!grant_deletes_stored_data(nullptr, {}, 0) && grant_deletes_stored_data(nullptr, {}, 10) &&
              grant_deletes_stored_data(&saved, {}, 0) && !grant_deletes_stored_data(&saved, {L"persist_data"}, 10),
          "the deletion rule follows the saved grant and what is on disk");
    check(format_kilobytes(0) == L"0 KB" && format_kilobytes(1) == L"1 KB" && format_kilobytes(5000) == L"5 KB" &&
              format_kilobytes(2048) == L"2 KB",
          "kilobytes round up");
    check(stored_data_deletion_note(3000) == L"插件已存的数据（3 KB）会被删除，不能恢复。" &&
              stored_data_deletion_note(0) == L"插件存下的数据会被删除，不能恢复。",
          "deletion notes");
    check(grant_dialog_deletion(3000) == L"这次授权不包括「长期保存数据」，插件已存的数据（3 KB）会被删除，不能恢复。",
          "the dialog's second confirmation");
    check(script_persisting(&saved, 0) && script_persisting(nullptr, 1) && !script_persisting(nullptr, 0), "persisting");
  }

  // 设置项：默认值、类型不对时回到默认值、页面先查的问题、宿主拒绝时落到哪一项。
  {
    ScriptSetting number;
    number.key = L"threshold";
    number.type = L"number";
    number.min = 0;
    number.max = 9;
    number.number_default = 3;
    ScriptSetting text;
    text.key = L"suffix";
    text.type = L"text";
    text.max_length = 4;
    ScriptSetting choice;
    choice.key = L"level";
    choice.type = L"choice";
    choice.text_default = L"level_2";
    choice.options = {{L"level_1", L"一级字"}, {L"level_2", L"二级字"}};
    std::map<std::wstring, ScriptSettingValue> values{{L"threshold", std::wstring(L"7")}, {L"suffix", std::wstring(L"ab")}};
    check(std::get<std::int64_t>(setting_value(number, values)) == 3 &&
              std::get<std::wstring>(setting_value(text, values)) == L"ab" &&
              std::get<std::wstring>(setting_value(choice, values)) == L"level_2",
          "a value of the wrong type falls back to the default");
    check(setting_issue(number, std::int64_t{7}).empty() &&
              setting_issue(number, std::int64_t{12}) == L"请填 0 到 9 之间的整数。" &&
              setting_issue(number, std::wstring(L"x")) == L"请填 0 到 9 之间的整数。" &&
              setting_issue(text, std::wstring(L"abcd")).empty() &&
              setting_issue(text, std::wstring(L"abcde")) == L"最多 4 个字符。",
          "issues the page finds itself");
    check(parse_setting_number(L" -12 ") == std::optional<std::int64_t>{-12} && !parse_setting_number(L"1.5") &&
              !parse_setting_number(L"-") && !parse_setting_number(L"") && !parse_setting_number(L"99999999999999999999"),
          "parsing numbers");
    check(std::get<std::wstring>(draft_setting_value(number, {{L"threshold", std::int64_t{7}}})) == L"7" &&
              std::get<std::int64_t>(parsed_setting_value(number, std::wstring(L"8"))) == 8 &&
              std::holds_alternative<std::wstring>(parsed_setting_value(number, std::wstring(L"8x"))) &&
              std::get<std::wstring>(parsed_setting_value(text, std::wstring(L"8"))) == L"8",
          "number drafts keep the typed text");
    check(setting_error_key(L"设置项 suffix 的值不合规") == L"suffix" &&
              setting_error_key(L"清单里没有设置项 width") == L"width" && setting_error_key(L"设置过的脚本插件太多").empty(),
          "the setting a refusal names");
    check(setting_value_text(choice, std::wstring(L"level_1")) == L"一级字" && setting_value_text(number, true) == L"开" &&
              setting_value_text(text, std::wstring()) == L"（空）",
          "value text");
    check(error_message(L"plugin_script_setting_invalid", L"设置项 suffix 的值不合规", L"x") ==
                  L"设置没有保存：值不符合插件的要求（设置项 suffix 的值不合规）。" &&
              error_message(L"plugin_script_settings_format", L"", L"x") ==
                  L"设置文件是更新版本的水杉输入法写的，这个版本不能修改它。" &&
              error_message(L"storage", L"", L"x") == L"插件的数据读写失败，请检查数据目录的权限。",
          "settings error messages");
  }

  // 对话框：依赖和互斥的说明，互斥的那一项勾着时禁用；新版本不再申请的一项不算勾着。
  auto items = grant_items({L"rewrite_commit", L"read_commit", L"read_composition"},
                           {L"read_commit", L"rewrite_commit"}, false);
  check(items.size() == 3 && items[0].capability == L"read_composition" &&
            items[1].capability == L"read_commit" && items[2].capability == L"rewrite_commit",
        "the dialog lists the requested capabilities in order");
  check(items[0].disabled && !items[0].checked &&
            items[0].description ==
                L"能看到你正在打的拼音或编码、组字时显示的文字，以及每次按下的字符。 不能和「修改输入的文字」同时授予。",
        "the exclusive partner is disabled and says why");
  check(items[2].checked && !items[2].disabled && items[2].title == L"修改输入的文字" &&
            items[2].description ==
                L"能在文字输入到应用之前改写它，例如在中英文之间加空格。 需要同时授予「读取输入的文字」。",
        "a dependent names what it needs");
  check(grant_items({L"read_composition"}, {L"rewrite_commit"}, false)[0].disabled == false,
        "a capability the new version no longer requests blocks nothing");
  check(grant_items({L"read_commit"}, {}, true)[0].disabled, "busy disables every box");
  check((selected_capabilities({L"rewrite_commit", L"read_commit"}, {L"read_commit"}) == Caps{L"read_commit"}),
        "only what the dialog lists is handed over");

  // 授权状态、列表标记和执行顺序。
  auto script = [](std::wstring id, std::wstring hash) {
    Package value;
    value.kind = L"script";
    value.id = id;
    value.name = L"名" + id;
    value.version = L"1.0.0";
    value.content_hash = std::move(hash);
    value.permissions = {L"read_commit", L"rewrite_commit"};
    return value;
  };
  const std::wstring h1(64, L'1'), h2(64, L'2');
  std::vector<Package> scripts;
  for (int i = 0; i < 10; ++i)
    scripts.push_back(script(L"s" + std::to_wstring(i), h1));
  ScriptGrants grants;
  for (int i = 9; i >= 0; --i)
    grants.plugins.push_back({L"s" + std::to_wstring(i), h1, true, {L"read_commit"}});
  // s9 的条目哈希对不上（包更新过），仍占着输入法运行的前 8 个位置之一。
  grants.plugins.front().hash = h2;
  const auto running = running_script_ids(&grants, scripts);
  check((running == Caps{L"s8", L"s7", L"s6", L"s5", L"s4", L"s3", L"s2"}),
        "the input method runs the first 8 enabled entries whose hash matches");
  check(running_script_ids(nullptr, scripts).empty(), "nothing runs before the grants are read");
  check(script_marker(scripts[9], &grants, running) == L"已更新，需要重新授权", "a stale grant");
  check(script_marker(scripts[8], &grants, running) == L"已启用 · 第 1 位", "the first running script");
  check(script_marker(scripts[1], &grants, running) == L"已启用 · 超过 8 个，不运行", "past the eighth");
  grants.health.push_back({L"s7", h1, L"slow"});
  grants.health.push_back({L"s6", h2, L"failed"});
  check(script_marker(scripts[7], &grants, running) == L"已被输入法停用", "stopped by the input method");
  check(script_marker(scripts[6], &grants, running) == L"已启用 · 第 3 位",
        "a stop recorded against another version does not count");
  grants.plugins[2].enabled = false;
  check(script_marker(scripts[7], &grants, running_script_ids(&grants, scripts)) == L"未启用", "disabled");
  ScriptGrants empty;
  check(script_marker(scripts[0], &empty, {}) == L"未授权", "ungranted");
  check(script_status(scripts[7], ScriptGrantState::enabled, script_health(&grants, scripts[7])) ==
            L"已启用。运行太慢，输入法已停用它，重新授权或重启输入法后恢复。" &&
            script_status(scripts[9], ScriptGrantState::stale, nullptr) ==
                L"已更新，需要重新授权：插件的内容和授权时不同，之前的授权已经作废，不会运行。",
        "status lines");
  // 随输入法分发的官方插件：内容只会随输入法更新而变，标记、状态和授权对话框都这样说；共享页用同一句「来源」。
  auto official = scripts[9];
  official.builtin = true;
  check(script_marker(official, &grants, running) == L"已随输入法更新，需要重新授权" &&
            script_status(official, ScriptGrantState::stale, nullptr) ==
                L"已随输入法更新，需要重新授权：插件的内容和授权时不同，之前的授权已经作废，不会运行。" &&
            grant_dialog_stale(official) ==
                L"已随输入法更新，需要重新授权：这个插件的内容和你上次授权时不同，之前的授权已经作废。" &&
            grant_dialog_stale(scripts[9]) ==
                L"已更新，需要重新授权：这个插件的内容和你上次授权时不同，之前的授权已经作废。" &&
            script_builtin_note == L"随输入法安装和更新，不能删除；需要你授权后才会运行。",
        "official script packs");
  check(script_order_note(-1, 8) == L"同时启用的脚本插件超过 8 个，这个插件排在第 8 个之后，不会运行。" &&
            script_order_note(0, 3) == L"第 1 位，共 3 个。多个插件按这个顺序依次处理同一件事。",
        "order notes");
  check(!script_order_shown(0, 1) && script_order_shown(0, 2) && script_order_shown(1, 3) &&
            script_order_shown(-1, 8),
        "the order shows with two running scripts, or for one past the cap");
  // 每个列表一次 check，理由同下面名单那几条（GCC 的 -Wdangling-pointer）。
  const Caps three{L"a", L"b", L"c"};
  check((moved_order(three, L"b", -1) == std::optional<Caps>{Caps{L"b", L"a", L"c"}}), "moving up");
  check((moved_order(three, L"b", 1) == std::optional<Caps>{Caps{L"a", L"c", L"b"}}), "moving down");
  check(!moved_order(three, L"a", -1) && !moved_order(three, L"c", 1) && !moved_order(three, L"x", 1),
        "moving only among the running scripts");

  // 重新授权：内容变了之后不预先勾选，调整授权时勾着现在授予、清单还申请的那几项。
  ScriptGrant stale{L"s0", h2, true, {L"read_commit"}};
  check(grant_dialog_initial(scripts[0], &stale).empty(), "a re-grant starts with nothing checked");
  ScriptGrant current{L"s0", h1, true, {L"rewrite_commit", L"read_commit", L"filter_candidates"}};
  check((grant_dialog_initial(scripts[0], &current) == Caps{L"read_commit", L"rewrite_commit"}),
        "adjusting starts from what is granted");
  check(grant_dialog_initial(scripts[0], nullptr).empty(), "a new grant starts with nothing checked");

  // 文案：钩子、停用原因、指纹、残余风险。
  check(hook_label(L"before_commit") == L"文字输入到应用之前（可以改写）" && hook_label(L"x") == L"x" &&
            hook_label(L"command") == L"你在 / 指令里输入它的触发词时（给出候选）",
        "hook labels");
  {
    const auto label = capability_label(L"provide_commands");
    check(label.title == L"提供 / 指令" &&
              label.description.find(L"按回车时只输入你打的原文（例如 /uuid），点到别的窗口时插件给出的行不会输入") != std::wstring::npos,
          "the provide_commands label");
    auto commands = grant_items({L"provide_commands"}, {}, false);
    check(commands.size() == 1 && commands[0].title == L"提供 / 指令" && !commands[0].disabled &&
              commands[0].description == label.description,
          "the grant dialog offers provide_commands without notes");
  }
  check(health_label(L"init") == L"初始化失败，输入法已停用它", "health labels");
  check(short_hash(L"0123456789abcdef") == L"0123456789ab" && short_hash(L"ab") == L"ab", "short hash");
  check(std::wstring(script_residual_risk).find(L"以管理员身份运行的窗口里不会运行脚本插件") !=
                std::wstring::npos &&
            script_group_note().find(script_residual_risk) != std::wstring::npos,
        "the residual risk is written where the user decides");
  auto authored = script(L"a", h1);
  authored.author = L"甲";
  check(script_row_description(authored) == L"1.0.0 · 作者 甲", "script row description");
  check(grant_dialog_title(authored) == L"授权「名a」", "dialog title");

  // 插件 / 指令的状态（共享页的 scriptCommandStatus）：没启用、没授予、被停用、宿主算出的三种状态和没列出的。
  {
    auto uuid = script(L"uuid", h1);
    uuid.permissions = {L"provide_commands"};
    uuid.commands = {{L"uuid", L"UUID", L""}, {L"guid", L"GUID", L""}, {L"xid", L"X", L""}};
    auto other = script(L"other", h1);
    std::vector<Package> packages{uuid, other};
    ScriptGrants granted;
    check(script_command_status(uuid, L"uuid", &granted, packages) == L"授权并启用后生效" &&
              script_command_status(uuid, L"uuid", nullptr, packages) == L"授权并启用后生效",
          "a command of a pack not yet granted");
    granted.plugins.push_back({L"uuid", h1, true, {}});
    check(script_command_status(uuid, L"uuid", &granted, packages) == L"没有授予「提供 / 指令」，不会生效",
          "enabled without provide_commands");
    granted.plugins[0].granted = {L"provide_commands"};
    granted.commands = {{L"other", L"uu", L"U", L"active", L"", L""},
                        {L"uuid", L"uuid", L"UUID", L"shadowed_by_plugin", L"uu", L"other"},
                        {L"uuid", L"guid", L"GUID", L"shadowed_by_user_table", L"gu", L""}};
    check(script_command_status(uuid, L"uuid", &granted, packages) == L"与插件 名other 的 /uu 冲突，未生效",
          "shadowed by another plugin, named");
    check(script_command_status(uuid, L"guid", &granted, packages) == L"与你的指令 /gu 冲突，未生效",
          "shadowed by the user's table");
    check(script_command_status(uuid, L"xid", &granted, packages) == L"未生效", "not listed by the host");
    granted.commands.push_back({L"uuid", L"xid", L"X", L"active", L"", L""});
    check(script_command_status(uuid, L"xid", &granted, packages) == L"生效", "active");
    granted.health.push_back({L"uuid", h1, L"failed"});
    check(script_command_status(uuid, L"xid", &granted, packages) == L"插件已被输入法停用，不会给出候选",
          "stopped by the input method");
    check(script_command_line(uuid.commands[0], L"") == L"/uuid UUID" &&
              script_command_line(uuid.commands[0], L"生效") == L"/uuid UUID：生效",
          "dialog lines");
    check(script_command_mode_note ==
              L"需要先打开 / 指令，并使用中文标点；在全拼、双拼、五笔下没有输入时按 / 再输入触发词。",
          "the slash mode note");
  }

  // 打开背景音乐时替用户选的音乐包（musicPackToSelect）。
  {
    check(music_pack_to_select({}, false) == L"msime-music-lofi",
          "a catalog not yet read picks the first bundled pack");
    auto ambient = pack(L"music", L"msime-music-ambient");
    ambient.builtin = true;
    auto lofi = pack(L"music", L"msime-music-lofi");
    lofi.builtin = true;
    auto mine = pack(L"music", L"mine");
    check(music_pack_to_select({mine, ambient, lofi}, true) == L"msime-music-lofi",
          "bundled packs win, in their own order");
    check(music_pack_to_select({mine, ambient}, true) == L"msime-music-ambient",
          "the other bundled pack when the first is gone");
    auto imitation = pack(L"music", L"msime-music-lofi");
    check(music_pack_to_select({pack(L"sound", L"s"), imitation, mine}, true) == L"msime-music-lofi" &&
              music_pack_to_select({pack(L"sound", L"s"), mine}, true) == L"mine",
          "otherwise the first music pack in the catalog");
    check(music_pack_to_select({pack(L"sound", L"s")}, true).empty(),
          "nothing to pick when the catalog lists no music pack");
  }

  // 插件库：分类、筛选、搜索和分节。
  {
    auto click = pack(L"sound", L"click", L"keys");
    click.author = L"Alice";
    click.description = L"清脆的机械键盘";
    auto tune = pack(L"sound", L"tune", L"sequence");
    tune.name = L"旋律";
    auto dates = pack(L"command_table", L"dates");
    auto runner = pack(L"script", L"runner");
    const std::vector<Package> packages{click, tune, dates, runner};
    const std::vector<MissingSelection> absent{{L"music", L"lofi", {L"当前音乐包"}, false}};
    const auto categories = library_categories(packages, absent);
    check((categories == std::vector<std::wstring>{L"sound", L"music", L"command_table", L"script"}),
          "categories follow the library order, the command table always among them");
    check((library_categories({}, {}) == std::vector<std::wstring>{L"command_table"}),
          "an empty catalog still lists command tables");
    check(library_count(packages, library_all) == 4 && library_count(packages, L"sound") == 2 &&
              library_count(packages, L"music") == 0,
          "chip counts are installed packs only");
    check(library_selected_kind(L"sound", categories) == L"sound" &&
              library_selected_kind(L"wordbook", categories) == L"all",
          "a filter whose kind is gone falls back to all");
    check(library_query(L"  ALice\u3000") == L"alice" && library_query(L" \t ").empty(), "queries are trimmed and folded");

    auto every = library_groups(packages, absent, categories, L"all", L"");
    check(every.size() == 4 && every[0].packs.size() == 2 && every[1].gone.size() == 1 && every[2].packs.size() == 1 &&
              every[3].kind == L"script",
          "every kind gets a section");
    auto by_author = library_groups(packages, absent, categories, L"all", library_query(L"alice"));
    check(by_author.size() == 1 && by_author[0].packs.size() == 1 && by_author[0].packs[0]->id == L"click",
          "search matches the author, and the empty command table section steps aside");
    check(library_groups(packages, absent, categories, L"all", library_query(L"机械")).size() == 1 &&
              library_groups(packages, absent, categories, L"all", library_query(L"音效包")).front().packs.size() == 2,
          "search matches description and kind label");
    auto by_id = library_groups(packages, absent, categories, L"all", library_query(L"TUNE"));
    check(by_id.size() == 1 && by_id[0].packs.size() == 1 && by_id[0].packs[0]->id == L"tune" && by_id[0].gone.empty(),
          "search matches the id when the name does not contain it");
    auto missing_hit = library_groups(packages, absent, categories, L"all", library_query(L"未找到"));
    check(missing_hit.size() == 1 && missing_hit[0].kind == L"music" && missing_hit[0].gone.size() == 1,
          "search matches a missing selection by its title");
    auto sounds = library_groups(packages, absent, categories, L"sound", L"");
    check(sounds.size() == 1 && sounds[0].kind == L"sound", "a kind filter keeps one section");
    auto commands = library_groups({}, {}, library_categories({}, {}), L"all", L"");
    check(commands.size() == 1 && commands[0].packs.empty(), "the command table section stays to explain / commands");
    check(library_groups(packages, absent, categories, L"all", library_query(L"nothing")).empty() &&
              library_empty_message(L"nothing") == L"没有匹配的插件，试试其他关键词。" &&
              library_empty_message(L"") == L"没有插件",
          "empty states");
    check(library_section_title(L"script") == L"脚本插件（实验）" && library_section_title(L"music") == L"音乐包",
          "section titles");
    check(command_table_note().find(L"最多启用 16 个。") != std::wstring::npos, "command table note");
  }

  // 插件编辑器（`plugin-editor-model.ts` 和 `plugin-editor.test.tsx` 的模型用例）。
  {
    check(authored_kinds.size() == 4 && authored_kinds[0].kind == L"phrase_table" && authored_kinds[3].title == L"单词本",
          "the four editable kinds in the create order");
    check(is_authored_kind(L"symbol_set") && !is_authored_kind(L"sound") && !is_authored_kind(L"script"),
          "only text packs are editable");
    auto mine = pack(L"phrase_table", L"my-1a2b3c4d");
    auto official = pack(L"phrase_table", L"latex-symbols");
    official.builtin = true;
    check(pack_editable(mine) && !pack_editable(official) && !pack_editable(pack(L"music", L"m")),
          "the edit button is for user text packs");
    check(error_message(L"plugin_exists", L"", L"x") == L"本机已经有一个同样 id 的插件，没有覆盖它。" &&
              error_message(L"plugin_not_editable", L"内置插件", L"x") == L"这个插件不能在设置页编辑（内置插件）。",
          "editor failure codes");

    const auto fresh = empty_draft(L"phrase_table");
    check(fresh.license == L"CC0-1.0" && fresh.version == L"1.0.0" && !fresh.id && fresh.phrases.size() == 1 &&
              fresh.commands.empty(),
          "a new draft has the default license and version and one blank row");
    check(draft_issue(fresh) == std::optional<std::wstring>(L"请给插件起个名字。"), "a name comes first");

    // 单词本的名字和单词按词书的上限，空行不保存。
    auto words = empty_draft(L"wordbook");
    words.name = std::wstring(65, L'x');
    words.words = {{L"apple", L" ", L"苹果"}, {L"", L"", L""}};
    check(draft_issue(words) == std::optional<std::wstring>(L"单词本的名字不能超过 64 个字。"), "wordbook name limit");
    words.name = L"合成";
    check(!draft_issue(words), "a valid wordbook");
    const auto saved_words = pack_from_draft(words);
    check(saved_words.words.size() == 1 && saved_words.words[0].word == L"apple" && !saved_words.words[0].phonetic &&
              saved_words.words[0].gloss == L"苹果" && saved_words.phrases.empty(),
          "blank rows and blank phonetics are dropped");
    auto tagged = words;
    tagged.words = {{L"#tag", L"", L"x"}};
    check(draft_issue(tagged) == std::optional<std::wstring>(L"第 1 个单词不能以 # 开头。"), "a word cannot start with #");
    tagged.words = {{L"a", L"", L"x"}, {L"a", L"", L"y"}};
    check(draft_issue(tagged) == std::optional<std::wstring>(L"「a」出现了不止一次。"), "duplicate words");

    auto phrases = empty_draft(L"phrase_table");
    phrases.name = L"合成";
    check(draft_issue(phrases) == std::optional<std::wstring>(L"至少要有一条短语。"), "a phrase table needs a phrase");
    phrases.phrases = {{L"DH", L"电话"}};
    check(draft_issue(phrases) == std::optional<std::wstring>(L"第 1 条短语的编码只能是 1 到 32 个小写字母。"),
          "phrase keys are lowercase letters");
    phrases.phrases = {{L"dh", L"电话"}, {L"", L""}, {L"dh", L" 电话 "}};
    check(draft_issue(phrases) == std::optional<std::wstring>(L"第 3 条短语和前面的一条重复了。"),
          "duplicates compare trimmed, row numbers count blank rows");
    phrases.phrases = {{L"dh", L"电话"}};
    phrases.license = L"随便用";
    check(draft_issue(phrases) == std::optional<std::wstring>(L"许可证只能写 SPDX 许可证代码，例如 CC0-1.0、CC-BY-4.0。"),
          "licenses are SPDX codes");
    phrases.license = L"CC-BY-4.0";
    phrases.author = L"甲\n";
    check(draft_issue(phrases) == std::optional<std::wstring>(L"作者太长了。"), "control characters in the author");
    phrases.author.clear();
    phrases.phrases = {{L"dh", std::wstring(200, L'x')}};
    check(draft_issue(phrases) == std::optional<std::wstring>(L"第 1 条短语的文字太长了。"), "phrase text limit");
    phrases.phrases = {{L"dh", L"a\tb"}};
    check(draft_issue(phrases) == std::optional<std::wstring>(L"第 1 条短语的文字里不能有换行或制表符。"),
          "phrase text cannot hold a tab");
    phrases.name = std::wstring(27, L'中');
    check(draft_issue(phrases) == std::optional<std::wstring>(L"名字太长了。"), "names are limited in UTF-8 bytes");

    auto commands = empty_draft(L"command_table");
    commands.name = L"合成";
    commands.commands = {{L"x", L"x", L"{clipboard}"}};
    check(draft_issue(commands) == std::optional<std::wstring>(L"第 1 条指令的花括号里只能写 {date}、{time} 或 {weekday}。"),
          "unknown placeholders");
    commands.commands = {{L"sig", L"签名", L"{date:%m月%d日} {weekday} {time}"}, {L"sig", L"x", L"y"}};
    check(draft_issue(commands) == std::optional<std::wstring>(L"指令 /sig 重复了。"), "duplicate triggers");
    commands.commands.pop_back();
    check(!draft_issue(commands), "known placeholders, with formats");
    check(!template_placeholders_known(L"{date") && !template_placeholders_known(L"}") &&
              !template_placeholders_known(L"{{date}}") && !template_placeholders_known(L"{dates}") &&
              template_placeholders_known(L"无占位符"),
          "braces pair up without nesting");

    auto symbols = empty_draft(L"symbol_set");
    symbols.name = L"合成";
    symbols.groups = {{L"symbols", L"同名", L"", L"→"}, {L"symbols", L"同名", L"", L"←"}};
    check(draft_issue(symbols) == std::optional<std::wstring>(L"第 2 组的标题和同一页的另一组重复了。"),
          "group titles are unique per tab");
    symbols.groups[1].tab = L"kaomoji";
    check(!draft_issue(symbols), "the same title on another tab");
    symbols.groups = {{L"kaomoji", L"开心", L"", L"(^_^)\n\n  ( ﾟ∀ﾟ)  \n"}, {L"symbols", L"箭头", L"jiantou", L"→\n→"}};
    check(draft_issue(symbols) == std::optional<std::wstring>(L"第 2 组里「→」重复了。"), "duplicate symbols");
    symbols.groups[1].items = L"→\n←";
    const auto saved_symbols = pack_from_draft(symbols);
    check(saved_symbols.groups.size() == 2 && saved_symbols.groups[0].tab == L"kaomoji" &&
              (saved_symbols.groups[0].items == std::vector<std::wstring>{L"(^_^)", L"( ﾟ∀ﾟ)"}) &&
              !saved_symbols.groups[0].keywords && saved_symbols.groups[1].keywords == std::optional<std::wstring>(L"jiantou"),
          "symbols are one per line, trimmed, blank lines skipped");

    // 读出的包变成草稿，再整理回去不变。
    AuthoredPack loaded;
    loaded.kind = L"symbol_set";
    loaded.id = L"my-1a2b3c4d";
    loaded.name = L"合成";
    loaded.version = L"2.0.0";
    loaded.license = L"CC-BY-4.0";
    loaded.author = L"甲";
    loaded.groups = {{L"symbols", L"箭头", std::nullopt, {L"→", L"←"}}};
    const auto draft = draft_from_pack(loaded);
    check(draft.id == loaded.id && draft.author == L"甲" && draft.description.empty() && draft.groups.size() == 1 &&
              draft.groups[0].items == L"→\n←" && draft.groups[0].keywords.empty(),
          "a loaded pack becomes a draft");
    const auto again = pack_from_draft(draft);
    check(again.id == loaded.id && again.author == loaded.author && !again.description &&
              again.groups[0].items == loaded.groups[0].items,
          "and saves back unchanged");
    auto changed = draft;
    check(changed == draft, "an untouched draft is not dirty");
    changed.groups[0].items += L"\n↑";
    check(changed != draft, "an edited draft is dirty");

    // 批量粘贴。
    const auto pasted = parse_phrase_lines(L"dz  合成 地址 \n\n dh 电话");
    check(!pasted.issue && pasted.rows.size() == 2 && pasted.rows[0].key == L"dz" && pasted.rows[0].text == L"合成 地址" &&
              pasted.rows[1].key == L"dh" && pasted.rows[1].text == L"电话",
          "phrase lines: key, then the rest of the line");
    check(parse_phrase_lines(L"dh 电话\r\nnospace").issue ==
              std::optional<std::wstring>(L"第 2 行要写成「编码 文字」，中间用空格隔开。"),
          "a phrase line without a space");
    const auto blank_text_row = parse_phrase_lines(L"dz   ");
    check(!blank_text_row.issue && blank_text_row.rows.size() == 1 && draft_issue([&] {
            auto value = empty_draft(L"phrase_table");
            value.name = L"合成";
            value.phrases = blank_text_row.rows;
            return value;
          }()) == std::optional<std::wstring>(L"第 1 条短语还没有填写文字。"),
          "only spaces after the key read as a phrase without text, as the regular expression does");
    const auto pasted_words = parse_word_lines(L"# 合成词表\napple\t/ˈæp.əl/\tn. 苹果\nbanana\tn. 香蕉\n");
    check(!pasted_words.issue && pasted_words.rows.size() == 2 && pasted_words.rows[0].phonetic == L"/ˈæp.əl/" &&
              pasted_words.rows[1].phonetic.empty() && pasted_words.rows[1].gloss == L"n. 香蕉",
          "word lines: two or three tab separated columns, # lines skipped");
    check(parse_word_lines(L"a\tb\tc\td").issue->find(L"第 1 行") != std::wstring::npos, "four columns are refused");
    const auto appended = append_rows(fresh.phrases, pasted.rows, blank_phrase);
    check(appended.size() == 2 && appended[0].key == L"dz", "pasted rows replace a lone blank row");
    check(append_rows(appended, pasted.rows, blank_phrase).size() == 4, "and are appended after real rows");

    // 保存之后的提醒。
    check(authored_save_note(L"phrase_table", false, true) ==
              L"已保存。打开下面的「启用」后，按 Shift+K 再输入编码就能用到这些短语。" &&
              authored_save_note(L"phrase_table", true, true) == L"已保存，修改已经生效。" &&
              authored_save_note(L"command_table", true, false) ==
                  L"已保存。/ 指令现在关着，在「输入 → 快捷模式」打开后才能用。" &&
              authored_save_note(L"wordbook", false, false) == L"已保存，这本书出现在背单词的词书里。",
          "what is still missing after a save");
    auto saved = pack(L"phrase_table", L"my-1a2b3c4d");
    saved.name = L"我的短语";
    check(saved_notice(saved) == L"已保存短语表「我的短语」。", "saved notice");

    // Win7 设置的整段文本：写出去再读回来是同一份内容。
    check(phrase_text({{L"dz", L"合成 地址"}, {L"", L""}, {L"dh", L"电话"}}) == L"dz 合成 地址\r\ndh 电话",
          "phrases as text");
    check(command_text({{L"sig", L"签名", L"{date} 张三"}}) == L"sig\t签名\t{date} 张三", "commands as text");
    check(word_text({{L"apple", L"", L"苹果"}, {L"pear", L"/peə/", L"梨"}}) == L"apple\t苹果\r\npear\t/peə/\t梨",
          "words as text");
    const auto groups_text = group_text(symbols.groups);
    check(groups_text == L"[颜文字] 开心\r\n(^_^)\r\n( ﾟ∀ﾟ)\r\n[符号] 箭头\tjiantou\r\n→\r\n←", "groups as text");
    auto reread = empty_draft(L"symbol_set");
    reread.name = L"合成";
    check(!read_authored_content(reread, groups_text) && pack_from_draft(reread).groups.size() == 2 &&
              pack_from_draft(reread).groups[1].items == std::vector<std::wstring>({L"→", L"←"}) &&
              pack_from_draft(reread).groups[1].keywords == std::optional<std::wstring>(L"jiantou") &&
              reread.groups[0].tab == L"kaomoji",
          "groups read back from text");
    check(parse_group_lines(L"→\n[符号] 箭头").issue ==
              std::optional<std::wstring>(L"第 1 行之前还没有组标题，先写一行「[符号] 标题」或「[颜文字] 标题」。"),
          "symbols before a header");
    auto reread_commands = empty_draft(L"command_table");
    check(read_authored_content(reread_commands, L"sig\t签名") ==
              std::optional<std::wstring>(L"第 1 行要写成「指令、标题、要插入的文字」三列，列之间用 Tab 隔开。") &&
              reread_commands.commands.size() == 1 && blank_command(reread_commands.commands[0]),
          "an unreadable text leaves the draft alone");
    check(!read_authored_content(reread_commands, L"sig\t签名\t{date} 张三\r\n") && reread_commands.commands.size() == 1 &&
              reread_commands.commands[0].template_text == L"{date} 张三",
          "commands read back from text");
    check(authored_text_format(L"symbol_set").find(L"[符号]") != std::wstring::npos && authored_text_format(L"x").empty(),
          "each kind says how to write it");
    check(utf8_length(L"aé中") == 6 && code_points(L"\U0001F642a") == 2 && utf8_length(L"\U0001F642") == 4,
          "UTF-8 bytes and code points");
  }

  // 成就音效：统计关着时不能打开，已经打开的可以关掉；不知道统计开没开时按打开处理。
  check(achievements_blocked(false, false) && !achievements_blocked(false, true) &&
            !achievements_blocked(true, false) && !achievements_blocked(std::nullopt, false),
        "the achievement jingle waits for typing statistics");

  if (failures == 0)
    std::puts("plugin page model: ok");
  return failures == 0 ? 0 : 1;
}
