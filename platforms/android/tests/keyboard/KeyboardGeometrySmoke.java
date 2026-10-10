import app.msime.android.BoundsPolicy;
import app.msime.android.KeyboardGapPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardLayout;

public final class KeyboardGeometrySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        // 与 crates/client-core 的 default_touch_key_spacing_tenths / default_touch_row_spacing_tenths 同值。
        check(KeyboardGeometry.keySpacing(-1) == 60);
        check(BoundsPolicy.nonNegative(-1) == 0 && BoundsPolicy.nonNegative(7) == 7);
        check(BoundsPolicy.nonNegative(-1L) == 0L && BoundsPolicy.nonNegative(7L) == 7L);
        check(KeyboardGeometry.rowSpacing(-1) == 70);
        check(KeyboardGeometry.DESIGN_KEY_GAP_DP == 5 && KeyboardGeometry.DESIGN_ROW_GAP_DP == 8);
        check(KeyboardGeometry.DESIGN_PADDING_TOP_DP == 8
            && KeyboardGeometry.DESIGN_PADDING_HORIZONTAL_DP == 6
            && KeyboardGeometry.DESIGN_PADDING_BOTTOM_DP == 6);
        // 底栏（#6354）：默认 46 dp 不加行距，键帽比键行矮一截；「加高底行」开着时和一行键同高、另加一份行距。
        check(KeyboardGeometry.bottomRowHeightDp(false) == KeyboardGeometry.STANDARD_ROW_HEIGHT_DP
            && KeyboardGeometry.bottomRowSpacings(false) == 0, "bottom row keeps 46 dp by default");
        check(KeyboardGeometry.bottomRowHeightDp(true) == KeyboardGeometry.KEY_ROW_HEIGHT_DP
            && KeyboardGeometry.bottomRowSpacings(true) == 1, "tall bottom row matches a key row");
        // 百分比换算 round(184 × (p − 100) / 100)：逐档钉住，往返不变。
        int[] percents = {75, 80, 85, 90, 95, 100, 105, 110, 115, 120, 125, 130, 140, 150, 160};
        int[] adjustments = {-46, -37, -28, -18, -9, 0, 9, 18, 28, 37, 46, 55, 74, 92, 110};
        for (int index = 0; index < percents.length; index++) {
            check(KeyboardGeometry.heightPercentToAdjustment(percents[index]) == adjustments[index]);
            check(KeyboardGeometry.heightAdjustmentToPercent(adjustments[index]) == percents[index]);
        }
        check(KeyboardGeometry.heightPercentToAdjustment(60) == -46);
        check(KeyboardGeometry.heightPercentToAdjustment(200) == 110);
        check(KeyboardGeometry.designHeightAdjustment(Integer.MIN_VALUE) == 0);
        check(KeyboardGeometry.designHeightAdjustment(-60) == -46);
        check(KeyboardGeometry.designHeightAdjustment(60) == 60);
        check(KeyboardGeometry.designHeightAdjustment(200) == 110);
        // #5564：130% 以内照画；更高的部分最多占窗口高度的 14%，但不低于 130% 的 55 dp。
        check(KeyboardGeometry.windowHeightAdjustment(55, 360) == 55);
        check(KeyboardGeometry.windowHeightAdjustment(-46, 360) == -46);
        check(KeyboardGeometry.windowHeightAdjustment(Integer.MIN_VALUE, 800) == 0);
        check(KeyboardGeometry.windowHeightAdjustment(110, 792) == 110, "portrait phone reaches 160%");
        check(KeyboardGeometry.windowHeightAdjustment(110, 700) == 98, "shorter window caps the extra height");
        check(KeyboardGeometry.windowHeightAdjustment(110, 360) == 55, "landscape phone stops at 130%");
        check(KeyboardGeometry.windowHeightAdjustment(110, 0) == 55, "unknown window stops at 130%");
        check(KeyboardGeometry.windowHeightAdjustment(500, 5000) == 110, "never beyond the design range");
        // 行高按新设计的范围钳制：-46 与 110 都要画出来，不再被共享偏好的 -12..48 截住。
        check(KeyboardGeometry.adjustedRowHeight(56, 110, 3, 0) == 93
            && KeyboardGeometry.adjustedRowHeight(56, 110, 3, 1) == 93
            && KeyboardGeometry.adjustedRowHeight(56, 110, 3, 2) == 92);
        check(KeyboardGeometry.adjustedRowHeight(56, -46, 3, 0) == 41);
        check(KeyboardGeometry.adjustedRowHeight(168, 55, 1, 0) == 223);
        check(KeyboardGeometry.designKeyHeight(100) == 46 && KeyboardGeometry.designKeyHeight(127) == 58);
        check(KeyboardGeometry.displayPercent(127).equals("127%"));
        check(KeyboardGeometry.keySpacing(29) == 30);
        check(KeyboardGeometry.keySpacing(61) == 60);
        check(KeyboardGeometry.rowSpacing(39) == 40);
        check(KeyboardGeometry.rowSpacing(101) == 100);
        check(KeyboardGeometry.keySpacing(35) == 35);
        check(KeyboardGeometry.rowSpacing(95) == 95);
        check(KeyboardGeometry.layoutKeySpacing(-1, KeyboardLayout.ZHUYIN_LAYOUT) == 40);
        check(KeyboardGeometry.layoutKeySpacing(35, KeyboardLayout.ZHUYIN_LAYOUT) == 35);
        check(KeyboardGeometry.layoutKeySpacing(-1, KeyboardLayout.STANDARD_TOUCH_LAYOUT) == 60);
        check(KeyboardGeometry.layoutKeySpacing(61, KeyboardLayout.KOREAN_LAYOUT) == 60);
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
        // 回车让给中/英的那一段：左侧整段外边距（9px）加键帽左侧 [0, yield)；不让时（0）哪里都不算。
        check(KeyboardGapPolicy.yieldsToLeft(0f, 9, 18f));
        check(KeyboardGapPolicy.yieldsToLeft(17.9f, 9, 18f));
        check(!KeyboardGapPolicy.yieldsToLeft(18f, 9, 18f));
        check(KeyboardGapPolicy.yieldsToLeft(-1f, 9, 18f));
        check(KeyboardGapPolicy.yieldsToLeft(-9f, 9, 18f));
        check(!KeyboardGapPolicy.yieldsToLeft(-9.5f, 9, 18f));
        check(!KeyboardGapPolicy.yieldsToLeft(0f, 9, 0f));
        // 挪进键帽时离边缘留 1px，键帽里的点不动。
        check(KeyboardGapPolicy.inside(-4, 100) == 1f);
        check(KeyboardGapPolicy.inside(104, 100) == 99f);
        check(KeyboardGapPolicy.inside(37.5f, 100) == 37.5f);
        // 键盘文字随系统字体调小照样跟随，调大时封顶：国产机出厂的「大」「超大」字体会把固定高度的键帽撑爆。
        check(KeyboardGeometry.keyboardFontScale(0.85f) == 0.85f);
        check(KeyboardGeometry.keyboardFontScale(1f) == 1f);
        check(KeyboardGeometry.keyboardFontScale(1.3f) == KeyboardGeometry.MAX_KEYBOARD_FONT_SCALE);
        check(KeyboardGeometry.keyboardFontScale(2f) == KeyboardGeometry.MAX_KEYBOARD_FONT_SCALE);
        check(KeyboardGeometry.keyboardFontScale(0f) == 1f);
        check(KeyboardGeometry.keyboardFontScale(Float.NaN) == 1f);
        check(KeyboardGeometry.keyboardFontScale(Float.POSITIVE_INFINITY) == 1f);
        System.out.println("Android keyboard geometry: Apple defaults, bounds and precision passed");
    }
}
