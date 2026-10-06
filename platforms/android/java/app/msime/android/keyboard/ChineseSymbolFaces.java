package app.msime.android;

import java.util.Map;

/** Displays the punctuation a standard symbol key inserts in Chinese mode. */
public final class ChineseSymbolFaces {
    private static final Map<String, String> CHINESE_FACES = Map.ofEntries(
        Map.entry(",", "，"), Map.entry(".", "。"), Map.entry("?", "？"),
        Map.entry("!", "！"), Map.entry(";", "；"), Map.entry(":", "："),
        Map.entry("(", "（"), Map.entry(")", "）"), Map.entry("[", "【"),
        Map.entry("]", "】"), Map.entry("\\", "、"), Map.entry("<", "《"),
        Map.entry(">", "》"), Map.entry("'", "‘"), Map.entry("\"", "“"),
        Map.entry("_", "——"));

    private ChineseSymbolFaces() {}

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
        if (ascii == null || !chineseMode) return ascii == null ? "" : ascii;
        return CHINESE_FACES.getOrDefault(ascii, ascii);
    }
}
