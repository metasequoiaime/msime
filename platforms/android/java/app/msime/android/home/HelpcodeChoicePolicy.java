package app.msime.android.home;

import app.msime.android.TextPolicy;
import java.nio.charset.CharacterCodingException;
import java.util.ArrayList;
import java.util.List;

/**
 * 「输入 › 辅助码方案」里内置方案和已导入码表的选择规则，与共享设置页 `helpcode-page.tsx` 相同。
 *
 * <p>导入的码表是 `kind = "helpcode"` 的插件包（`msime_client_plugins` 的 `import_helpcode_table`），选中时写进当前方案那一份的 `plugins.helpcode_pack_quanpin` 或 `helpcode_pack_shuangpin`，`*_helpcode.schema` 不动；选内置方案时写 `schema`，并把包的选择清成空字符串。包载入不了时 host-api 退回 `schema`，所以清掉选择总是安全的。这里是其中不碰 Android 和 JSON 的部分，check-host 直接编译并在 JVM 里测它。
 */
public final class HelpcodeChoicePolicy {
    private HelpcodeChoicePolicy() {}

    /** 方案列表里已导入码表的值前缀：`pack:<包 id>`，与共享设置页相同。 */
    public static final String PACK_PREFIX = "pack:";
    /** 码表文件的字节上限，即 client-services 的 `helpcode_pack::MAX_TABLE_BYTES`。超过它就不读进内存、不调 JNI。 */
    public static final int MAX_TABLE_BYTES = 1024 * 1024;
    /** 两份方案各自的包选择键。 */
    public static final List<String> PACK_KEYS = List.of("helpcode_pack_quanpin", "helpcode_pack_shuangpin");

    /** 一个已导入的码表：包 id 和显示名。 */
    public record Pack(String id, String name) {}

    /** 方案列表里的一项：写回的值和显示的文字。 */
    public record Option(String value, String label) {}

    /**
     * 一次选择对偏好的改动。
     *
     * @param schema 要写进 `*_helpcode.schema` 的内置方案；为 null 时不改
     * @param pack 要写进 `plugins.helpcode_pack_*` 的包 id，空字符串表示清掉；为 null 时不改
     */
    public record Change(String schema, String pack) {}

    /** 码表文件严格解码的结果：成功时 `text` 非 null，失败时 `failure` 是给用户看的原因。 */
    public record Decoded(String text, String failure) {}

    /** 当前方案的包选择键：双拼用 `helpcode_pack_shuangpin`，其余用 `helpcode_pack_quanpin`。 */
    public static String packKey(boolean shuangpin) {
        return shuangpin ? "helpcode_pack_shuangpin" : "helpcode_pack_quanpin";
    }

    public static String packValue(String id) {
        return PACK_PREFIX + id;
    }

    /** 列表里的已导入码表：每个包一项；选中的包已经不在了时补一项「id（插件，未找到）」，让用户看得见当前选择并能改掉它。 */
    public static List<Option> packOptions(List<Pack> packs, String selectedPack) {
        List<Option> options = new ArrayList<>(packs.size() + 1);
        boolean found = false;
        for (Pack pack : packs) {
            options.add(new Option(packValue(pack.id()), pack.name() + "（插件）"));
            found |= pack.id().equals(selectedPack);
        }
        if (!selectedPack.isEmpty() && !found) {
            options.add(new Option(packValue(selectedPack), selectedPack + "（插件，未找到）"));
        }
        return options;
    }

    /** 「辅助码方案」行尾的文字：选了包时是包那一项的文字，否则是内置方案的名字。 */
    public static String rowValue(List<Pack> packs, String selectedPack, String schemaLabel) {
        if (selectedPack.isEmpty()) return schemaLabel;
        String value = packValue(selectedPack);
        for (Option option : packOptions(packs, selectedPack)) {
            if (option.value().equals(value)) return option.label();
        }
        return schemaLabel;
    }

    /** 当前选中的那一项的值：选了包时是 `pack:<id>`，否则是方案。 */
    public static String selectedValue(String schema, String selectedPack) {
        return selectedPack.isEmpty() ? schema : packValue(selectedPack);
    }

    /** 在列表里选了 `value`：选包只改包的选择；选内置方案写方案，原来选着包时一并清掉。 */
    public static Change choose(String value, String selectedPack) {
        if (value.startsWith(PACK_PREFIX)) return new Change(null, value.substring(PACK_PREFIX.length()));
        return new Change(value, selectedPack.isEmpty() ? null : "");
    }

    /** 删除包 `removedId` 时这一份选择要不要清掉：选的正是它才清，清掉后回到内置方案。 */
    public static boolean clearsSelection(String removedId, String selectedPack) {
        return !selectedPack.isEmpty() && selectedPack.equals(removedId);
    }

    /** 读到的码表字节严格按 UTF-8 解码。`bytes` 为 null 表示超过了 {@link #MAX_TABLE_BYTES}。不宽松解码：GBK 码表被替换成 U+FFFD 后，报出来的会是「等号左边必须恰好是一个字」这种看不出原因的错误。 */
    public static Decoded decode(byte[] bytes) {
        if (bytes == null) return new Decoded(null, "码表超过 1 MiB，无法导入");
        try {
            return new Decoded(TextPolicy.utf8Strict(bytes), null);
        } catch (CharacterCodingException error) {
            return new Decoded(null, "码表不是 UTF-8 编码，请先另存为 UTF-8 再导入");
        }
    }

    /** 导入成功的提示；`selected` 为 false 时导入成功但没能切换过去。 */
    public static String importedMessage(String name, long entries, boolean selected) {
        String imported = "已导入「" + name + "」，共 " + entries + " 条";
        return selected ? imported + "，已切换到这套辅助码" : imported + "；切换失败，请在列表里选择它";
    }

    /** `import_helpcode_table` 失败时的提示：码表不合格时把 client-services 给的原因（带文件名和行号）原样告诉用户。 */
    public static String importFailure(String error, String detail) {
        if ("plugin_invalid".equals(error) && detail != null && !detail.isEmpty()) return "无法导入：" + detail;
        if ("plugin_storage".equals(error) || "storage".equals(error)) return "保存码表失败，请重试";
        if ("invalid".equals(error)) return "无法导入这个码表，请确认它是 1 MiB 以内的 UTF-8 文本";
        return "导入失败，请重试";
    }

    /** 删除成功的提示；`wasSelected` 时说明已经回到内置方案。 */
    public static String removedMessage(String name, boolean wasSelected) {
        return wasSelected ? "已删除「" + name + "」，已改回内置方案" : "已删除「" + name + "」";
    }
}
