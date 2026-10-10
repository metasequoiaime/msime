package app.msime.android;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

/** Shared character-level checks for text accepted by Android host policies. */
public final class TextPolicy {
    private TextPolicy() {}

    /** Return whether a code point is one of the 52 ASCII Latin letters. */
    public static boolean isAsciiLetter(int codePoint) {
        return (codePoint >= 'A' && codePoint <= 'Z')
            || (codePoint >= 'a' && codePoint <= 'z');
    }

    /** Return whether a code point is an ASCII Latin letter or decimal digit. */
    public static boolean isAsciiLetterOrDigit(int codePoint) {
        return isAsciiLetter(codePoint) || (codePoint >= '0' && codePoint <= '9');
    }

    /** Return whether text is exactly a fixed number of lower-case hexadecimal digits. */
    public static boolean isLowerHex(String value, int length) {
        return value != null && value.length() == length
            && value.chars().allMatch(codePoint -> codePoint >= '0' && codePoint <= '9'
                || codePoint >= 'a' && codePoint <= 'f');
    }

    /** Return whether a code point is Unicode whitespace or a Unicode space character. */
    public static boolean isSpace(int codePoint) {
        return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
    }

    public static boolean blank(String value) {
        if (value == null || value.isEmpty()) return true;
        return value.codePoints().allMatch(TextPolicy::isSpace);
    }

    public static boolean hasControl(String value) {
        if (value == null) return false;
        return value.codePoints().anyMatch(TextPolicy::isControl);
    }

    /** Return whether a code point is an ISO control character. */
    public static boolean isControl(int codePoint) {
        return Character.isISOControl(codePoint);
    }

    public static boolean hasControlExceptWhitespace(String value) {
        return value.codePoints().anyMatch(codePoint -> Character.isISOControl(codePoint)
            && codePoint != '\n' && codePoint != '\r' && codePoint != '\t');
    }

    /** Return whether text contains an ISO control character other than line feed. */
    public static boolean hasControlExceptNewline(String value) {
        return value.codePoints().anyMatch(codePoint -> Character.isISOControl(codePoint)
            && codePoint != '\n');
    }

    /** Replace ISO control characters while preserving all other UTF-16 units. */
    public static String replaceControls(String value, char replacement) {
        if (value == null || value.isEmpty()) return value == null ? "" : value;
        StringBuilder result = new StringBuilder(value.length());
        for (int index = 0; index < value.length(); index++) {
            char unit = value.charAt(index);
            result.append(Character.isISOControl(unit) ? replacement : unit);
        }
        return result.toString();
    }

    /** Remove ISO control code points while preserving all other Unicode text. */
    public static String removeControls(String value) {
        if (value == null || value.isEmpty()) return value == null ? "" : value;
        StringBuilder result = new StringBuilder(value.length());
        value.codePoints().filter(codePoint -> !Character.isISOControl(codePoint))
            .forEach(result::appendCodePoint);
        return result.toString();
    }

    /** Accepts a bounded URL with the requested scheme and a non-empty authority. */
    public static boolean validAuthority(String value, String scheme, int maxBytes) {
        if (value == null || value.isEmpty() || !value.startsWith(scheme)
                || utf8Length(value) > maxBytes || hasControl(value) || !validUnicode(value)) return false;
        String rest = value.substring(scheme.length());
        int end = rest.length();
        for (char separator : new char[] {'/', '?', '#'}) {
            int position = rest.indexOf(separator);
            if (position >= 0 && position < end) end = position;
        }
        return end > 0 && !value.contains("@") && !value.contains("#");
    }

    public static boolean validUnicode(String value) {
        for (int index = 0; index < value.length(); index++) {
            char unit = value.charAt(index);
            if (Character.isHighSurrogate(unit)) {
                if (++index >= value.length() || !Character.isLowSurrogate(value.charAt(index))) return false;
            } else if (Character.isLowSurrogate(unit)) return false;
        }
        return true;
    }

    public static int utf8Length(String value) {
        return value.getBytes(StandardCharsets.UTF_8).length;
    }

    /** Decode UTF-8 response bytes with the shared text policy. */
    public static String utf8(byte[] value) {
        return value == null ? "" : new String(value, StandardCharsets.UTF_8);
    }

    /** Decode UTF-8 bytes strictly, reporting malformed or unmappable input to the caller. */
    public static String utf8Strict(byte[] value) throws CharacterCodingException {
        return StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(value)).toString();
    }

    /** Encode text as UTF-8 strictly, reporting malformed or unmappable input to the caller. */
    public static byte[] utf8StrictBytes(String value) throws CharacterCodingException {
        ByteBuffer encoded = StandardCharsets.UTF_8.newEncoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
            .encode(CharBuffer.wrap(value));
        byte[] result = new byte[encoded.remaining()];
        encoded.get(result);
        return result;
    }

    /** Encode UTF-8 request text, treating a missing value as empty text. */
    public static byte[] utf8Bytes(String value) {
        return (value == null ? "" : value).getBytes(StandardCharsets.UTF_8);
    }

    /** Return lowercase text using the stable root locale, treating null as empty. */
    public static String lowercase(String value) {
        return (value == null ? "" : value).toLowerCase(Locale.ROOT);
    }

    /** 去掉首尾 ASCII 空白后按稳定的根区域规则转成小写，{@code null} 按空文本处理。 */
    public static String lowercaseTrimmed(String value) {
        return lowercase(trimmed(value));
    }

    /** Return uppercase text using the stable root locale, treating null as empty. */
    public static String uppercase(String value) {
        return (value == null ? "" : value).toUpperCase(Locale.ROOT);
    }

    /** Return text with ASCII whitespace trimmed, treating null as empty. */
    public static String trimmed(String value) {
        return value == null ? "" : value.trim();
    }

    /** Return whether text contains at least one non-whitespace character. */
    public static boolean hasText(String value) {
        return !blank(value);
    }

    /** Return whether text is non-blank, valid Unicode, control-free and within a UTF-8 byte bound. */
    public static boolean boundedNonBlank(String value, int maxBytes) {
        return hasText(value) && utf8Length(value) <= maxBytes
            && !hasControl(value) && validUnicode(value);
    }

    /** Return text with Unicode whitespace stripped, treating null as empty. */
    public static String stripped(String value) {
        return value == null ? "" : value.strip();
    }

    /** Strip Unicode whitespace and space characters at both ends, preserving null. */
    public static String stripSpaceChars(String value) {
        if (value == null || value.isEmpty()) return value;
        int start = 0;
        while (start < value.length()) {
            int codePoint = value.codePointAt(start);
            if (!isSpace(codePoint)) break;
            start += Character.charCount(codePoint);
        }
        int end = value.length();
        while (end > start) {
            int codePoint = value.codePointBefore(end);
            if (!isSpace(codePoint)) break;
            end -= Character.charCount(codePoint);
        }
        return value.substring(start, end);
    }

    /** Return text unchanged, treating a missing value as empty text. */
    public static String emptyIfNull(String value) {
        return value == null ? "" : value;
    }

    /** 返回文本的第一个 Unicode 码点；文本为空时返回调用方给出的后备值。 */
    public static String initial(CharSequence value, String fallback) {
        if (value == null || value.length() == 0) return fallback;
        return new String(Character.toChars(Character.codePointAt(value, 0)));
    }

    /** Return the number of Unicode code points in text, or zero for null. */
    public static int codePointLength(String value) {
        return value == null ? 0 : value.codePointCount(0, value.length());
    }

    /** Truncates UTF-8 text by bytes without splitting a code point. */
    public static String clipUtf8(String value, int maxBytes) {
        if (value == null) return "";
        if (value.getBytes(StandardCharsets.UTF_8).length <= maxBytes) return value;
        int bytes = 0;
        int index = 0;
        while (index < value.length()) {
            int codePoint = value.codePointAt(index);
            int size = codePoint < 0x80 ? 1 : codePoint < 0x800 ? 2 : codePoint < 0x10000 ? 3 : 4;
            if (bytes + size > maxBytes) break;
            bytes += size;
            index += Character.charCount(codePoint);
        }
        return value.substring(0, index);
    }

    /** Truncate text to at most {@code maxChars} UTF-16 code units. */
    public static String clip(String value, int maxChars) {
        if (value == null || maxChars <= 0) return "";
        return value.length() <= maxChars ? value : value.substring(0, maxChars);
    }

    /** Truncate UTF-16 text without leaving a high surrogate at the end. */
    public static String clipSurrogateSafe(String value, int maxChars) {
        if (value == null || maxChars <= 0) return value;
        if (value.length() <= maxChars) return value;
        int end = maxChars;
        if (Character.isHighSurrogate(value.charAt(end - 1))) end--;
        return value.substring(0, end);
    }

    /** Truncate text to at most {@code maxCodePoints} without splitting a surrogate pair. */
    public static String clipCodePoints(String value, int maxCodePoints) {
        if (value == null || maxCodePoints <= 0) return "";
        if (codePointLength(value) <= maxCodePoints) return value;
        return value.substring(0, value.offsetByCodePoints(0, maxCodePoints));
    }

    /** Return whether non-null text fits within a Unicode code-point limit. */
    public static boolean withinCodePoints(String value, int maxCodePoints) {
        return value != null && maxCodePoints >= 0
            && codePointLength(value) <= maxCodePoints;
    }

    /** Keep at most the final Unicode code points without splitting a surrogate pair. */
    public static String tailCodePoints(String value, int maxCodePoints) {
        if (value == null || maxCodePoints <= 0) return "";
        int count = codePointLength(value);
        return count <= maxCodePoints ? value
            : value.substring(value.offsetByCodePoints(0, count - maxCodePoints));
    }

    /** Truncate text and append an ellipsis only when the character limit is exceeded. */
    public static String clipWithEllipsis(String value, int maxChars) {
        if (value == null || maxChars <= 0) return "";
        if (value.length() <= maxChars) return value;
        int end = maxChars;
        if (Character.isHighSurrogate(value.charAt(end - 1))) end--;
        return value.substring(0, end) + "\n…";
    }
}
