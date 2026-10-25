package app.msime.android;

/** Pure presentation rules for the optional remaining-code hint on Wubi candidates. */
public final class WubiCodeHintPolicy {
    public static final int WUBI_SCHEME = 2;
    private static final int MAX_CODE_LENGTH = 64;

    /** 五笔 14 键每一组的代表字母（引擎 `wubi::grid`）到这一组的字母，按 a–z 排；ZX 只代表 `x`，不是代表字母的位置为 null。与鸿蒙 `WubiCodeHintPolicy.ts` 的表相同。 */
    private static final String[] FOURTEEN_KEY_GROUPS = {
        "as", "bn", "cv", "df", "er", null, "gh", null, null, "jk", null, "l", "m",
        null, "op", null, "qw", null, null, "ty", "ui", null, null, "x", null, null,
    };

    private WubiCodeHintPolicy() {}

    /**
     * 候选编码里已经打过的那一段，交给 {@link #hint} 作 `typed`。26 键就是预编辑；五笔 14 键的 `editing_text` 是组码（`qq`），预编辑只是首选的编码（`wq`），别的编码的候选要按组对照：编码的前几位逐位落在这几组里，这几位就是它已打的部分（`qqkk` 已打 `qq`，提示 `kk`），剩下的才是提示。不是 14 键或对不上组时返回预编辑，交给 {@link #hint} 判为不提示。
     */
    public static String typed(String code, String preedit, String keyGrid, String editingText) {
        if (!"fourteen_key".equals(keyGrid) || code == null || editingText == null || editingText.isEmpty())
            return preedit;
        if (code.length() < editingText.length() || editingText.length() > MAX_CODE_LENGTH) return preedit;
        for (int index = 0; index < editingText.length(); index++) {
            char group = editingText.charAt(index);
            String letters = group >= 'a' && group <= 'z' ? FOURTEEN_KEY_GROUPS[group - 'a'] : null;
            if (letters == null || letters.indexOf(code.charAt(index)) < 0) return preedit;
        }
        return code.substring(0, editingText.length());
    }

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
