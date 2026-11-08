package app.msime.android;

/** 从编辑器完整文本和活动选区计算当前行的范围。 */
public final class TextEditLineSelection {
    public record Range(int start, int end) {}

    private TextEditLineSelection() {}

    public static Range range(CharSequence text, int selectionStart, int selectionEnd) {
        if (text == null || selectionStart < 0 || selectionEnd < 0) return null;
        int length = text.length();
        int caret = Math.max(0, Math.min(Math.max(selectionStart, selectionEnd), length));
        if (caret > 0 && caret < length && text.charAt(caret) == '\n') caret--;
        int start = caret;
        while (start > 0 && text.charAt(start - 1) != '\n') start--;
        int end = caret;
        while (end < length && text.charAt(end) != '\n') end++;
        return new Range(start, end);
    }

    /**
     * 光标所在行的行首：{@code caret} 之前最近一个换行之后，没有换行就是 0。行按换行符分，和 {@link #range} 一样，不管屏幕上的自动折行。
     *
     * <p>光标在空行上时行首、行尾都是它自己；{@link #range} 在空行上选的是前一行，那是选择行原有的规则，这里不跟。
     *
     * @return 行首位置；文本为空引用或位置为负时返回 -1
     */
    public static int lineStart(CharSequence text, int caret) {
        if (text == null || caret < 0) return -1;
        int at = Math.min(caret, text.length());
        while (at > 0 && text.charAt(at - 1) != '\n') at--;
        return at;
    }

    /**
     * 光标所在行的行尾：从 {@code caret} 起第一个换行之前，没有换行就是文本末尾。
     *
     * @return 行尾位置；文本为空引用或位置为负时返回 -1
     */
    public static int lineEnd(CharSequence text, int caret) {
        if (text == null || caret < 0) return -1;
        int length = text.length();
        int at = Math.min(caret, length);
        while (at < length && text.charAt(at) != '\n') at++;
        return at;
    }
}
