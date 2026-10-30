import app.msime.android.PendingTypingStatistics;

/** Which directory the input service asks the native statistics store to flush. */
public final class PendingTypingStatisticsSmoke {
    public static void main(String[] args) {
        PendingTypingStatistics pending = new PendingTypingStatistics();
        check(pending.drain() == null, "nothing recorded means nothing to flush");
        check(pending.record("/a") == null, "the first record has no earlier directory");
        check(pending.record("/a") == null, "more records into the same directory flush nothing early");
        check("/a".equals(pending.drain()), "the recorded directory is flushed");
        check(pending.drain() == null, "a flushed directory is not flushed again");

        check(pending.record("/a") == null, "recording again after a flush starts over");
        check("/a".equals(pending.record("/b")), "moving to another directory hands the earlier one back");
        check("/b".equals(pending.drain()), "the later directory is the one left to flush");

        check(pending.record("") == null && pending.record(null) == null, "an empty directory is never noted");
        check(pending.drain() == null, "and leaves nothing to flush");
        System.out.println("Android typing statistics flush: pending directory tracked");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
