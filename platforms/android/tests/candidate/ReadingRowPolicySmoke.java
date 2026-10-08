import app.msime.android.ReadingRowPolicy;

public final class ReadingRowPolicySmoke {
    static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
    }

    public static void main(String[] args) {
        // 3x 密度的设计高度 14dp = 42px。16sp 的读音在 3x 上 ascent 约 -45、descent 约 12，文字要 57px；按设计高度画就切掉 y、g 的下伸部（#5591）。
        check(ReadingRowPolicy.heightPx(42, -45, 12, 0) == 57, "a 16sp reading grows the row to its full ink height");
        // 读音视图自己的上下内边距也算进去。
        check(ReadingRowPolicy.heightPx(42, -45, 12, 4) == 61, "vertical padding is part of the row");
        // 12sp 的读音（3x 上 ascent 约 -33、descent 约 9）本来就放得下，行高保持设计值，键盘总高与改动前相同。
        check(ReadingRowPolicy.heightPx(42, -33, 9, 0) == 42, "a reading that fits keeps the design height");
        // 字体度量异常（descent 小于 ascent）或负的内边距不会把行高算成负数或比设计还矮。
        check(ReadingRowPolicy.heightPx(42, 10, -10, -6) == 42, "bad metrics never shrink the row");
        check(ReadingRowPolicy.heightPx(-1, 0, 0, 0) == 0, "a negative design height is not a height");
        System.out.println("Android reading row: the row is as tall as the reading's ink passed");
    }
}
