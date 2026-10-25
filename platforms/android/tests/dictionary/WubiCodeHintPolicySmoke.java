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
        // 五笔 14 键（#9691）：预编辑只是首选的编码，其余候选按组码逐位对照出自己已打的部分。
        check(WubiCodeHintPolicy.typed("qqkk", "wq", "fourteen_key", "qq").equals("qq")
            && WubiCodeHintPolicy.hint("qqkk", WubiCodeHintPolicy.typed("qqkk", "wq", "fourteen_key", "qq"),
                true, 2, "none", false).equals("kk"), "a fourteen-key candidate hints its own remaining code");
        check(WubiCodeHintPolicy.typed("wqiy", "wq", "fourteen_key", "qq").equals("wq"),
            "the top candidate's typed part is its code prefix");
        check(WubiCodeHintPolicy.typed("vbg", "wq", "fourteen_key", "qq").equals("wq")
            && WubiCodeHintPolicy.hint("vbg", "wq", true, 2, "none", false).isEmpty(),
            "a code outside the typed groups falls back to the preedit and gets no hint");
        check(WubiCodeHintPolicy.typed("q", "wq", "fourteen_key", "qq").equals("wq"),
            "a code shorter than the typed groups falls back");
        check(WubiCodeHintPolicy.typed("xa", "x", "fourteen_key", "x").equals("x")
            && WubiCodeHintPolicy.typed("za", "xb", "fourteen_key", "x").equals("xb"),
            "the ZX group stands for x only");
        check(WubiCodeHintPolicy.typed("qqkk", "wq", "none", "qq").equals("wq")
            && WubiCodeHintPolicy.typed("qqkk", "wq", "fourteen_key", "").equals("wq"),
            "26 keys and an empty composition keep the preedit");
        System.out.println("Android Wubi code hint: prefix, preference, mode guards and fourteen-key groups passed");
    }
}
