package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 五笔查码页（#6789）怎么排、怎么写：与共享设置页的 `wubi-code-lookup-section.tsx` 同一套文字和规则。
 *
 * <p>查码本身是词库请求 `{operation:"wubi_codes", text}`（host-api `wubi_codes_json`），设置应用进程直接读 `msime-wubi.db`，不经键盘、不排队。这里只管把回答排成行：整段词（有的话）在前，再逐字列出；每行写 86 和 98 两列编码，当前所选的码表标「（当前）」，码表里没有的写「—」。本类不碰 `org.json`，JVM 冒烟能直接跑。
 */
public final class WubiCodeLookupPolicy {
    /** 输入框最多收的字符数；宿主拒绝超过 4096 字节的文字，一千个汉字（扩展区的字每个 4 字节）也在这之内。 */
    public static final int MAX_LENGTH = 1000;
    public static final String WUBI_86 = "wubi86";
    public static final String WUBI_98 = "wubi98";
    public static final String DESCRIPTION = "输入或粘贴汉字，列出每个字的 86 和 98 五笔编码，简码在前、全码在后。打字时也能查：五笔拼音混输关闭时先按 z 再打拼音，候选后面标着它的五笔编码。";
    public static final String FAILED = "查码失败，请稍后重试。";
    public static final String NOTHING = "没有可以查的汉字。";

    /** 一段文字（一个字或整段词）在两张码表里的编码，各自短码在前。 */
    public record Codes(String text, List<String> wubi86, List<String> wubi98) {}

    private WubiCodeLookupPolicy() {}

    /** 输入框里有不是空白的字才查。 */
    public static boolean searchable(String text) {
        return text != null && !text.trim().isEmpty();
    }

    /** 结果表的行：整段词在前，再按第一次出现的顺序逐字列出。 */
    public static List<Codes> rows(Codes phrase, List<Codes> characters) {
        List<Codes> rows = new ArrayList<>(characters.size() + 1);
        if (phrase != null) rows.add(phrase);
        rows.addAll(characters);
        return rows;
    }

    /** 列标题：当前所选的码表后面标「（当前）」。不认识的版本按 86 处理，与键盘相同。 */
    public static String columnTitle(String column, String profile) {
        String title = WUBI_98.equals(column) ? "98 五笔" : "86 五笔";
        return column.equals(normalizedProfile(profile)) ? title + "（当前）" : title;
    }

    /** 一列的编码，空格分开；码表里没有时是「—」。 */
    public static String codes(List<String> codes) {
        return codes.isEmpty() ? "—" : String.join(" ", codes);
    }

    /** 一行的两列，每列一行：「86 五笔（当前）  wq wqiy」。 */
    public static String columns(Codes codes, String profile) {
        return columnTitle(WUBI_86, profile) + "  " + codes(codes.wubi86()) + "\n"
            + columnTitle(WUBI_98, profile) + "  " + codes(codes.wubi98());
    }

    /** 读屏念的一行：文字和两列编码，编码逐个字母念不必另外处理。 */
    public static String description(Codes codes, String profile) {
        return codes.text() + "，" + columnTitle(WUBI_86, profile) + " " + codes(codes.wubi86()) + "，"
            + columnTitle(WUBI_98, profile) + " " + codes(codes.wubi98());
    }

    /** 不同的字太多、后面没有列出时的提示。 */
    public static String truncated(int listed) {
        return "字太多，只列出了前 " + listed + " 个不同的字。";
    }

    private static String normalizedProfile(String profile) {
        return WUBI_98.equals(profile) ? WUBI_98 : WUBI_86;
    }
}
