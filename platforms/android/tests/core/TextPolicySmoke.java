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
        check(!TextPolicy.hasText(null) && !TextPolicy.hasText("  \n\t")
                && !TextPolicy.hasText("\u2003\u00a0\u3000")
                && TextPolicy.hasText(" synthetic "),
            "hasText must reject blank text and accept non-blank text");
        check(TextPolicy.boundedNonBlank("token", 5),
            "bounded non-blank text is accepted");
        check(!TextPolicy.boundedNonBlank("", 5)
                && !TextPolicy.boundedNonBlank("token", 4)
                && !TextPolicy.boundedNonBlank("bad\u0000", 20)
                && !TextPolicy.boundedNonBlank("bad\uD800", 20),
            "bounded non-blank text enforces all shared boundaries");
        check(TextPolicy.boundedNonEmpty("  ", 2)
                && !TextPolicy.boundedNonBlank("  ", 2),
            "non-empty metadata permits spaces without weakening non-blank fields");
        check(!TextPolicy.boundedNonEmpty(null, 2)
                && !TextPolicy.boundedNonEmpty("", 2)
                && !TextPolicy.boundedNonEmpty("😀", 3)
                && !TextPolicy.boundedNonEmpty("x\u0000", 2)
                && !TextPolicy.boundedNonEmpty("x\uD800", 2),
            "bounded non-empty text enforces byte, control and Unicode boundaries");
        check(TextPolicy.initial("词库", "?").equals("词"),
            "initial must return the first basic-plane code point");
        check(TextPolicy.initial("\ud840\udc00字", "?").equals("\ud840\udc00"),
            "initial must preserve a supplementary-plane code point");
        check(TextPolicy.initial("", "?").equals("?")
                && TextPolicy.initial(null, "?").equals("?"),
            "initial must use the fallback for missing or empty text");
        check(TextPolicy.isAsciiLetter('A') && TextPolicy.isAsciiLetter('z'),
            "ASCII letters must be recognized");
        check(!TextPolicy.isAsciiLetter('0') && !TextPolicy.isAsciiLetter(0xff21),
            "digits and full-width letters are not ASCII letters");
        check(TextPolicy.stripSpaceChars("\u2003\u00a0text\u3000").equals("text"),
            "space-character trimming must cover Unicode space separators");
        check(TextPolicy.stripSpaceChars(null) == null
                && TextPolicy.stripSpaceChars("\u2003").isEmpty(),
            "space-character trimming preserves null and removes all-space text");
        check(TextPolicy.trimUnicodeWhitespace("\u0085\u00a0\u202f\u3000").isEmpty(),
            "与 Rust 一致地把全部 Unicode 空白裁成空字符串");
        check(TextPolicy.trimUnicodeWhitespace("\u0085合成🙂 key\u202f").equals("合成🙂 key"),
            "只去掉首尾空白，保留正文与补充平面字符");
        check(TextPolicy.trimUnicodeWhitespace("\u001c合成\u001f").equals("\u001c合成\u001f")
                && TextPolicy.trimUnicodeWhitespace("\ufeff").equals("\ufeff"),
            "不能像 Java 空白谓词那样去掉 Rust 不认可的控制字符或 BOM");
        String emojiBoundary = "a".repeat(1023) + "\ud83d\ude42";
        String clipped = TextPolicy.clipWithEllipsis(emojiBoundary, 1024);
        check(!Character.isHighSurrogate(clipped.charAt(1023)),
            "display clipping must not leave an isolated high surrogate");
        System.out.println("Android shared text policy passed");
    }
}
