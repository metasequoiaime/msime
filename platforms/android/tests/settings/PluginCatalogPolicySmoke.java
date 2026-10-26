import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginCatalogPolicy.Missing;
import app.msime.android.policy.PluginCatalogPolicy.Pack;
import app.msime.android.policy.PluginCatalogPolicy.Selection;
import java.util.List;

/**
 * 插件页的列表规则：手机上生效的类型、用不上的包的计数、行尾标记、缺失的选择、启用顺序和失败句子。读目录、读写偏好的那一半在 `home/PluginsPage` 等页面里，要 org.json 和设备，这里跑不了。
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

        Selection on = new Selection("default", List.of("addr"), List.of("sig", "gone"), "custom-mine", "", true, true, false);
        check("使用中".equals(PluginCatalogPolicy.marker(keys, on)), "the selected key pack is in use");
        check("已选 · 按键音未开".equals(PluginCatalogPolicy.marker(keys,
            new Selection("default", List.of(), List.of(), "", "", false, true, false))), "with the switch off it says so");
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
        Selection tune = new Selection("my-tune", List.of(), List.of(), "x", "x", true, true, true);
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

    private static Pack pack(String kind, String id, boolean builtin, String mode) {
        return new Pack(kind, id, id, "1.0.0", "CC0-1.0", "", "", builtin, mode);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
