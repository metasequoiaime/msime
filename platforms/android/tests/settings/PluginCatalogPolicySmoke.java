import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginCatalogPolicy.Missing;
import app.msime.android.policy.PluginCatalogPolicy.Pack;
import app.msime.android.policy.PluginCatalogPolicy.Selection;
import java.util.ArrayList;
import java.util.List;

/**
 * 插件页的列表规则：手机上生效的类型、用不上的包的计数、行尾标记、缺失的选择、启用顺序、列表行上的开关、展示顺序（`plugins.plugin_order`）和失败句子。读目录、读写偏好的那一半在 `home/PluginsPage` 等页面里，要 org.json 和设备，这里跑不了。
 */
public final class PluginCatalogPolicySmoke {
    public static void main(String[] arguments) {
        Pack keys = pack("sound", "default", true, "keys");
        Pack melody = pack("sound", "twinkle", true, "sequence");
        Pack myMelody = pack("sound", "my-tune", false, "sequence");
        Pack music = pack("music", "msime-music-lofi", true, "");
        Pack effect = pack("effect", "glow", false, "");
        Pack phrases = pack("phrase_table", "addr", false, "");
        Pack commands = pack("command_table", "sig", false, "");
        Pack helpcode = pack("helpcode", "custom-mine", false, "");
        Pack wordbook = pack("wordbook", "cet4", false, "");
        Pack symbols = pack("symbol_set", "arrows", false, "");

        // 手机上生效的：按键音效包、指令表、短语表、辅助码表、单词本、符号集；旋律、音乐、特效不列。
        check(PluginCatalogPolicy.usedOnPhone(keys) && !PluginCatalogPolicy.usedOnPhone(melody)
            && !PluginCatalogPolicy.usedOnPhone(music) && !PluginCatalogPolicy.usedOnPhone(effect)
            && PluginCatalogPolicy.usedOnPhone(phrases) && PluginCatalogPolicy.usedOnPhone(symbols), "phone kinds");
        check(PluginCatalogPolicy.unusedCount(List.of(keys, melody, myMelody, music, effect, phrases)) == 2,
            "built-in melody and music are not counted, the user's melody and effect are");
        check(PluginCatalogPolicy.unusedNote(0) == null && PluginCatalogPolicy.unusedNote(2).startsWith("另有 2 个"),
            "the unused line only when there is something");
        check(PluginCatalogPolicy.communityKindShown("wordbook") && !PluginCatalogPolicy.communityKindShown("music")
            && !PluginCatalogPolicy.communityKindShown("effect"), "community lists phone kinds only");
        check(PluginCatalogPolicy.kindLabel("helpcode").equals("辅助码表")
            && PluginCatalogPolicy.packKindLabel(myMelody).equals("音效包·旋律"), "kind labels");

        Selection on = new Selection("default", List.of("addr"), List.of("sig", "gone"), "custom-mine", "", true, true, false, List.of());
        check("使用中".equals(PluginCatalogPolicy.marker(keys, on)), "the selected key pack is in use");
        check("已选 · 按键音未开".equals(PluginCatalogPolicy.marker(keys,
            new Selection("default", List.of(), List.of(), "", "", false, true, false, List.of()))), "with the switch off it says so");
        check("已启用 · 第 1 位".equals(PluginCatalogPolicy.marker(phrases, on)), "an enabled phrase table and its place");
        check("已启用 · / 指令未开".equals(PluginCatalogPolicy.marker(commands, on)), "an enabled command table with / off");
        check("用于全拼".equals(PluginCatalogPolicy.marker(helpcode, on)), "a helpcode pack names its scheme");
        check(PluginCatalogPolicy.marker(wordbook, on) == null && PluginCatalogPolicy.marker(symbols, on) == null,
            "wordbooks and symbol sets have no marker");

        // 缺失：指令表 gone 不在本机；按键音包选成了旋律包时是「不可用」。
        List<Missing> missing = PluginCatalogPolicy.missingSelections(on, List.of(keys, phrases, commands, helpcode));
        check(missing.size() == 1 && missing.get(0).id().equals("gone") && !missing.get(0).mismatched()
            && PluginCatalogPolicy.missingTitle(missing.get(0)).equals("gone（未找到）")
            && PluginCatalogPolicy.missingAction(missing.get(0)).equals("移除"), "an enabled table that is gone");
        Selection tune = new Selection("my-tune", List.of(), List.of(), "x", "x", true, true, true, List.of());
        List<Missing> wrong = PluginCatalogPolicy.missingSelections(tune, List.of(myMelody));
        check(wrong.size() == 2 && wrong.get(0).mismatched() && wrong.get(0).kind().equals("sound")
            && PluginCatalogPolicy.missingTitle(wrong.get(0)).equals("my-tune（不可用）")
            && wrong.get(1).uses().equals(List.of("全拼辅助码", "双拼辅助码")), "a melody as the key pack, and one entry per pack");
        check(PluginCatalogPolicy.missingNote(wrong.get(1)).contains("已不在本机"), "the missing note");

        // 启用顺序：启用的排到最后，最多 16 个。
        check(PluginCatalogPolicy.withTable(List.of("a", "b"), "a", true).equals(List.of("b", "a")), "re-enabling moves last");
        check(PluginCatalogPolicy.withTable(List.of("a", "b"), "a", false).equals(List.of("b")), "disabling removes");
        List<String> full = List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16");
        check(!PluginCatalogPolicy.tableSwitchEnabled(full, "x") && PluginCatalogPolicy.tableSwitchEnabled(full, "3"),
            "a full list still lets an enabled table go");
        check(PluginCatalogPolicy.tableSwitchDescription("phrase_table", full, "x").equals("最多启用 16 个短语表。")
            && PluginCatalogPolicy.tableSwitchDescription("command_table", List.of("a"), "a").equals("第 1 位。同一指令以靠前的表为准。"),
            "switch descriptions");

        // 列表行上的开关和详情页的「启用」用同一条规则：读哪张表、能不能点、点了以后的列表。
        check(on.tables("phrase_table").equals(List.of("addr")) && on.tables("command_table").equals(List.of("sig", "gone")),
            "a row reads the enabled tables of its own kind");
        check(PluginCatalogPolicy.switchedTables(List.of("a"), "b", true).equals(List.of("a", "b"))
            && PluginCatalogPolicy.switchedTables(List.of("a", "b"), "a", false).equals(List.of("b")),
            "the shared switch enables last and disables in place");
        check(PluginCatalogPolicy.switchedTables(full, "x", true).equals(full)
            && PluginCatalogPolicy.switchedTables(full, "3", false).size() == 15,
            "a full list is left alone by a stale switch, and still lets an enabled table go");
        check(PluginCatalogPolicy.tableSwitchLabel("phrase_table", List.of(), phrases).equals("启用「addr」")
            && PluginCatalogPolicy.tableSwitchLabel("phrase_table", full, phrases).equals("启用「addr」，最多启用 16 个短语表。"),
            "the row switch names its plugin, and says why it cannot be turned on");

        pluginOrder(keys, melody, myMelody, music, phrases, commands, wordbook);

        check(PluginCatalogPolicy.errorMessage("plugin_invalid", "缺少 plugin.toml", "x").equals("插件不符合要求（缺少 plugin.toml）。")
            && PluginCatalogPolicy.errorMessage("unknown", "", "导入失败，请重试。").equals("导入失败，请重试。"), "failure sentences");
        check(PluginCatalogPolicy.communityMessage("community_conflict", null, true).contains("20 个")
            && PluginCatalogPolicy.communityMessage("plugin_community_mismatch", null, false).equals("下载的插件与作品信息不符，已停止安装。")
            && PluginCatalogPolicy.communityMessage("plugin_invalid", "x", false).equals("插件不符合要求（x）。")
            && PluginCatalogPolicy.needsSignIn("community_unauthorized"), "community sentences");
        check(PluginCatalogPolicy.wordbookBookId("cet4").equals("pack-cet4"), "wordbook book id");
        check(PluginCatalogPolicy.megabytes(1024 * 1024 + 52_000).equals("1.0 MB"), "sizes");
        check(PluginCatalogPolicy.rowDescription(keys).equals("内置")
            && PluginCatalogPolicy.rowDescription(new Pack("wordbook", "w", "w", "1.0", "MIT", "甲", "", false, ""))
                .equals("1.0 · 作者 甲"), "row descriptions");
        check(PluginCatalogPolicy.publishNameValid(" 我的短语 ") && !PluginCatalogPolicy.publishNameValid("　")
            && !PluginCatalogPolicy.publishNameValid("字".repeat(33)) && PluginCatalogPolicy.publishNameValid("字".repeat(32))
            && PluginCatalogPolicy.publishDescriptionValid("") && !PluginCatalogPolicy.publishDescriptionValid("x".repeat(281)),
            "publish fields");
        System.out.println("PluginCatalogPolicySmoke ok");
    }

    /** 展示顺序 `plugins.plugin_order`：读时忽略未知、重复和已删除的键，没提到的按目录先后接在后面；拖动一组只换这一组的位置；删包去掉它的键；写回的一定通得过 client-core 的校验。 */
    private static void pluginOrder(Pack keys, Pack melody, Pack myMelody, Pack music, Pack phrases, Pack commands,
            Pack wordbook) {
        Pack clicky = pack("sound", "clicky", false, "keys");
        Pack work = pack("phrase_table", "work", false, "");
        Pack home = pack("phrase_table", "home", false, "");
        // 目录原来的先后；短语表和单词本各有一个与别的类型同名的 id。
        Pack sameId = pack("wordbook", "addr", false, "");
        List<Pack> catalog = List.of(keys, melody, clicky, myMelody, music, phrases, work, home, commands, wordbook, sameId);

        check(PluginCatalogPolicy.pluginKey("phrase_table", "addr").equals("phrase_table/addr"), "the key is kind/id");
        check(PluginCatalogPolicy.orderedPluginKeys(catalog, List.of()).equals(List.of("sound/default", "sound/twinkle",
            "sound/clicky", "sound/my-tune", "music/msime-music-lofi", "phrase_table/addr", "phrase_table/work",
            "phrase_table/home", "command_table/sig", "wordbook/cet4", "wordbook/addr")), "no stored order is the catalog order");
        List<String> stored = List.of("phrase_table/home", "phrase_table/gone", "script/x", "phrase_table/home", "not-a-key",
            "wordbook/addr", "phrase_table/addr");
        check(PluginCatalogPolicy.orderedPluginKeys(catalog, stored).equals(List.of("phrase_table/home", "wordbook/addr",
            "phrase_table/addr", "sound/default", "sound/twinkle", "sound/clicky", "sound/my-tune", "music/msime-music-lofi",
            "phrase_table/work", "command_table/sig", "wordbook/cet4")),
            "unknown, removed and repeated keys are ignored, the rest follow in catalog order");

        // 分组只列手机上生效的包；同名 id 的单词本不会被当成短语表。
        check(ids(PluginCatalogPolicy.groupInOrder(catalog, "phrase_table", stored)).equals(List.of("home", "addr", "work")),
            "a group follows the stored order, unmentioned packs last");
        check(ids(PluginCatalogPolicy.groupInOrder(catalog, "wordbook", stored)).equals(List.of("addr", "cet4")),
            "the same id of another kind has its own place");
        check(ids(PluginCatalogPolicy.groupInOrder(catalog, "sound", List.of("sound/my-tune", "sound/clicky")))
            .equals(List.of("clicky", "default")), "melodies are not listed on the phone");
        check(PluginCatalogPolicy.groupInOrder(catalog, "helpcode", stored).isEmpty(), "an empty group");

        // 拖动短语表：只换短语表占着的三个位置，别的组、手机上不列的包都留在原位。
        List<String> moved = PluginCatalogPolicy.reorderedPluginKeys(catalog, stored,
            List.of("phrase_table/work", "phrase_table/home", "phrase_table/addr"));
        check(moved.equals(List.of("phrase_table/work", "wordbook/addr", "phrase_table/home", "sound/default", "sound/twinkle",
            "sound/clicky", "sound/my-tune", "music/msime-music-lofi", "phrase_table/addr", "command_table/sig",
            "wordbook/cet4")), "a group's slots take the new order and the full list is written");
        check(ids(PluginCatalogPolicy.groupInOrder(catalog, "phrase_table", moved)).equals(List.of("work", "home", "addr"))
            && ids(PluginCatalogPolicy.groupInOrder(catalog, "wordbook", moved)).equals(List.of("addr", "cet4")),
            "reading it back gives the dragged order and leaves other groups alone");
        // 音效包组里按键旋律不列出：两个按键音效包对调，旋律包留在原位。
        check(PluginCatalogPolicy.reorderedPluginKeys(catalog, List.of(), List.of("sound/clicky", "sound/default")).subList(0, 4)
            .equals(List.of("sound/clicky", "sound/twinkle", "sound/default", "sound/my-tune")),
            "packs of the kind that are not listed keep their slots");
        check(PluginCatalogPolicy.reorderedPluginKeys(catalog, stored,
            List.of("phrase_table/work", "phrase_table/gone", "phrase_table/work", "phrase_table/home", "phrase_table/addr"))
            .equals(moved), "keys that are not in the catalog or repeat are ignored when writing");
        check(PluginCatalogPolicy.reorderedPluginKeys(List.of(), stored, List.of("phrase_table/home")).isEmpty(),
            "an empty catalog writes nothing, so the key is removed");

        // 写回的条目都过 client-core 的校验：kind/id 两段都是 safe_id、不重复、最多 256 条。
        check(PluginCatalogPolicy.safeId("a") && PluginCatalogPolicy.safeId("my-tune_2.0") && PluginCatalogPolicy.safeId("9x")
            && PluginCatalogPolicy.safeId("a".repeat(64)) && !PluginCatalogPolicy.safeId("a".repeat(65))
            && !PluginCatalogPolicy.safeId("") && !PluginCatalogPolicy.safeId(null) && !PluginCatalogPolicy.safeId("-a")
            && !PluginCatalogPolicy.safeId(".a") && !PluginCatalogPolicy.safeId("Abc") && !PluginCatalogPolicy.safeId("a/b")
            && !PluginCatalogPolicy.safeId("表") && !PluginCatalogPolicy.safeId("a b"), "safe_id");
        check(PluginCatalogPolicy.storablePluginOrder(List.of("sound/a", "sound", "/a", "sound/", "sound/a/b", "Sound/a",
            "sound/a", "wordbook/b")).equals(List.of("sound/a", "wordbook/b")), "only well-formed keys, once each");
        List<Pack> many = new ArrayList<>();
        for (int index = 0; index < 300; index++) many.add(pack("wordbook", "w" + index, false, ""));
        many.add(pack("wordbook", "Bad Id", false, ""));
        List<String> capped = PluginCatalogPolicy.reorderedPluginKeys(many, List.of(), List.of("wordbook/w1", "wordbook/w0"));
        check(capped.size() == PluginCatalogPolicy.MAX_PLUGIN_ORDER && capped.get(0).equals("wordbook/w1")
            && capped.get(1).equals("wordbook/w0") && capped.get(255).equals("wordbook/w255"), "at most 256 entries");
        check(ids(PluginCatalogPolicy.groupInOrder(many, "wordbook", capped)).subList(254, 258)
            .equals(List.of("w254", "w255", "w256", "w257")), "packs past the cap follow in catalog order");

        // 删包：去掉它的键，别的类型同名 id 的键留着。
        check(PluginCatalogPolicy.withoutPluginOrder(moved, "phrase_table", "addr").equals(List.of("phrase_table/work",
            "wordbook/addr", "phrase_table/home", "sound/default", "sound/twinkle", "sound/clicky", "sound/my-tune",
            "music/msime-music-lofi", "command_table/sig", "wordbook/cet4")), "removing a pack drops its key only");
        check(PluginCatalogPolicy.withoutPluginOrder(stored, "effect", "glow").equals(stored)
            && PluginCatalogPolicy.withoutPluginOrder(List.of("effect/glow"), "effect", "glow").isEmpty(),
            "a pack that was never ordered changes nothing, and the last key leaves an empty list");
    }

    private static List<String> ids(List<Pack> packs) {
        List<String> ids = new ArrayList<>();
        for (Pack pack : packs) ids.add(pack.id());
        return ids;
    }

    private static Pack pack(String kind, String id, boolean builtin, String mode) {
        return new Pack(kind, id, id, "1.0.0", "CC0-1.0", "", "", builtin, mode);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
