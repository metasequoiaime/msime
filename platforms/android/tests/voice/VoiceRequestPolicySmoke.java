package app.msime.android;

/** The Android plugin must apply the same request-id and language bounds as the shared host API. */
public final class VoiceRequestPolicySmoke {
    public static void main(String[] args) {
        check(VoiceRequestPolicy.valid("fixture-request-1", "zh-CN"),
            "ordinary voice requests pass");
        check(!VoiceRequestPolicy.valid("请求-1", "zh-CN"),
            "request ids stay ASCII even when Java considers the characters letters");
        check(!VoiceRequestPolicy.valid("fixture-request-1", "😀".repeat(17)),
            "language is bounded by UTF-8 bytes rather than UTF-16 units");
        check(VoiceRequestPolicy.valid("x".repeat(64), "l".repeat(64)),
            "the shared 64-byte ceilings remain inclusive");
        check(!VoiceRequestPolicy.valid("x".repeat(65), "zh-CN"),
            "request ids over 64 bytes are refused");
        System.out.println("Android voice request policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
