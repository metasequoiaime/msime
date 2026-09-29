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

    private QuickPunctuationPolicy() {}

    /** Returns display faces and the ASCII input each entry sends to Engine. */
    public static List<Entry> entries(boolean dedicatedEnglish, int scheme, String localMode) {
        if (dedicatedEnglish || !"none".equals(localMode)) return ASCII;
        return scheme == 3 ? JAPANESE : CHINESE;
    }
}
