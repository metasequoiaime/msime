package app.msime.android;

import java.util.ArrayDeque;
import java.util.Objects;

/** 快速删除的一次性内存备份；不接入剪贴板、统计或磁盘。 */
public final class QuickDeleteUndo {
    /** 单次撤回只提交至多 128 KiB UTF-16，给共享的 Binder 事务缓冲留出余量。 */
    public static final int MAX_UNITS = 64 * 1024;
    public static final int CONTEXT_UNITS = 64;
    public static final long LIFETIME_MS = 60_000;

    /** 只收集编辑器明确接受删除的部分，倒序拼回原文；超限立即放弃备份。 */
    public static final class Capture {
        private final ArrayDeque<String> chunks = new ArrayDeque<>();
        private int units;
        private boolean overflow;

        public void deleted(CharSequence text) {
            if (overflow || text == null || text.length() == 0) return;
            if (text.length() > MAX_UNITS - units) {
                overflow = true;
                chunks.clear();
                return;
            }
            chunks.addFirst(text.toString());
            units += text.length();
        }

        public String text() {
            if (overflow || units == 0) return "";
            StringBuilder value = new StringBuilder(units);
            for (String chunk : chunks) value.append(chunk);
            return value.toString();
        }
    }

    private final Object target;
    private final int cursor;
    private final String text;
    private final String before;
    private final String after;
    private final long createdAt;

    public QuickDeleteUndo(Object target, int cursor, String text,
                           String before, String after, long createdAt) {
        this.target = Objects.requireNonNull(target);
        this.cursor = cursor;
        this.text = Objects.requireNonNull(text);
        this.before = context(before, true);
        this.after = context(after, false);
        this.createdAt = createdAt;
    }

    public boolean atSelection(int start, int end) {
        return start == cursor && end == cursor;
    }

    public boolean matches(Object currentTarget, int currentCursor, CharSequence currentBefore,
                           CharSequence currentAfter, long now) {
        return target == currentTarget && cursor >= 0 && currentCursor == cursor
            && !text.isEmpty() && text.length() <= MAX_UNITS
            && now >= createdAt && now - createdAt < LIFETIME_MS
            && before != null && after != null && currentBefore != null && currentAfter != null
            && before.equals(context(currentBefore, true)) && after.equals(context(currentAfter, false));
    }

    /** 防止编辑器忽略读取长度时把整份文档当作短上下文保存。 */
    private static String context(CharSequence value, boolean preceding) {
        if (value == null) return null;
        int count = Math.min(value.length(), CONTEXT_UNITS);
        int start = preceding ? value.length() - count : 0;
        return value.subSequence(start, start + count).toString();
    }

    public String text() { return text; }
}
