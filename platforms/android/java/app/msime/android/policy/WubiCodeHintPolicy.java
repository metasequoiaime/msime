package app.msime.android;

/** Pure presentation rules for the optional remaining-code hint on Wubi candidates. */
public final class WubiCodeHintPolicy {
    public static final int WUBI_SCHEME = 2;
    private static final int MAX_CODE_LENGTH = 64;

    private WubiCodeHintPolicy() {}

    /**
     * Returns only the untyped suffix when the candidate code is a strict extension of the
     * current Wubi preedit. Fallback and local candidates are deliberately not annotated.
     */
    public static String hint(String code, String typed, boolean enabled, int scheme,
                              String localMode, boolean answeredByPinyinFallback) {
        if (!enabled || scheme != WUBI_SCHEME || answeredByPinyinFallback
                || !"none".equals(localMode) || code == null || typed == null
                || typed.isEmpty() || code.length() > MAX_CODE_LENGTH
                || typed.length() > MAX_CODE_LENGTH || code.length() <= typed.length()
                || !code.startsWith(typed)) return "";
        return code.substring(typed.length());
    }
}
