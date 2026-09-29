package app.msime.android;

/**
 * One clipboard history entry, as this keyboard draws it.
 *
 * <p>A data holder and nothing more. The ordering, the fifty-entry limit, eviction and the rule
 * that a pinned entry is never evicted used to live here, in a second implementation of what the
 * shared store already did — and that second implementation was the reason this keyboard and the
 * settings page disagreed about what the history contained. They belong to the shared store now,
 * and are reached through {@link ClipboardHistoryStore}.
 */
public final class ClipboardHistory {
    private ClipboardHistory() {}

    /**
     * An entry, identified by its text.
     *
     * <p>Text rather than a generated id because that is how the shared store names an entry, and
     * a local id would have to be mapped back to one on every removal or pin.
     */
    public static final class Item {
        private final String text;
        private final long timestamp;
        private final boolean pinned;

        public Item(String text, long timestamp, boolean pinned) {
            this.text = text;
            this.timestamp = timestamp;
            this.pinned = pinned;
        }

        public String text() { return text; }
        public long timestamp() { return timestamp; }
        public boolean pinned() { return pinned; }
    }
}
