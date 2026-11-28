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
    private static final List<Entry> URI_ASCII = List.of(
        new Entry(".", '.'), new Entry(",", ','), new Entry("?", '?'), new Entry("!", '!'),
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

    /** 返回每一项的显示字面和它发送给 Engine 的 ASCII 输入。韩语、越南语和藏文的标点是半角 ASCII。 */
    public static List<Entry> entries(boolean dedicatedEnglish, int scheme, String localMode) {
        return entries(dedicatedEnglish, scheme, localMode, false);
    }

    /** 只在英文网址框且单标点底键可见时，把句号放到点按位置。 */
    public static boolean uriPeriodPrimary(boolean dedicatedEnglish, int inputType, boolean commaSecondaryPeriod) {
        return dedicatedEnglish && commaSecondaryPeriod && EditorPolicy.uri(inputType);
    }

    public static List<Entry> entries(boolean dedicatedEnglish, int scheme, String localMode,
                                      boolean uriPeriodPrimary) {
        if (uriPeriodPrimary) return URI_ASCII;
        if (dedicatedEnglish || !"none".equals(localMode)
                || scheme == InputSchemeTraits.KOREAN
                || scheme == InputSchemeTraits.VIETNAMESE
                || scheme == InputSchemeTraits.TIBETAN) return ASCII;
        if (scheme == InputSchemeTraits.JAPANESE) return JAPANESE;
        return scheme == InputSchemeTraits.ZHUYIN ? ZHUYIN : CHINESE;
    }

    /** 同上，再按 View 的 `preserve_english_punctuation`（#11890）换中文表的键面：设成保留英文的键 Engine 原样写出 ASCII，这一项就画这个键，键面与上屏一致（与 {@link ChineseSymbolFaces#face} 同一条规则）。注音经 Shift 键走自己的表、日文表不看这个集合，都不换。 */
    public static List<Entry> entries(boolean dedicatedEnglish, int scheme, String localMode,
                                      boolean uriPeriodPrimary, String preserveEnglish) {
        List<Entry> entries = entries(dedicatedEnglish, scheme, localMode, uriPeriodPrimary);
        if (entries != CHINESE || preserveEnglish == null || preserveEnglish.isEmpty()) return entries;
        return entries.stream()
            .map(entry -> preserveEnglish.indexOf(entry.input()) >= 0
                ? new Entry(String.valueOf(entry.input()), entry.input()) : entry)
            .toList();
    }
}
