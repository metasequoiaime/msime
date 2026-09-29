import app.msime.android.WubiCodeHintPolicy;

public final class WubiCodeHintPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(WubiCodeHintPolicy.hint("wqaa", "wq", true, 2, "none", false).equals("aa"),
            "strict Wubi prefix gets its remaining code");
        check(WubiCodeHintPolicy.hint("wq", "wq", true, 2, "none", false).isEmpty(),
            "complete code has no hint");
        check(WubiCodeHintPolicy.hint("wa", "wq", true, 2, "none", false).isEmpty(),
            "non-prefix code has no hint");
        check(WubiCodeHintPolicy.hint("wqaa", "wq", false, 2, "none", false).isEmpty(),
            "disabled preference hides hint");
        check(WubiCodeHintPolicy.hint("wqaa", "wq", true, 1, "none", false).isEmpty(),
            "non-Wubi scheme has no hint");
        check(WubiCodeHintPolicy.hint("wqaa", "wq", true, 2, "emoji", false).isEmpty(),
            "local mode has no hint");
        check(WubiCodeHintPolicy.hint("wqaa", "wq", true, 2, "none", true).isEmpty(),
            "pinyin fallback has no hint");
        check(WubiCodeHintPolicy.hint("wqaa", "", true, 2, "none", false).isEmpty(),
            "empty preedit has no hint");
        check(WubiCodeHintPolicy.hint("wqaa", "wq", true, 2, "none", false).equals("aa"),
            "candidate code is returned without changing it");
        System.out.println("Android Wubi code hint: prefix, preference and mode guards passed");
    }
}
