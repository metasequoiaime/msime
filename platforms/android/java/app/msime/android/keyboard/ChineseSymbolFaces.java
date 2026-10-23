package app.msime.android;

import java.util.Map;

/** Displays the punctuation a standard symbol key inserts in Chinese mode. */
public final class ChineseSymbolFaces {
    // `、` 不在表里：它的顿号键（`\` 和 `/`）由 View 的顿号开关决定，键面走 face() 的开关分支。
    private static final Map<String, String> CHINESE_FACES = Map.ofEntries(
        Map.entry(",", "，"), Map.entry(".", "。"), Map.entry("?", "？"),
        Map.entry("!", "！"), Map.entry(";", "；"), Map.entry(":", "："),
        Map.entry("(", "（"), Map.entry(")", "）"), Map.entry("[", "【"),
        Map.entry("]", "】"), Map.entry("<", "《"),
        Map.entry(">", "》"), Map.entry("'", "‘"), Map.entry("\"", "“"),
        Map.entry("_", "——"));

    private ChineseSymbolFaces() {}

    /**
     * 引号与方头括号样式，取自 View 的 `punctuation_marks`（host-api 已按繁体输出开关解析好 `corner_when_traditional`）。直角引号下 `'` 出「」、`"` 出『』，空心括号下 `[` `]` 出〖〗。`enumerationComma*` 是同一 View 的顿号键开关：`\` 缺省开、`/` 缺省关，顿号键面按它们画。
     */
    public record Marks(boolean cornerQuotes, boolean hollowBrackets,
                        boolean enumerationCommaBackslash, boolean enumerationCommaSlash) {
        /** 缺省：弯引号、实心括号、顿号只有 `\`，即没有这些偏好时的行为。 */
        public static final Marks DEFAULT = new Marks(false, false, true, false);

        /** `quotes` 和 `brackets` 是 View 里的取值（"curly"|"corner"、"solid"|"hollow"），不认识的取值按缺省；顿号键按缺省开关。 */
        public static Marks of(String quotes, String brackets) {
            return of(quotes, brackets, true, false);
        }

        public static Marks of(String quotes, String brackets,
                               boolean enumerationCommaBackslash, boolean enumerationCommaSlash) {
            return new Marks("corner".equals(quotes), "hollow".equals(brackets),
                enumerationCommaBackslash, enumerationCommaSlash);
        }
    }

    /**
     * 英文、日语、韩语、越南语、藏文和本地工具模式都不用中文标点。韩语、越南语和藏文无论中文标点开关怎样都写半角 ASCII 标点。
     *
     * <p>`chinesePunctuation` is the user's own switch, which the runtime holds and the toolbar
     * card and the chord move. It gates the faces as well as the output: a key that shows 。 and
     * commits . is worse than either choice on its own.
     */
    public static boolean shouldUseChineseFaces(boolean dedicatedEnglish, int scheme,
                                                String localMode, boolean chinesePunctuation) {
        return chinesePunctuation && !dedicatedEnglish && scheme != 3
            && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && scheme != InputSchemeTraits.VIETNAMESE && scheme != InputSchemeTraits.TIBETAN
            && "none".equals(localMode);
    }

    /** Returns the key face for the active language; unmapped symbols retain their own face. */
    public static String face(String ascii, boolean chineseMode) {
        return face(ascii, chineseMode, Marks.DEFAULT);
    }

    /** 按引号与方头括号样式取键面：这些键经 Engine 上屏，键面和写出的字一致。 */
    public static String face(String ascii, boolean chineseMode, Marks marks) {
        if (ascii == null || !chineseMode) return ascii == null ? "" : ascii;
        Marks styled = marks == null ? Marks.DEFAULT : marks;
        if (styled.cornerQuotes()) {
            if ("'".equals(ascii)) return "「";
            if ("\"".equals(ascii)) return "『";
        }
        if (styled.hollowBrackets()) {
            if ("[".equals(ascii)) return "〖";
            if ("]".equals(ascii)) return "〗";
        }
        // 顿号键随 View 的顿号开关：关着的键画它自己的 ASCII 面。
        if ("\\".equals(ascii)) return styled.enumerationCommaBackslash() ? "、" : ascii;
        if ("/".equals(ascii)) return styled.enumerationCommaSlash() ? "、" : ascii;
        return CHINESE_FACES.getOrDefault(ascii, ascii);
    }
}
