package app.msime.android;

/** Apple-parity capitalization rules without Android or editor dependencies. */
public final class EnglishCapitalizationPolicy {
    public enum Mode { NONE, WORDS, SENTENCES, ALL_CHARACTERS }

    private EnglishCapitalizationPolicy() {}

    public static boolean shouldShift(Mode mode, CharSequence contextBeforeInput) {
        return switch (mode) {
            case NONE -> false;
            case ALL_CHARACTERS -> true;
            case WORDS -> shouldShiftWords(contextBeforeInput);
            case SENTENCES -> shouldShiftSentences(contextBeforeInput);
        };
    }

    private static boolean shouldShiftWords(CharSequence context) {
        if (context == null) return false;
        if (context.length() == 0) return true;
        int codePoint = Character.codePointBefore(context, context.length());
        if (codePoint == '\'' || codePoint == '\u2019') return false;
        return !Character.isLetterOrDigit(codePoint);
    }

    private static boolean shouldShiftSentences(CharSequence context) {
        if (context == null) return false;
        if (context.length() == 0) return true;
        for (int offset = context.length(); offset > 0;) {
            int codePoint = Character.codePointBefore(context, offset);
            offset -= Character.charCount(codePoint);
            if (codePoint == '\n' || codePoint == '\r') return true;
            if (Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint)
                    || isClosing(codePoint)) continue;
            return isSentenceTerminator(codePoint);
        }
        return true;
    }

    private static boolean isClosing(int codePoint) {
        return codePoint == '\'' || codePoint == '"' || codePoint == '\u2019'
            || codePoint == '\u201d' || codePoint == ')' || codePoint == ']'
            || codePoint == '}';
    }

    private static boolean isSentenceTerminator(int codePoint) {
        return codePoint == '.' || codePoint == '!' || codePoint == '?'
            || codePoint == '\u3002' || codePoint == '\uff01' || codePoint == '\uff1f';
    }
}
