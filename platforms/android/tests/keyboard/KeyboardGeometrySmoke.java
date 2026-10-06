import app.msime.android.KeyboardGapPolicy;
import app.msime.android.KeyboardGeometry;

public final class KeyboardGeometrySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // 与 crates/client-core 的 default_touch_key_spacing_tenths / default_touch_row_spacing_tenths 同值。
        check(KeyboardGeometry.keySpacing(-1) == 60);
        check(KeyboardGeometry.rowSpacing(-1) == 70);
        check(KeyboardGeometry.DESIGN_KEY_GAP_DP == 5 && KeyboardGeometry.DESIGN_ROW_GAP_DP == 8);
        check(KeyboardGeometry.DESIGN_PADDING_TOP_DP == 8
            && KeyboardGeometry.DESIGN_PADDING_HORIZONTAL_DP == 6
            && KeyboardGeometry.DESIGN_PADDING_BOTTOM_DP == 6);
        // 百分比换算与 Rust height_percent_to_adjustment 同式：12 档逐一钉住，往返不变。
        int[] percents = {75, 80, 85, 90, 95, 100, 105, 110, 115, 120, 125, 130};
        int[] adjustments = {-46, -37, -28, -18, -9, 0, 9, 18, 28, 37, 46, 55};
        for (int index = 0; index < percents.length; index++) {
            check(KeyboardGeometry.heightPercentToAdjustment(percents[index]) == adjustments[index]);
            check(KeyboardGeometry.heightAdjustmentToPercent(adjustments[index]) == percents[index]);
        }
        check(KeyboardGeometry.heightPercentToAdjustment(60) == -46);
        check(KeyboardGeometry.heightPercentToAdjustment(200) == 55);
        check(KeyboardGeometry.designHeightAdjustment(Integer.MIN_VALUE) == 0);
        check(KeyboardGeometry.designHeightAdjustment(-60) == -46);
        check(KeyboardGeometry.designHeightAdjustment(60) == 55);
        check(KeyboardGeometry.designKeyHeight(100) == 46 && KeyboardGeometry.designKeyHeight(127) == 58);
        check(KeyboardGeometry.displayPercent(127).equals("127%"));
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
