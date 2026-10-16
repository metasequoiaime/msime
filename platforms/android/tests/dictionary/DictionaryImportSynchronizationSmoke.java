import app.msime.android.DictionaryImportSynchronization;
import app.msime.android.DictionaryImportSynchronization.Batch;
import app.msime.android.DictionaryImportSynchronization.Outcome;

public final class DictionaryImportSynchronizationSmoke {
    private static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        int[] pending = {74};
        int[] calls = {0};
        Outcome applied = DictionaryImportSynchronization.run(() -> {
            calls[0]++;
            pending[0] = Math.max(0, pending[0] - 4);
            return new Batch(pending[0], 0, 0);
        }, () -> 0);
        check(applied == Outcome.APPLIED && pending[0] == 0 && calls[0] == 19);

        // 队列容量为 128：一批写完后还可能送入下一批，不能看见 pending=0 就提前结束。
        int[] batch = {0};
        check(DictionaryImportSynchronization.run(() -> {
            batch[0]++;
            return batch[0] == 1 ? new Batch(0, 6, 0) : new Batch(0, 0, 0);
        }, () -> 0) == Outcome.APPLIED && batch[0] == 2);

        check(DictionaryImportSynchronization.run(() -> { throw new IllegalStateException("busy"); },
            () -> 0) == Outcome.PENDING);
        check(DictionaryImportSynchronization.run(() -> new Batch(0, 0, 1), () -> 0) == Outcome.FAILED);
        check(DictionaryImportSynchronization.run(() -> new Batch(-1, 0, 0), () -> 0) == Outcome.PENDING);
        // 一条坏词不能阻止后面仍然有效的词继续应用，最后统一报告失败。
        pending[0] = 12;
        calls[0] = 0;
        check(DictionaryImportSynchronization.run(() -> {
            calls[0]++;
            pending[0] -= 4;
            return new Batch(pending[0], 0, 1);
        }, () -> 0) == Outcome.FAILED && calls[0] == 3);
        check(DictionaryImportSynchronization.run(() -> new Batch(0, 0, -1), () -> 0) == Outcome.PENDING);
        calls[0] = 0;
        check(DictionaryImportSynchronization.run(() -> {
            calls[0]++;
            return new Batch(7, 0, 0);
        }, () -> 0) == Outcome.PENDING && calls[0] == 2);
        long[] clock = {0};
        calls[0] = 0;
        check(DictionaryImportSynchronization.run(() -> {
            calls[0]++;
            clock[0] += 2_000_000_000L;
            return new Batch(7, 0, 0);
        }, () -> clock[0]) == Outcome.PENDING && calls[0] == 1);
        check(DictionaryImportSynchronization.notice(Outcome.APPLIED).isEmpty());
        check(DictionaryImportSynchronization.notice(Outcome.PENDING).contains("等待"));
        check(DictionaryImportSynchronization.notice(Outcome.FAILED).contains("失败原因"));
    }
}
