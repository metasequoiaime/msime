import app.msime.android.CandidateGridPolicy;
import java.util.Arrays;

public final class CandidateGridPolicySmoke {
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
        // 360 dp 宽的手机减去两侧 8 dp 内边距：每列至少 64、间隔 6，放得下五列，而不是 88 dp 的 Button 默认最小宽度下的三格。
        check(CandidateGridPolicy.columns(344, 64, 6) == 5, "phone width fits five columns");
        check(CandidateGridPolicy.columns(134, 64, 6) == 2, "two columns exactly");
        check(CandidateGridPolicy.columns(63, 64, 6) == 1, "narrower than a column still has one");
        check(CandidateGridPolicy.columns(0, 64, 6) == 1, "zero width has one column");

        // 余下的像素摊到各列：最后一列贴住右边，整行的格子正好等于可用宽度。
        check(CandidateGridPolicy.columnStart(0, 5, 344, 6) == 0, "first column starts at the edge");
        check(CandidateGridPolicy.columnStart(4, 5, 344, 6) == 280, "fifth column start");
        check(CandidateGridPolicy.cellWidth(4, 1, 5, 344, 6) == 64, "last column width");
        check(CandidateGridPolicy.cellWidth(0, 5, 5, 344, 6) == 344, "a full-row cell spans the width");
        check(CandidateGridPolicy.cellWidth(0, 2, 5, 344, 6) == 134, "a two-column cell includes the gap");
        check(CandidateGridPolicy.cellWidth(0, 3, 4, 343, 6) + 6 + CandidateGridPolicy.cellWidth(3, 1, 4, 343, 6)
            == 343, "uneven remainders still end at the right edge");

        check(CandidateGridPolicy.span(54, 5, 344, 6) == 1, "a two-character word takes one column");
        check(CandidateGridPolicy.span(64, 5, 344, 6) == 1, "exactly one column");
        check(CandidateGridPolicy.span(71, 5, 344, 6) == 2, "a wider word takes two columns");
        check(CandidateGridPolicy.span(0, 5, 344, 6) == 1, "an empty cell still takes a column");
        check(CandidateGridPolicy.span(10_000, 5, 344, 6) == 5, "never more than a row");

        // 一行剩下的列放不下下一格时换行，换行前那一格拉宽到行尾；最后一行不拉宽。
        int[][] cells = CandidateGridPolicy.place(5, new int[] {1, 1, 2, 2, 2, 1});
        check(Arrays.deepEquals(cells, new int[][] {
            {0, 0, 1}, {0, 1, 1}, {0, 2, 3}, {1, 0, 2}, {1, 2, 2}, {1, 4, 1}
        }), "rows fill to the right edge: " + Arrays.deepToString(cells));
        check(Arrays.deepEquals(CandidateGridPolicy.place(4, new int[] {1, 1}),
            new int[][] {{0, 0, 1}, {0, 1, 1}}), "the last row is not stretched");
        check(Arrays.deepEquals(CandidateGridPolicy.place(2, new int[] {2, 1}),
            new int[][] {{0, 0, 2}, {1, 0, 1}}), "a full row needs no stretch");
        check(CandidateGridPolicy.place(3, new int[0]).length == 0, "empty grid");

        rejects(() -> CandidateGridPolicy.columns(-1, 64, 6));
        rejects(() -> CandidateGridPolicy.columns(100, 0, 6));
        rejects(() -> CandidateGridPolicy.columns(100, 64, -1));
        rejects(() -> CandidateGridPolicy.columnStart(6, 5, 344, 6));
        rejects(() -> CandidateGridPolicy.cellWidth(4, 2, 5, 344, 6));
        rejects(() -> CandidateGridPolicy.span(-1, 5, 344, 6));
        rejects(() -> CandidateGridPolicy.place(0, new int[] {1}));
        rejects(() -> CandidateGridPolicy.place(3, new int[] {4}));
        rejects(() -> CandidateGridPolicy.place(3, new int[] {0}));
        System.out.println("Android candidate grid: columns, spans and row fill passed");
    }
}
