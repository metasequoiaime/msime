package app.msime.android;

import java.nio.charset.StandardCharsets;

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

    /** Accepts a bounded URL with the requested scheme and a non-empty authority. */
    public static boolean validAuthority(String value, String scheme, int maxBytes) {
        if (value == null || value.isEmpty() || !value.startsWith(scheme)
                || utf8Length(value) > maxBytes || hasControl(value)) return false;
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
}
