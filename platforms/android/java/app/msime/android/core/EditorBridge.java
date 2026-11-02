package app.msime.android;

/** 按顺序写入组字，不依赖 Android，便于在 JVM 上验证契约。 */
public final class EditorBridge {
    public interface Sink {
        void begin();
        boolean commit(String text);
        boolean compose(String text);
        boolean finish();
        boolean select(int start, int end);
        void end();
        /** 批量编辑结束后才能读回实际文字；验证或回退拒写时向调用方报告失败。 */
        default boolean verify() { return true; }
        /** 已提交且结束批次后，宿主核对真实文本并尽力定位；不支持时保留文字和默认位置。 */
        default void positionCommitCursor(String text, String suffix) {}
    }
    private boolean composing;
    public boolean apply(Sink sink, String commit, String editing) {
        return apply(sink, commit, editing, null);
    }

    /** 显式光标先于下一段组字生效，读回和定位不发生在尚未应用的批次里。 */
    public boolean apply(Sink sink, String commit, String editing, String cursorSuffix) {
        if (commit == null || cursorSuffix == null || cursorSuffix.isEmpty() || !commit.endsWith(cursorSuffix))
            return applyBatch(sink, commit, editing);
        if (!applyBatch(sink, commit, "")) return false;
        try { sink.positionCommitCursor(commit, cursorSuffix); }
        catch (RuntimeException disconnected) {
            // 文字已经提交，定位失败不能让调用方重试并重复上屏。
        }
        return editing.isEmpty() || applyBatch(sink, null, editing);
    }

    private boolean applyBatch(Sink sink, String commit, String editing) {
        if (!begin(sink)) return false;
        boolean written;
        boolean ended;
        try { written = applyWrites(sink, commit, editing); }
        catch (RuntimeException disconnected) { written = false; }
        finally { ended = end(sink); }
        if (!written || !ended) return false;
        try { return sink.verify(); }
        catch (RuntimeException disconnected) { return false; }
    }

    private boolean applyWrites(Sink sink, String commit, String editing) {
        if (commit != null) {
            if (!sink.commit(commit)) return false;
            composing = false;
        }
        if (!editing.isEmpty()) {
            if (!sink.compose(editing)) return false;
            composing = true;
        } else if (composing) {
            if (!sink.compose("")) return false;
            composing = false;
            if (!sink.finish()) return false;
        }
        return true;
    }
    /** External selection/focus changes must not delete an editor's new selection. */
    public void abandon(Sink sink) {
        composing = false;
        try { sink.finish(); }
        catch (RuntimeException disconnected) { /* 连接已断开时没有组字可结束。 */ }
    }

    /**
     * Removes the marked composition instead of leaving it in the document, for a composition that is not text the user wrote: Stroke marks its stroke glyphs (一丨丿丶乛＊), and `commits_on_blur` is false, so finishing the region would write those glyphs as real characters. The editor's new selection is put back where the user moved it, shifted left by the removed text when it lies after the region. Without a reported region (composingStart < 0) the region is still removed, which leaves the caret where the composition was rather than corrupting the text.
     */
    public void discard(Sink sink, int composingStart, int composingEnd, int selectionStart, int selectionEnd) {
        composing = false;
        if (!begin(sink)) return;
        try {
            try {
                sink.compose("");
                sink.finish();
                if (composingStart >= 0 && composingEnd >= composingStart) {
                    sink.select(shifted(selectionStart, composingStart, composingEnd),
                        shifted(selectionEnd, composingStart, composingEnd));
                }
            } catch (RuntimeException disconnected) {
                // 失去焦点后清理组字只是尽力而为，不能让输入法进程随编辑器一起退出。
            }
        } finally { end(sink); }
    }

    /**
     * 不经引擎结束组字区，但组字区里画的不是确定时的文字（日语末尾单个 `n` 画成字母，确定时是ん）：先把组字区换成 {@code settled} 再结束，留在文档里的是确定的文字。用户移到的选区放回原处：在组字区之后的按两段文字的长度差平移，落在组字区里的不超出新文字。
     *
     * <p>只替换编辑器报告还在的组字区（composingStart >= 0 且不为空）。编辑器报告没有组字区时（应用自己改了文字或去掉了组字区：发送后清空的聊天框、值被重置的 Compose/Flutter 输入框、自己确定了组字的编辑器），{@code setComposingText} 会在光标处插入新文字并替换选中的文字，所以这时和 {@link #abandon} 一样只结束组字，不写任何文字。
     */
    public void settle(Sink sink, String settled, int composingStart, int composingEnd,
            int selectionStart, int selectionEnd) {
        if (composingStart < 0 || composingEnd <= composingStart) {
            abandon(sink);
            return;
        }
        composing = false;
        if (!begin(sink)) return;
        try {
            try {
                sink.compose(settled);
                sink.finish();
                sink.select(resettled(selectionStart, composingStart, composingEnd, settled.length()),
                    resettled(selectionEnd, composingStart, composingEnd, settled.length()));
            } catch (RuntimeException disconnected) {
                // 失去焦点后结束组字只是尽力而为，不能让输入法进程随编辑器一起退出。
            }
        } finally { end(sink); }
    }

    /** [start, end) 换成长度为 {@code length} 的文字之后，选区端点落在哪里。 */
    static int resettled(int offset, int start, int end, int length) {
        if (offset >= end) return offset - (end - start) + length;
        if (offset <= start) return offset;
        return BoundsPolicy.atMost(offset, start + length);
    }

    /** Removes the marked composition where the caret still is: the session ends with nothing moved, so there is no selection to restore. */
    public void discard(Sink sink) { discard(sink, -1, -1, -1, -1); }

    private static boolean begin(Sink sink) {
        try {
            sink.begin();
            return true;
        } catch (RuntimeException disconnected) {
            return false;
        }
    }

    private static boolean end(Sink sink) {
        try { sink.end(); return true; }
        catch (RuntimeException disconnected) { return false; }
    }

    /** Where a selection offset lands once [start, end) has been removed. */
    static int shifted(int offset, int start, int end) {
        if (offset >= end) return offset - (end - start);
        return BoundsPolicy.atMost(offset, start);
    }
}
