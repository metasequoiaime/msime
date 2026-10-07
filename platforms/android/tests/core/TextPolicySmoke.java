package app.msime.android;

import java.nio.charset.StandardCharsets;

/** Shared UTF-8 request encoding preserves the exact Unicode payload. */
public final class TextPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] arguments) {
        String value = "合成🙂/request";
        check(java.util.Arrays.equals(TextPolicy.utf8Bytes(value),
                value.getBytes(StandardCharsets.UTF_8)),
            "UTF-8 helper must preserve the request bytes");
        check(TextPolicy.utf8Bytes("").length == 0,
            "UTF-8 helper must encode empty text as an empty payload");
        System.out.println("Android UTF-8 request encoding passed");
    }
}
