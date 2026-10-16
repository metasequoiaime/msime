package app.msime.android;

/** 读回编辑器文字，判断长度过滤器是否完整保留了组字区。 */
public final class CompositionTextPolicy {
    private CompositionTextPolicy() {}

    /**
     * 编辑器不提供最大长度，但 `setComposingText` 所在批量编辑结束后可以读光标前的文字。
     * 批内读取可能只是 Compose 的旧快照，不能用来判断截短。
     * 读不到上下文时保守地认为成功，避免把正常组字误判成被截短。
     */
    public static boolean matchesEditorSuffix(CharSequence beforeCursor, String composing) {
        int length = composingLength(beforeCursor, composing);
        return length < 0 || length == (composing == null ? 0 : composing.length());
    }

    /** 返回编辑器实际保留的组字前缀长度；上下文不可读时返回 -1。 */
    public static int composingLength(CharSequence beforeCursor, String composing) {
        if (composing == null || composing.isEmpty()) return 0;
        if (beforeCursor == null) return -1;
        String before = beforeCursor.toString();
        for (int length = composing.length(); length >= 0; length--) {
            if (before.endsWith(composing.substring(0, length))) return length;
        }
        return 0;
    }
}
