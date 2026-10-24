package app.msime.android;

/** Host-boundary Simplified/Traditional output policy without Android dependencies. */
public final class ChineseOutputPolicy {
    @FunctionalInterface
    public interface Converter {
        String convert(String text);
    }

    private ChineseOutputPolicy() {}

    /** 来源 18 已由共享运行时逐段转换，日文汉字必须保持原样。 */
    public static String candidateOutput(String text, boolean traditional, boolean applies,
                                         int source, Converter converter) {
        return output(text, traditional, applies && source != 18, converter);
    }

    /** 只转换全拼、双拼和五笔的上屏：日语、韩语、越南语和藏文不是中文，粤拼和注音本来就写繁体字，笔画上屏的就是按笔画查到的那个字。本宿主不认识的方案序号沿用原来的答案，照常转换。 */
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
