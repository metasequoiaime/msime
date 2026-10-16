package app.msime.android;

import java.util.function.LongSupplier;

/** 导入已持久化后，趁词库空闲分批应用；不阻塞首次键盘输入，也不无限等待维护锁。 */
public final class DictionaryImportSynchronization {
    private static final long BUDGET_NANOS = 2_000_000_000L;
    private static final int MAX_BATCHES = 10_000;

    public enum Outcome { APPLIED, PENDING, FAILED }
    public record Batch(int pending, int sent, int failed) {}
    @FunctionalInterface public interface Step {
        Batch synchronize() throws Exception;
    }

    private DictionaryImportSynchronization() {}

    public static Outcome run(Step step, LongSupplier clock) {
        long started = clock.getAsLong();
        int previous = Integer.MAX_VALUE;
        boolean failed = false;
        for (int batch = 0; batch < MAX_BATCHES; batch++) {
            final Batch result;
            try {
                result = step.synchronize();
            } catch (Exception | LinkageError error) {
                // busy 或原生层暂不可用：词已保存，下次键盘空闲继续，不把导入说成失败。
                return failed ? Outcome.FAILED : Outcome.PENDING;
            }
            if (result.pending() < 0 || result.sent() < 0 || result.failed() < 0)
                return failed ? Outcome.FAILED : Outcome.PENDING;
            failed |= result.failed() > 0;
            if (result.pending() == 0 && result.sent() == 0) return failed ? Outcome.FAILED : Outcome.APPLIED;
            if (clock.getAsLong() - started >= BUDGET_NANOS) return failed ? Outcome.FAILED : Outcome.PENDING;
            if (result.sent() == 0 && result.pending() >= previous) return failed ? Outcome.FAILED : Outcome.PENDING;
            previous = result.pending();
        }
        return failed ? Outcome.FAILED : Outcome.PENDING;
    }

    public static String notice(Outcome outcome) {
        return switch (outcome) {
            case APPLIED -> "";
            case PENDING -> "词条已保存，等待键盘空闲后生效";
            case FAILED -> "部分词条未生效，请在个人词库中查看失败原因";
        };
    }
}
