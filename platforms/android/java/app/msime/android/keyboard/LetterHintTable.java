package app.msime.android;

import java.util.Map;

/** 26 键字母键右上角的「更多符号提示」：设计稿的 q1…p0 / a@ s# d¥ f% g& h* j( k) l" / z~ x… c、 v? b! n- m/。下滑或长按预览输入的就是这个字符。 */
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
     * 一个字母键的提示字符。
     *
     * @param letter 小写或大写的单个 ASCII 字母
     * @return 提示字符；不是 26 个字母之一时返回 null
     */
    public static String hint(String letter) {
        if (letter == null || letter.length() != 1) return null;
        char c = Character.toLowerCase(letter.charAt(0));
        return HINTS.get(String.valueOf(c));
    }

    /** 某个字母键是否带提示。 */
    public static boolean hasHint(String letter) {
        return hint(letter) != null;
    }
}
