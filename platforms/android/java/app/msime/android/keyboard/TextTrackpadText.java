package app.msime.android;

import java.text.BreakIterator;
import java.util.Locale;

/** 编辑器文字的有界显示与选区计算；片段端点是否为文档端点由宿主显式提供。 */
public final class TextTrackpadText {
    public record Range(int start, int end) { }
    public record Preview(String text, int selectionStart, int selectionEnd, boolean masked) {
        public Preview direction(boolean reverse) {
            return reverse ? new Preview(text, selectionEnd, selectionStart, masked) : this;
        }
    }
    public static final int CONTEXT_CHARS = 96;
    public static final int SELECTION_CHARS = 4096;

    private TextTrackpadText() { }

    /** 编辑器回读入口；长度和密码限制必须在跨进程读取之前判断。 */
    public static Preview readPreview(java.util.function.IntFunction<CharSequence> before,
            java.util.function.IntFunction<CharSequence> after, int selectionLength, boolean password) {
        if (password) return preview(null, null, 0, true);
        if (selectionLength < 0) return null;
        try {
            return preview(before.apply(CONTEXT_CHARS), after.apply(CONTEXT_CHARS), selectionLength, false);
        } catch (RuntimeException ignored) { return null; }
    }

    /** 回读比片段剩余文字多一个字符，核对内容和长度后才能认定文档尾部。 */
    public static boolean endsDocument(CharSequence extracted, int selectionEnd, CharSequence after) {
        if (extracted == null || after == null || selectionEnd < 0 || selectionEnd > extracted.length()
                || extracted.length() - selectionEnd > SELECTION_CHARS
                || after.length() != extracted.length() - selectionEnd) return false;
        for (int at = 0; at < after.length(); at++)
            if (after.charAt(at) != extracted.charAt(selectionEnd + at)) return false;
        return true;
    }

    public static Range word(String text, int offset, int caret, boolean beginsDocument,
            boolean endsDocument, Locale locale) {
        if (!valid(text, offset, caret) || text.isEmpty() || locale == null) return null;
        int focus = Math.min(caret - offset, text.length() - 1);
        if (Character.isLowSurrogate(text.charAt(focus)) && focus > 0) focus--;
        if (Character.isWhitespace(text.charAt(focus)) && focus > 0) focus--;
        BreakIterator words = BreakIterator.getWordInstance(locale);
        words.setText(text);
        int start = words.preceding(focus + 1);
        int end = words.following(focus);
        if (start < 0 || end < 0 || start >= end
                || start == 0 && !beginsDocument || end == text.length() && !endsDocument) return null;
        boolean content = text.substring(start, end).codePoints().anyMatch(cp -> !Character.isWhitespace(cp));
        return content ? new Range(offset + start, offset + end) : null;
    }

    public static Range line(String text, int offset, int caret, boolean beginsDocument, boolean endsDocument) {
        if (!valid(text, offset, caret)) return null;
        int focus = caret - offset;
        if (focus < text.length() && text.charAt(focus) == '\n' && focus > 0 && text.charAt(focus - 1) == '\r')
            focus--;
        int start = focus;
        int end = focus;
        while (start > 0 && !newline(text.charAt(start - 1))) start--;
        while (end < text.length() && !newline(text.charAt(end))) end++;
        if (start == 0 && !beginsDocument || end == text.length() && !endsDocument) return null;
        return new Range(offset + start, offset + end);
    }

    /** 不接受已选文字，长选区始终只放一个省略号；密码分支先返回，连上下文长度也不访问。 */
    public static Preview preview(CharSequence before, CharSequence after, int selectionLength, boolean password) {
        if (password) return new Preview("••••••••", 4, 4, true);
        if (before == null || after == null || selectionLength < 0) return null;
        String prefix = bounded(before, true);
        String suffix = bounded(after, false);
        String middle = selectionLength == 0 ? "" : "…";
        return new Preview(prefix + middle + suffix, prefix.length(), prefix.length() + middle.length(), false);
    }

    private static boolean valid(String text, int offset, int caret) {
        return text != null && text.length() <= SELECTION_CHARS && offset >= 0
            && (long) offset + text.length() <= Integer.MAX_VALUE
            && caret >= offset && (long) caret <= (long) offset + text.length();
    }

    private static boolean newline(char value) { return value == '\n' || value == '\r'; }

    private static String bounded(CharSequence text, boolean tail) {
        int length = text.length();
        int start = tail ? Math.max(0, length - CONTEXT_CHARS) : 0;
        int end = tail ? length : Math.min(length, CONTEXT_CHARS);
        if (start < end && Character.isLowSurrogate(text.charAt(start))) start++;
        if (start < end && Character.isHighSurrogate(text.charAt(end - 1))) end--;
        StringBuilder clean = new StringBuilder(end - start);
        for (int at = start; at < end; at++) {
            char c = text.charAt(at);
            if (c == '\n' || c == '\r') clean.append('↵');
            else if (c == '\t') clean.append('⇥');
            else if (Character.isISOControl(c) || Character.getType(c) == Character.FORMAT) clean.append('�');
            else clean.append(c);
        }
        return clean.toString();
    }
}
