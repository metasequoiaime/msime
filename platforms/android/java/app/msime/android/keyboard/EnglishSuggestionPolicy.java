package app.msime.android.keyboard;

/** Pure text boundaries for direct English completion in the Android host. */
public final class EnglishSuggestionPolicy {
    private EnglishSuggestionPolicy() {}

    /** Read Latin letters backwards, normalizing full-width forms for the ASCII dictionary. */
    public static String currentWord(CharSequence beforeCursor) {
        if (beforeCursor == null || beforeCursor.length() == 0) return "";
        StringBuilder result = new StringBuilder(beforeCursor.length());
        for (int offset = beforeCursor.length(); offset > 0;) {
            int codePoint = Character.codePointBefore(beforeCursor, offset);
            int normalized = normalizeLetter(codePoint);
            if (normalized < 0) break;
            result.appendCodePoint(normalized);
            offset -= Character.charCount(codePoint);
        }
        return result.reverse().toString();
    }

    private static int normalizeLetter(int codePoint) {
        if (isAsciiLetter(codePoint)) return codePoint;
        if ((codePoint >= 0xff21 && codePoint <= 0xff3a)
                || (codePoint >= 0xff41 && codePoint <= 0xff5a)) {
            return codePoint - 0xfee0;
        }
        return -1;
    }

    private static boolean isAsciiLetter(int codePoint) {
        return (codePoint >= 'A' && codePoint <= 'Z')
            || (codePoint >= 'a' && codePoint <= 'z');
    }

    public static Replacement replacement(String typed, String candidate, boolean startedCapitalized) {
        if (typed == null || candidate == null || candidate.isEmpty()) return null;
        String word = candidate;
        if (startedCapitalized) {
            int first = word.codePointAt(0);
            word = new StringBuilder(word.length()).appendCodePoint(Character.toUpperCase(first))
                .append(word.substring(Character.charCount(first))).toString();
        }
        return word.equals(typed) ? null : new Replacement(typed.length(), word);
    }

    public record Replacement(int deleteCount, String insert) {}
}
