import app.msime.android.KeyboardGeometry;

public final class KeyboardGeometrySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(KeyboardGeometry.keySpacing(-1) == 60);
        check(KeyboardGeometry.rowSpacing(-1) == 70);
        check(KeyboardGeometry.keySpacing(29) == 30);
        check(KeyboardGeometry.keySpacing(61) == 60);
        check(KeyboardGeometry.rowSpacing(39) == 40);
        check(KeyboardGeometry.rowSpacing(101) == 100);
        check(KeyboardGeometry.keySpacing(35) == 35);
        check(KeyboardGeometry.rowSpacing(95) == 95);
        check(KeyboardGeometry.heightAdjustment(Integer.MIN_VALUE) == 0);
        check(KeyboardGeometry.CANDIDATE_ROW_HEIGHT_DP == 48);
        check(KeyboardGeometry.heightAdjustment(-13) == -12);
        check(KeyboardGeometry.heightAdjustment(49) == 48);
        check(KeyboardGeometry.heightAdjustment(24) == 24);
        check(KeyboardGeometry.adjustedRowHeight(48, 0, 3, 0) == 48);
        check(KeyboardGeometry.adjustedRowHeight(48, 24, 3, 2) == 56);
        check(KeyboardGeometry.adjustedRowHeight(48, -12, 3, 1) == 44);
        check(KeyboardGeometry.adjustedRowHeight(48, 1, 3, 0) == 49);
        check(KeyboardGeometry.adjustedRowHeight(48, 1, 3, 1) == 48);
        check(KeyboardGeometry.displayHeight(0).equals("0"));
        check(KeyboardGeometry.displayHeight(24).equals("+24"));
        check(KeyboardGeometry.display(35).equals("3.5"));
        check(KeyboardGeometry.display(100).equals("10.0"));
        check(KeyboardGeometry.halfGapPixels(60, 1) == 3);
        check(KeyboardGeometry.halfGapPixels(35, 2) == 4);
        check(KeyboardGeometry.halfGapPixels(60, Float.NaN) == 0);
        System.out.println("Android keyboard geometry: Apple defaults, bounds and precision passed");
    }
}
