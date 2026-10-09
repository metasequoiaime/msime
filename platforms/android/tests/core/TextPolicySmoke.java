package app.msime.android;

import java.nio.charset.StandardCharsets;

/** 共享文本策略保留 Unicode 内容并按码点处理显示文本。 */
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
        check(TextPolicy.lowercaseTrimmed("  EN-US\t").equals("en-us"),
            "lowercase-trimmed text must use the root locale after ASCII trimming");
        check(TextPolicy.lowercaseTrimmed(null).isEmpty(),
            "lowercase-trimmed text must treat a missing value as empty");
        check(TextPolicy.initial("词库", "?").equals("词"),
            "initial must return the first basic-plane code point");
        check(TextPolicy.initial("\ud840\udc00字", "?").equals("\ud840\udc00"),
            "initial must preserve a supplementary-plane code point");
        check(TextPolicy.initial("", "?").equals("?")
                && TextPolicy.initial(null, "?").equals("?"),
            "initial must use the fallback for missing or empty text");
        System.out.println("Android shared text policy passed");
    }
}
