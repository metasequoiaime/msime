package app.msime.android;

import android.icu.text.BreakIterator;
import android.text.Spanned;
import android.text.style.ReplacementSpan;
import android.view.inputmethod.InputConnection;
import java.util.Locale;

/** 普通退格按系统字素边界删除，编辑器返回的内联表情范围也不能拆开。 */
final class EditorBackspaceBoundary {
    static final int CONTEXT_LENGTH = 1024;

    private EditorBackspaceBoundary() {}

    static CharSequence before(InputConnection connection) {
        CharSequence value = InputConnectionGuard.textBeforeCursor(connection,
            CONTEXT_LENGTH, InputConnection.GET_TEXT_WITH_STYLES);
        // 编辑器可能忽略长度提示；本地仍只处理查询窗口，保留窗口内的样式。
        return value == null || value.length() <= CONTEXT_LENGTH ? value
            : value.subSequence(value.length() - CONTEXT_LENGTH, value.length());
    }

    /** 占位结尾的私有范围可能没跨进程返回；只发退格，让编辑器决定范围，不解析表情标签。 */
    static boolean shouldDelegate(CharSequence before, int length) {
        if (before == null || before.length() == 0 || length != 1) return false;
        char last = before.charAt(before.length() - 1);
        return last == ']' || last == '\ufffc';
    }

    /** 返回 UTF-16 删除长度；上下文不足时返回 0，交给编辑器自己的退格。 */
    static int length(CharSequence before) {
        if (before == null || before.length() == 0) return 0;
        String text = before.toString();
        BreakIterator characters = BreakIterator.getCharacterInstance(Locale.ROOT);
        characters.setText(text);
        int start = characters.preceding(text.length());
        if (start == BreakIterator.DONE) return 0;
        if (text.length() >= CONTEXT_LENGTH) {
            // 区域指示符按整段奇偶数配对；窗口截断了整段时不能推测最后一面旗帜的起点。
            int regionalStart = text.length();
            while (regionalStart > 0) {
                int point = text.codePointBefore(regionalStart);
                if (point < 0x1f1e6 || point > 0x1f1ff) break;
                regionalStart -= Character.charCount(point);
            }
            if (regionalStart == 0 || regionalStart == 1 && Character.isLowSurrogate(text.charAt(0)))
                return 0;
        }
        if (before instanceof Spanned styled) {
            // 一个可见内联对象可以覆盖多个普通字素，删到它就覆盖整个对象。
            int previous;
            do {
                previous = start;
                for (ReplacementSpan span : styled.getSpans(start, text.length(), ReplacementSpan.class)) {
                    if (styled.getSpanStart(span) < text.length() && styled.getSpanEnd(span) > start)
                        start = Math.min(start, styled.getSpanStart(span));
                }
            } while (start < previous);
        }
        // 查询窗口恰好从字素中间开始时不能声称知道完整边界。
        return start == 0 && text.length() >= CONTEXT_LENGTH ? 0 : text.length() - start;
    }
}
