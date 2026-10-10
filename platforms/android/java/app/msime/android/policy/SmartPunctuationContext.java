package app.msime.android;

/** Minimal, non-persisted adapter from Android editor context to the shared punctuation policy. */
public final class SmartPunctuationContext {
    private SmartPunctuationContext() {}

    public static boolean isAsciiPunctuation(char value) {
        return value >= '!' && value <= '~' && !TextPolicy.isLetterOrDigit(value);
    }

    /** Returns zero when the editor exposes no preceding character. */
    public static int precedingCodePoint(CharSequence contextBeforeCursor) {
        if (contextBeforeCursor == null || contextBeforeCursor.length() == 0) return 0;
        int codePoint = Character.codePointBefore(contextBeforeCursor, contextBeforeCursor.length());
        return codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE
            ? 0 : codePoint;
    }
}
