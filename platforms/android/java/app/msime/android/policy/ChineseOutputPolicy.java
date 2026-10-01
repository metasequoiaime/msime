package app.msime.android;

/** Host-boundary Simplified/Traditional output policy without Android dependencies. */
public final class ChineseOutputPolicy {
    @FunctionalInterface
    public interface Converter {
        String convert(String text);
    }

    private ChineseOutputPolicy() {}

    /** Only Quanpin, Shuangpin and Wubi commits are converted: Japanese, Korean and Vietnamese text is not Chinese, and Cantonese and Zhuyin already write Traditional characters. A scheme number this host does not know keeps the old answer and is converted. */
    public static boolean applies(boolean dedicatedEnglish, int scheme, String localMode) {
        boolean schemeConverts = !InputSchemeTraits.known(scheme)
            || InputSchemeTraits.scriptConversionApplies(scheme);
        return !dedicatedEnglish && schemeConverts && !"temporary_japanese".equals(localMode);
    }

    public static String output(String text, boolean traditional, boolean applies,
                                Converter converter) {
        if (!traditional || !applies || text.isEmpty()) return text;
        try {
            String converted = converter.convert(text);
            return converted == null ? text : converted;
        } catch (RuntimeException | LinkageError error) {
            return text;
        }
    }
}
