package app.msime.android;

/** Ordered composition writes, independent of Android so the contract is JVM-testable. */
public final class EditorBridge {
    public interface Sink {
        void begin();
        boolean commit(String text);
        boolean compose(String text);
        boolean finish();
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
}
