package app.msime.android;

/** Ordered composition writes, independent of Android so the contract is JVM-testable. */
public final class EditorBridge {
    public interface Sink {
        void begin();
        boolean commit(String text);
        boolean compose(String text);
        boolean finish();
        boolean select(int start, int end);
        void end();
    }
    private boolean composing;
    public boolean apply(Sink sink, String commit, String editing) {
        sink.begin();
        try {
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
        } finally { sink.end(); }
    }
    /** External selection/focus changes must not delete an editor's new selection. */
    public void abandon(Sink sink) { composing = false; sink.finish(); }

    /**
     * Removes the marked composition instead of leaving it in the document, for a composition that is not text the user wrote: Stroke marks its stroke glyphs (一丨丿丶乛＊), and `commits_on_blur` is false, so finishing the region would write those glyphs as real characters. The editor's new selection is put back where the user moved it, shifted left by the removed text when it lies after the region. Without a reported region (composingStart < 0) the region is still removed, which leaves the caret where the composition was rather than corrupting the text.
     */
    public void discard(Sink sink, int composingStart, int composingEnd, int selectionStart, int selectionEnd) {
        composing = false;
        sink.begin();
        try {
            sink.compose("");
            sink.finish();
            if (composingStart >= 0 && composingEnd >= composingStart) {
                sink.select(shifted(selectionStart, composingStart, composingEnd),
                    shifted(selectionEnd, composingStart, composingEnd));
            }
        } finally { sink.end(); }
    }

    /** Removes the marked composition where the caret still is: the session ends with nothing moved, so there is no selection to restore. */
    public void discard(Sink sink) { discard(sink, -1, -1, -1, -1); }

    /** Where a selection offset lands once [start, end) has been removed. */
    static int shifted(int offset, int start, int end) {
        if (offset >= end) return offset - (end - start);
        return Math.min(offset, start);
    }
}
