package app.msime.android;

/**
 * Which statistics directory may still hold committed text the native store has not written yet.
 *
 * <p>The native store keeps each commit's counts in memory and writes them at most every half minute, or when the host asks it to flush. The host asks when the editor or the keyboard goes away, and only for the directory it actually recorded into since the last flush, so an idle editor change costs no native call. Recording into a different directory hands the earlier one back first: nothing else would ever flush it.
 *
 * <p>Holds a path only, never text. Not thread-safe: the input service touches it on the main thread only.
 */
public final class PendingTypingStatistics {
    private String directory;

    /**
     * Notes a commit recorded into {@code directory}.
     *
     * @return the earlier directory that must be flushed now because the records moved elsewhere, or {@code null}.
     */
    public String record(String directory) {
        if (directory == null || directory.isEmpty()) return null;
        String previous = this.directory == null || this.directory.equals(directory) ? null : this.directory;
        this.directory = directory;
        return previous;
    }

    /** The directory to flush, which is then forgotten; {@code null} when nothing was recorded since the last flush. */
    public String drain() {
        String pending = directory;
        directory = null;
        return pending;
    }
}
