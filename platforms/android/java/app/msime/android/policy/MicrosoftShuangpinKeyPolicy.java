package app.msime.android;

/** Visibility contract for the Microsoft double-pinyin ing key. */
public final class MicrosoftShuangpinKeyPolicy {
    private MicrosoftShuangpinKeyPolicy() { }

    public static boolean visible(boolean dedicatedEnglish, KeyboardScheme scheme,
            String localMode) {
        return !dedicatedEnglish && scheme == KeyboardScheme.MICROSOFT
            && "none".equals(localMode);
    }
}
