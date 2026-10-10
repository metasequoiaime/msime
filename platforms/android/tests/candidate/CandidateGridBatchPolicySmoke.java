import app.msime.android.CandidateGridBatchPolicy;

public final class CandidateGridBatchPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    static void rejects(Runnable operation) {
        try {
            operation.run();
            throw new AssertionError("Expected invalid candidate grid input to be rejected");
        } catch (IllegalArgumentException expected) {
            // Expected.
        }
    }

    public static void main(String[] args) {
        int batch = CandidateGridBatchPolicy.BATCH;
        // #6471：606 个候选打开时只建第一批，与总数无关。
        check(CandidateGridBatchPolicy.initialCount(606, 0, false) == batch, "first batch only");
        check(CandidateGridBatchPolicy.initialCount(606, 300, false) == batch,
            "a new generation starts again from the first batch");
        check(CandidateGridBatchPolicy.initialCount(10, 0, false) == 10, "short lists are built whole");
        check(CandidateGridBatchPolicy.initialCount(0, 0, true) == 0, "empty list");
        // 同一代重画（释义晚到）建回已经滚出来的格子，滚动位置不被截断。
        check(CandidateGridBatchPolicy.initialCount(606, 300, true) == 300, "redraw keeps built cells");
        check(CandidateGridBatchPolicy.initialCount(606, 5, true) == batch, "redraw builds at least a batch");
        check(CandidateGridBatchPolicy.initialCount(200, 300, true) == 200, "never more than the total");

        check(CandidateGridBatchPolicy.nextCount(batch, 606) == 2 * batch, "next batch");
        check(CandidateGridBatchPolicy.nextCount(600, 606) == 606, "last batch is clipped");
        check(CandidateGridBatchPolicy.nextCount(Integer.MAX_VALUE - 1, Integer.MAX_VALUE)
            == Integer.MAX_VALUE, "no overflow");

        // 视口 300、网格底边 1000：滚到 400 时下一屏到 1000，开始追加；更早不追加。
        check(!CandidateGridBatchPolicy.shouldAppend(batch, 606, 0, 300, 1000), "far from the bottom");
        check(!CandidateGridBatchPolicy.shouldAppend(batch, 606, 399, 300, 1000), "just short of a screen");
        check(CandidateGridBatchPolicy.shouldAppend(batch, 606, 400, 300, 1000), "within a screen");
        // 第一批没填够一屏多时不等滚动就追加。
        check(CandidateGridBatchPolicy.shouldAppend(batch, 606, 0, 300, 500), "first batch too short");
        check(!CandidateGridBatchPolicy.shouldAppend(606, 606, 0, 300, 100), "everything built");
        check(!CandidateGridBatchPolicy.shouldAppend(batch, 606, 0, 0, 0), "not laid out yet");

        rejects(() -> CandidateGridBatchPolicy.initialCount(-1, 0, false));
        rejects(() -> CandidateGridBatchPolicy.initialCount(1, -1, true));
        rejects(() -> CandidateGridBatchPolicy.nextCount(2, 1));
        rejects(() -> CandidateGridBatchPolicy.nextCount(-1, 1));
        rejects(() -> CandidateGridBatchPolicy.shouldAppend(2, 1, 0, 1, 1));
        rejects(() -> CandidateGridBatchPolicy.shouldAppend(0, 1, 0, -1, 1));
        System.out.println("Android candidate grid: batched build, redraw retention and append threshold passed");
    }
}
