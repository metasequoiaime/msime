import app.msime.android.home.HelpcodeChoicePolicy;
import app.msime.android.home.HelpcodeChoicePolicy.Change;
import app.msime.android.home.HelpcodeChoicePolicy.Decoded;
import app.msime.android.home.HelpcodeChoicePolicy.Option;
import app.msime.android.home.HelpcodeChoicePolicy.Pack;
import java.nio.charset.StandardCharsets;
import java.util.List;

/**
 * 「输入 › 辅助码方案」里内置方案和已导入码表的选择规则：列表项、行尾文字、选择时偏好怎么改、删除时清不清选择，以及码表文件的严格解码。读写偏好和调用 JNI 的那一半在 `TypingPage` 里，要 org.json 和设备，这里跑不了。
 */
public final class HelpcodeChoicePolicySmoke {
    public static void main(String[] arguments) {
        List<Pack> packs = List.of(new Pack("custom-mine", "我的码"), new Pack("custom-0123456789ab", "部首"));

        // 包选择键跟着当前方案走。
        check(HelpcodeChoicePolicy.packKey(true).equals("helpcode_pack_shuangpin"), "shuangpin key");
        check(HelpcodeChoicePolicy.packKey(false).equals("helpcode_pack_quanpin"), "quanpin key");
        check(HelpcodeChoicePolicy.PACK_KEYS.contains(HelpcodeChoicePolicy.packKey(true))
            && HelpcodeChoicePolicy.PACK_KEYS.contains(HelpcodeChoicePolicy.packKey(false)), "both keys are cleared on removal");

        // 列表项：每个包一项，选中的包不在了补一项「未找到」。
        List<Option> options = HelpcodeChoicePolicy.packOptions(packs, "");
        check(options.size() == 2, "one option per pack");
        check(options.get(0).value().equals("pack:custom-mine") && options.get(0).label().equals("我的码（插件）"),
            "a pack option names the pack");
        List<Option> missing = HelpcodeChoicePolicy.packOptions(packs, "custom-gone");
        check(missing.size() == 3 && missing.get(2).value().equals("pack:custom-gone")
            && missing.get(2).label().equals("custom-gone（插件，未找到）"), "a selected pack that is gone stays listed");
        check(HelpcodeChoicePolicy.packOptions(packs, "custom-mine").size() == 2, "a selected pack that exists is not listed twice");

        // 行尾文字：选了包时是包，否则是内置方案。
        check(HelpcodeChoicePolicy.rowValue(packs, "", "蓝天小雨点").equals("蓝天小雨点"), "no pack shows the schema");
        check(HelpcodeChoicePolicy.rowValue(packs, "custom-mine", "蓝天小雨点").equals("我的码（插件）"), "a pack shows its name");
        check(HelpcodeChoicePolicy.rowValue(packs, "custom-gone", "蓝天小雨点").equals("custom-gone（插件，未找到）"),
            "a missing pack says so");
        check(HelpcodeChoicePolicy.selectedValue("lantian", "").equals("lantian"), "the schema is selected without a pack");
        check(HelpcodeChoicePolicy.selectedValue("lantian", "custom-mine").equals("pack:custom-mine"), "the pack is selected");

        // 选包只改包的选择；选内置方案写方案，原来选着包时清成空字符串，没选包时不碰 plugins。
        Change pack = HelpcodeChoicePolicy.choose("pack:custom-mine", "");
        check(pack.schema() == null && "custom-mine".equals(pack.pack()), "choosing a pack selects it and keeps the schema");
        Change builtin = HelpcodeChoicePolicy.choose("xiaohe", "custom-mine");
        check("xiaohe".equals(builtin.schema()) && "".equals(builtin.pack()), "choosing a built-in schema clears the pack");
        Change plain = HelpcodeChoicePolicy.choose("xiaohe", "");
        check("xiaohe".equals(plain.schema()) && plain.pack() == null, "without a pack only the schema changes");

        // 删除：只有选的正是被删的包时才清。
        check(HelpcodeChoicePolicy.clearsSelection("custom-mine", "custom-mine"), "removing the selected pack clears it");
        check(!HelpcodeChoicePolicy.clearsSelection("custom-mine", "custom-other"), "another selection stays");
        check(!HelpcodeChoicePolicy.clearsSelection("custom-mine", ""), "nothing selected, nothing to clear");

        // 严格解码：UTF-8（含 BOM）原样交出，GBK 和超长文件在调 JNI 之前就拒绝。
        Decoded utf8 = HelpcodeChoicePolicy.decode("\uFEFF你=ab\n".getBytes(StandardCharsets.UTF_8));
        check(utf8.failure() == null && "\uFEFF你=ab\n".equals(utf8.text()), "UTF-8 decodes as is");
        Decoded gbk = HelpcodeChoicePolicy.decode(new byte[] {(byte) 0xC4, (byte) 0xE3, '=', 'a', 'b', '\n'});
        check(gbk.text() == null && gbk.failure().contains("UTF-8"), "GBK is refused before the core sees it");
        Decoded large = HelpcodeChoicePolicy.decode(null);
        check(large.text() == null && large.failure().contains("1 MiB"), "an oversized file is refused");

        // 提示文字。
        check(HelpcodeChoicePolicy.importedMessage("我的码", 3, true).equals("已导入「我的码」，共 3 条，已切换到这套辅助码"),
            "import and switch");
        check(HelpcodeChoicePolicy.importedMessage("我的码", 3, false).contains("切换失败"), "import without the switch");
        check(HelpcodeChoicePolicy.importFailure("plugin_invalid", "mine.txt 第 2 行的码必须是 1 到 2 个小写字母")
            .equals("无法导入：mine.txt 第 2 行的码必须是 1 到 2 个小写字母"), "the core's reason reaches the user");
        check(HelpcodeChoicePolicy.importFailure("plugin_invalid", null).equals("导入失败，请重试"), "no detail, generic text");
        check(HelpcodeChoicePolicy.importFailure("invalid", null).contains("UTF-8"), "a refused request");
        check(HelpcodeChoicePolicy.importFailure("plugin_storage", null).contains("保存"), "storage");
        check(HelpcodeChoicePolicy.removedMessage("我的码", true).contains("内置方案"), "removing the selection says where it went");
        check(HelpcodeChoicePolicy.removedMessage("我的码", false).equals("已删除「我的码」"), "plain removal");

        System.out.println("Android helpcode choice policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
