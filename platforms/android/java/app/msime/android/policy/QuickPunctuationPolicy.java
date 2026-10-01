package app.msime.android;

import java.util.List;

/** Provides the Apple-compatible quick punctuation menu for the alphabetic keyboard. */
public final class QuickPunctuationPolicy {
    public record Entry(String face, char input) {
        public Entry {
            if (face == null || face.isEmpty() || input < 32 || input > 126)
                throw new IllegalArgumentException("Invalid quick punctuation entry");
        }
    }

    private static final List<Entry> ASCII = List.of(
        new Entry(",", ','), new Entry(".", '.'), new Entry("?", '?'), new Entry("!", '!'),
        new Entry(":", ':'), new Entry(";", ';'), new Entry("@", '@'));
    private static final List<Entry> CHINESE = List.of(
        new Entry("，", ','), new Entry("。", '.'), new Entry("？", '?'), new Entry("！", '!'),
        new Entry("、", '\\'), new Entry("；", ';'), new Entry("：", ':'));
    private static final List<Entry> JAPANESE = List.of(
        new Entry("、", '\\'), new Entry("。", '.'), new Entry("？", '?'), new Entry("！", '!'),
        new Entry("「", '['), new Entry("」", ']'), new Entry("・", '/'));
    // Dachen claims , . ; / - and the digits as bopomofo keys, so each mark is sent as the Shift key the Engine's Zhuyin overlay turns into it: < is ，, > is 。, [ and ] are 「」.
    private static final List<Entry> ZHUYIN = List.of(
        new Entry("，", '<'), new Entry("。", '>'), new Entry("？", '?'), new Entry("！", '!'),
        new Entry("：", ':'), new Entry("「", '['), new Entry("」", ']'));

    private QuickPunctuationPolicy() {}

    /** Returns display faces and the ASCII input each entry sends to Engine. Korean and Vietnamese punctuation is half-width ASCII. */
    public static List<Entry> entries(boolean dedicatedEnglish, int scheme, String localMode) {
        if (dedicatedEnglish || !"none".equals(localMode)
                || scheme == InputSchemeTraits.KOREAN
                || scheme == InputSchemeTraits.VIETNAMESE) return ASCII;
        if (scheme == InputSchemeTraits.JAPANESE) return JAPANESE;
        return scheme == InputSchemeTraits.ZHUYIN ? ZHUYIN : CHINESE;
    }
}
