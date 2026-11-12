package app.msime.android;

/** Visibility contract for the double-pinyin `;` final key: Microsoft, Sogou and Ziguang put ing there, Cantonese Shuangpin puts ing and ik there. */
public final class MicrosoftShuangpinKeyPolicy {
    private MicrosoftShuangpinKeyPolicy() { }

    public static boolean visible(boolean dedicatedEnglish, KeyboardScheme scheme,
            String localMode) {
        return !dedicatedEnglish && showsSemicolonFinal(scheme) && "none".equals(localMode);
    }

    /** 第二排 L 右边带 `;` 韵母键的方案：微软、搜狗和紫光双拼（ing）以及粤语双拼（ing、ik）。其他方案的 `;` 不是编码键，不显示。 */
    public static boolean showsSemicolonFinal(KeyboardScheme scheme) {
        return scheme == KeyboardScheme.MICROSOFT || scheme == KeyboardScheme.SOGOU
            || scheme == KeyboardScheme.ZIGUANG || scheme == KeyboardScheme.CANTONESE_SHUANGPIN;
    }

    /** `;` 键的读屏标签，按方案说明它打哪个韵母。 */
    public static String accessibilityLabel(KeyboardScheme scheme) {
        if (scheme == KeyboardScheme.CANTONESE_SHUANGPIN) return "分号，粤语双拼 ing 和 ik";
        if (scheme == KeyboardScheme.SOGOU) return "分号，搜狗双拼 ing";
        if (scheme == KeyboardScheme.ZIGUANG) return "分号，紫光双拼 ing";
        return "分号，微软双拼 ing";
    }

    /** 这一下 `;` 是不是韵母：微软双拼或粤语双拼、正在组字、不在本地模式时是，交给引擎当双拼键；否则照常是标点。 */
    public static boolean routesAsFinal(char key, boolean composing, boolean dedicatedEnglish,
            KeyboardScheme scheme, String localMode) {
        return key == ';' && composing && visible(dedicatedEnglish, scheme, localMode);
    }
}
