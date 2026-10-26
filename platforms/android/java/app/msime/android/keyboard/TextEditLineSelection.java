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
}
