package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.util.Locale;

/** Shared character-level checks for text accepted by Android host policies. */
public final class TextPolicy {
    private TextPolicy() {}

    public static boolean blank(String value) {
        if (value == null || value.isEmpty()) return true;
        return value.codePoints().allMatch(codePoint -> Character.isWhitespace(codePoint)
            || Character.isSpaceChar(codePoint));
    }

    public static boolean hasControl(String value) {
        if (value == null) return false;
        return value.codePoints().anyMatch(Character::isISOControl);
    }

    public static boolean hasControlExceptWhitespace(String value) {
        return value.codePoints().anyMatch(codePoint -> Character.isISOControl(codePoint)
            && codePoint != '\n' && codePoint != '\r' && codePoint != '\t');
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

    /** Encode UTF-8 request text, treating a missing value as empty text. */
    public static byte[] utf8Bytes(String value) {
        return (value == null ? "" : value).getBytes(StandardCharsets.UTF_8);
    }

    /** Return lowercase text using the stable root locale, treating null as empty. */
    public static String lowercase(String value) {
        return (value == null ? "" : value).toLowerCase(Locale.ROOT);
    }

    /** Return uppercase text using the stable root locale, treating null as empty. */
    public static String uppercase(String value) {
        return (value == null ? "" : value).toUpperCase(Locale.ROOT);
    }

    /** Return text with ASCII whitespace trimmed, treating null as empty. */
    public static String trimmed(String value) {
        return value == null ? "" : value.trim();
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
        return value.length() <= maxChars ? value : value.substring(0, maxChars) + "\n…";
    }
}
