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
     * Chinese punctuation is not used by English, Japanese, Korean, Vietnamese, or local utility modes. Korean and Vietnamese always write half-width ASCII punctuation, whatever the Chinese punctuation switch says.
     *
     * <p>`chinesePunctuation` is the user's own switch, which the runtime holds and the toolbar
     * card and the chord move. It gates the faces as well as the output: a key that shows 。 and
     * commits . is worse than either choice on its own.
     */
    public static boolean shouldUseChineseFaces(boolean dedicatedEnglish, int scheme,
                                                String localMode, boolean chinesePunctuation) {
        return chinesePunctuation && !dedicatedEnglish && scheme != 3
            && scheme != KoreanInputPolicy.KOREAN_SCHEME
            && scheme != InputSchemeTraits.VIETNAMESE && "none".equals(localMode);
    }

    /** Returns the key face for the active language; unmapped symbols retain their own face. */
    public static String face(String ascii, boolean chineseMode) {
        if (ascii == null || !chineseMode) return ascii == null ? "" : ascii;
        return CHINESE_FACES.getOrDefault(ascii, ascii);
    }
}
