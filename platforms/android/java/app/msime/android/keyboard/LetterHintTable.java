package app.msime.android;

import java.util.Map;

/** 26 键字母键右上角的「更多符号提示」：中文键使用 q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/，英文键只把 X/C 换成下划线和反引号；中文标点生效时，其中 Engine 会转写的 ASCII 标点（j k l v b）键面画中文形式（{@link #face}），下滑或长按交给 Engine 的标点路由上屏（{@link #routesThroughEngine}），其余键面和输出都是这个字符本身。 */
public final class LetterHintTable {
    private static final Map<String, String> HINTS = Map.ofEntries(
        Map.entry("q", "1"), Map.entry("w", "2"), Map.entry("e", "3"), Map.entry("r", "4"),
        Map.entry("t", "5"), Map.entry("y", "6"), Map.entry("u", "7"), Map.entry("i", "8"),
        Map.entry("o", "9"), Map.entry("p", "0"),
        Map.entry("a", "@"), Map.entry("s", "#"), Map.entry("d", "¥"), Map.entry("f", "%"),
        Map.entry("g", "&"), Map.entry("h", "*"), Map.entry("j", "("), Map.entry("k", ")"),
        Map.entry("l", "\""),
        Map.entry("z", "~"), Map.entry("x", "…"), Map.entry("c", "、"), Map.entry("v", "?"),
        Map.entry("b", "!"), Map.entry("n", "-"), Map.entry("m", "/"));

    private LetterHintTable() { }

    /**
     * 一个非英文模式字母键的提示字符。
     *
     * @param letter 小写或大写的单个 ASCII 字母
     * @return 提示字符；不是 26 个字母之一时返回 null
     */
    public static String hint(String letter) {
        if (letter == null || letter.length() != 1) return null;
        char c = TextPolicy.lowercase(letter).charAt(0);
        return HINTS.get(String.valueOf(c));
    }

    /** 英文模式只替换 X/C；语言模式与中文标点开关分别处理。 */
    public static String hint(String letter, boolean english) {
        String hint = hint(letter);
        if (!english || hint == null) return hint;
        String normalized = TextPolicy.lowercase(letter);
        if ("x".equals(normalized)) return "_";
        if ("c".equals(normalized)) return "`";
        return hint;
    }

    /**
     * 字母键右上角画的键面，也是下滑或长按时气泡显示的字符。
     *
     * @param letter 小写或大写的单个 ASCII 字母
     * @param chinesePunctuation 中文标点是否生效（{@code MSIMEInputService.sendsChinesePunctuation()}）
     * @return 中文标点生效时，( ) " ? ! 换成 （ ） “ ？ ！（与 123 层同一份 {@link ChineseSymbolFaces}），其余提示照旧；没有提示时返回 null
     */
    public static String face(String letter, boolean chinesePunctuation) {
        return face(letter, chinesePunctuation, ChineseSymbolFaces.Marks.DEFAULT);
    }

    /** 同 {@link #face(String, boolean)}，按引号样式画 " 的提示：直角引号下是『。 */
    public static String face(String letter, boolean chinesePunctuation, ChineseSymbolFaces.Marks marks) {
        return face(letter, false, chinesePunctuation, marks);
    }

    /** 按语言模式取角标，再独立应用中文标点键面；英文模式不转写中文标点。 */
    public static String face(String letter, boolean english, boolean chinesePunctuation,
            ChineseSymbolFaces.Marks marks) {
        String hint = hint(letter, english);
        if (hint == null || english || !chinesePunctuation) return hint;
        return ChineseSymbolFaces.face(hint, true, marks);
    }

    /**
     * 中文标点生效时，这个提示字符是否交给 Engine 的标点路由上屏，而不是按字面上屏。只有 Engine 会转写成中文标点的单个 ASCII 字符（( ) " ? !）为真；数字、Engine 不收的 @ # % & * ~ - /，以及本来就不是 ASCII 的 ¥、…、顿号都为假，仍按字面上屏。
     *
     * @param hint {@link #hint} 的结果
     */
    public static boolean routesThroughEngine(String hint) {
        if (hint == null || hint.length() != 1 || hint.charAt(0) >= 128) return false;
        return !ChineseSymbolFaces.face(hint, true).equals(hint);
    }

    /** 某个字母键是否带提示。 */
    public static boolean hasHint(String letter) {
        return hint(letter) != null;
    }
}
