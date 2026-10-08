import app.msime.android.ReadingRowPolicy;

public final class ReadingRowPolicySmoke {
    static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
    }

    public static void main(String[] args) {
        // 3x 密度的设计高度 14dp = 42px。12sp 的读音在系统字体放大 1.15 倍时是 41.4px，默认字体 ascent 约 -39、descent 约 11，文字要 50px；按设计高度画就切掉 y、g 的下伸部（#5591）。厂商字体更高时差得更多。
        check(ReadingRowPolicy.heightPx(42, -39, 11, 0) == 50, "a scaled-up 12sp reading grows the row to its full ink height");
        // 读音视图自己的上下内边距也算进去。
        check(ReadingRowPolicy.heightPx(42, -39, 11, 4) == 54, "vertical padding is part of the row");
        // 系统字体不放大时 12sp 的读音（3x 上 ascent 约 -33、descent 约 9）本来就放得下，行高保持设计值，键盘总高与改动前相同。
        check(ReadingRowPolicy.heightPx(42, -33, 9, 0) == 42, "a reading that fits keeps the design height");
        // 字体度量异常（descent 小于 ascent）或负的内边距不会把行高算成负数或比设计还矮。
        check(ReadingRowPolicy.heightPx(42, 10, -10, -6) == 42, "bad metrics never shrink the row");
        check(ReadingRowPolicy.heightPx(-1, 0, 0, 0) == 0, "a negative design height is not a height");
        System.out.println("Android reading row: the row is as tall as the reading's ink passed");
    }
}
