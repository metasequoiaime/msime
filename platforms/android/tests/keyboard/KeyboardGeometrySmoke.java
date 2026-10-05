import app.msime.android.KeyboardGapPolicy;
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
        check(KeyboardGeometry.strictInt(45.0, -1) == -1);
        check(KeyboardGeometry.strictLong(45.0, -1) == -1);
        // 键距空隙归属：键帽 100x50，左右外边距 9px、上下 10px。
        check(KeyboardGapPolicy.gapDistance(50, 25, 100, 50, 9, 10, 9, 10) == 0f);
        check(KeyboardGapPolicy.gapDistance(-4, 25, 100, 50, 9, 10, 9, 10) == 4f);
        check(KeyboardGapPolicy.gapDistance(50, 54, 100, 50, 9, 10, 9, 10) == 5f);
        check(KeyboardGapPolicy.gapDistance(-3, -4, 100, 50, 9, 10, 9, 10) == 5f);
        check(KeyboardGapPolicy.gapDistance(-9, 25, 100, 50, 9, 10, 9, 10) == 9f);
        check(KeyboardGapPolicy.gapDistance(-9.5f, 25, 100, 50, 9, 10, 9, 10) < 0);
        check(KeyboardGapPolicy.gapDistance(109, 25, 100, 50, 9, 10, 9, 10) < 0);
        check(KeyboardGapPolicy.gapDistance(50, 60, 100, 50, 9, 10, 9, 10) < 0);
        // 没有外边距的控件（工具栏图标、方案胶囊）不认领任何空隙。
        check(KeyboardGapPolicy.gapDistance(-1, 25, 100, 50, 0, 0, 0, 0) < 0);
        // 挪进键帽时离边缘留 1px，键帽里的点不动。
        check(KeyboardGapPolicy.inside(-4, 100) == 1f);
        check(KeyboardGapPolicy.inside(104, 100) == 99f);
        check(KeyboardGapPolicy.inside(37.5f, 100) == 37.5f);
        System.out.println("Android keyboard geometry: Apple defaults, bounds and precision passed");
    }
}
