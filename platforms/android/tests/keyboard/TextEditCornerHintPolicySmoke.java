import app.msime.android.TextEditCornerHintPolicy;

public final class TextEditCornerHintPolicySmoke {
    public static void main(String[] args) {
        // 按 1 dp = 1 px 估：「全选」16 sp 约 32 宽、墨迹上边约在 18，「选行」10 sp 约 20 宽、墨迹下边约在 13，右边距 5，空隙 2。
        check(TextEditCornerHintPolicy.fits(165, (165 + 32) / 2f, 18, 165 - 5 - 20, 13, 2),
            "a wide landscape key keeps the hint beside the label");
        check(TextEditCornerHintPolicy.fits(74, (74 + 32) / 2f, 18, 74 - 5 - 20, 13, 2),
            "a portrait key keeps it above the label even though they overlap sideways");
        check(!TextEditCornerHintPolicy.fits(74, (74 + 32) / 2f, 14, 74 - 5 - 20, 13, 2),
            "a short key where they overlap both ways drops the hint");
        check(TextEditCornerHintPolicy.fits(74, (74 + 32) / 2f, 15, 74 - 5 - 20, 13, 2),
            "exactly the gap apart vertically still fits");
        check(!TextEditCornerHintPolicy.fits(24, (24 + 20) / 2f, 30, 24 - 5 - 20, 13, 2),
            "a hint that runs past the key's left edge is dropped");
        check(!TextEditCornerHintPolicy.fits(0, 0, 0, 0, 0, 2), "no size yet");
        System.out.println("Android text edit corner hint: fits beside or above the label, dropped otherwise passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
