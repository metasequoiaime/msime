import app.msime.android.DictionarySnapshotBacklog;

/** 一直在输入框之间切换时，被取消的词库快照处理最多欠一分钟。 */
public final class DictionarySnapshotBacklogSmoke {
    public static void main(String[] arguments) {
        DictionarySnapshotBacklog backlog = new DictionarySnapshotBacklog();
        check(!backlog.owed(), "nothing is owed before the first blur");

        check(backlog.owe(1_000), "the first blur defers its pass");
        check(backlog.owed(), "a deferred pass stays owed until it runs");
        check(backlog.owe(1_000 + DictionarySnapshotBacklog.MAX_DEFER_MILLIS - 1),
            "blurs just under the bound keep deferring");
        check(!backlog.owe(1_000 + DictionarySnapshotBacklog.MAX_DEFER_MILLIS),
            "the blur that reaches the bound queues its pass at once");
        check(!backlog.owe(1_000 + 2 * DictionarySnapshotBacklog.MAX_DEFER_MILLIS),
            "until a pass settles, later blurs stay overdue");

        backlog.settled();
        check(!backlog.owed(), "a settled pass owes nothing");
        check(backlog.owe(500_000), "after a settled pass the next blur defers again");
        check(backlog.owe(500_000 + DictionarySnapshotBacklog.MAX_DEFER_MILLIS / 2),
            "and the clock starts at that blur, not at the first one ever");
        System.out.println("Android dictionary snapshot backlog: deferral bounded");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
