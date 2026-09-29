package app.msime.android;

/** Availability contract for the Japanese post-kana variant key. */
public final class JapaneseVariantPolicy {
    private JapaneseVariantPolicy() { }

    public static boolean enabled(boolean japaneseNineKey, boolean symbols, boolean composing) {
        return japaneseNineKey && !symbols && composing;
    }

    public static String accessibilityLabel(boolean enabled) {
        return enabled
            ? "小假名、浊音、半浊音"
            : "小假名、浊音、半浊音；请先输入假名";
    }
}
