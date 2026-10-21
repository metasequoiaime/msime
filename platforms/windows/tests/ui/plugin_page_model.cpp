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
  // provide_commands 是第 8 项：排在最后，没有依赖、不和任何一项互斥，页面认得它，不会把它当成不认识的能力丢掉。
  check(script_capabilities.size() == 8 && script_capabilities.back() == L"provide_commands",
        "provide_commands is the eighth capability");
  check((sort_capabilities({L"provide_commands", L"read_commit"}) == Caps{L"read_commit", L"provide_commands"}),
        "provide_commands sorts last");
  check(capability_requires(L"provide_commands").empty() && exclusive_partner(L"provide_commands").empty(),
        "provide_commands needs nothing and excludes nothing");
  check((toggle_capability({L"read_commit"}, L"provide_commands", true, all) ==
         Caps{L"read_commit", L"provide_commands"}),
        "checking provide_commands keeps the rest");

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

  // 成就音效：统计关着时不能打开，已经打开的可以关掉；不知道统计开没开时按打开处理。
  check(achievements_blocked(false, false) && !achievements_blocked(false, true) &&
            !achievements_blocked(true, false) && !achievements_blocked(std::nullopt, false),
        "the achievement jingle waits for typing statistics");

  if (failures == 0)
    std::puts("plugin page model: ok");
  return failures == 0 ? 0 : 1;
}
