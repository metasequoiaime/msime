package app.msime.android;

/** Visibility contract for the Microsoft double-pinyin ing key. */
public final class MicrosoftShuangpinKeyPolicy {
    private MicrosoftShuangpinKeyPolicy() { }

    public static boolean visible(boolean dedicatedEnglish, KeyboardScheme scheme,
            String localMode) {
        return !dedicatedEnglish && scheme == KeyboardScheme.MICROSOFT
            && "none".equals(localMode);
    }

    /** 这一下 `;` 是不是 ing 韵母：微软双拼、正在组字、不在本地模式时是，交给引擎当双拼键；否则照常是标点。 */
    public static boolean routesAsFinal(char key, boolean composing, boolean dedicatedEnglish,
            KeyboardScheme scheme, String localMode) {
        return key == ';' && composing && visible(dedicatedEnglish, scheme, localMode);
    }
}
