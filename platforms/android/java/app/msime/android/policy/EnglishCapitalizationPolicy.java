package app.msime.android;

/** 不依赖 Android 编辑器的英文自动大写规则；本机开关关闭时所有编辑器模式都保持小写。 */
public final class EnglishCapitalizationPolicy {
    public enum Mode { NONE, WORDS, SENTENCES, ALL_CHARACTERS }

    private EnglishCapitalizationPolicy() {}

    public static boolean shouldShift(boolean enabled, Mode mode, CharSequence contextBeforeInput) {
        return enabled && shouldShift(mode, contextBeforeInput);
    }

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
        return !TextPolicy.isLetterOrDigit(codePoint);
    }

    private static boolean shouldShiftSentences(CharSequence context) {
        if (context == null) return false;
        if (context.length() == 0) return true;
        // 句末标点后要隔着空白才算新句，与 `TextUtils.getCapsMode` 和 iOS 系统键盘一致；否则刚点下 `.`、删字删到 `abc.` 后面或在 `你好。` 后切到英文都会立刻变成大写，`3.14`、`e.g`、网址也打不出来。
        boolean whitespaceAfter = false;
        for (int offset = context.length(); offset > 0;) {
            int codePoint = Character.codePointBefore(context, offset);
            offset -= Character.charCount(codePoint);
            if (codePoint == '\n' || codePoint == '\r') return true;
            if (TextPolicy.isSpace(codePoint)) {
                whitespaceAfter = true;
                continue;
            }
            if (isClosing(codePoint)) continue;
            return whitespaceAfter && isSentenceTerminator(codePoint);
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
