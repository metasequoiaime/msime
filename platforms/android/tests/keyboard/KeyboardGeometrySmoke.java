import app.msime.android.KeyboardHeightPolicy;
import app.msime.android.KeyboardSpacingPolicy;
import app.msime.android.KeyboardTypographyPolicy;
import app.msime.android.NumberPolicy;
import app.msime.android.BoundsPolicy;
import app.msime.android.KeyboardGapPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardLayout;

public final class KeyboardGeometrySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    /** 某个调整量下三行键（第 0–2 行）和底行各自的高度（dp）。 */
    static void checkRows(int adjustment, int first, int second, int third, int bottom) {
        int keys = KeyboardHeightPolicy.keyRowsAdjustment(adjustment);
        int[] rows = {
            KeyboardHeightPolicy.adjustedRowHeight(KeyboardGeometry.KEY_ROW_HEIGHT_DP, keys, 3, 0),
            KeyboardHeightPolicy.adjustedRowHeight(KeyboardGeometry.KEY_ROW_HEIGHT_DP, keys, 3, 1),
            KeyboardHeightPolicy.adjustedRowHeight(KeyboardGeometry.KEY_ROW_HEIGHT_DP, keys, 3, 2),
            KeyboardGeometry.KEY_ROW_HEIGHT_DP + KeyboardHeightPolicy.bottomRowAdjustment(adjustment),
        };
        check(rows[0] == first && rows[1] == second && rows[2] == third && rows[3] == bottom,
            "rows at " + adjustment + ": " + java.util.Arrays.toString(rows));
    }

    public static void main(String[] args) {
        // 与 crates/client-core 的 default_touch_key_spacing_tenths / default_touch_row_spacing_tenths 同值。
        check(KeyboardSpacingPolicy.keySpacing(-1) == 60);
        check(BoundsPolicy.nonNegative(-1) == 0 && BoundsPolicy.nonNegative(7) == 7);
        check(BoundsPolicy.nonNegative(-1L) == 0L && BoundsPolicy.nonNegative(7L) == 7L);
        check(KeyboardSpacingPolicy.rowSpacing(-1) == 70);
        check(KeyboardGeometry.DESIGN_KEY_GAP_DP == 5 && KeyboardGeometry.DESIGN_ROW_GAP_DP == 8);
        check(KeyboardGeometry.DESIGN_PADDING_TOP_DP == 8
            && KeyboardGeometry.DESIGN_PADDING_HORIZONTAL_DP == 6
            && KeyboardGeometry.DESIGN_PADDING_BOTTOM_DP == 6);
        // 底行与上面的键行同高（#6354、#6472）：四行都是 52 dp 加一份行距，默认行距下键区 4 × 59 = 236 dp，与原来 3 × 63 + 46 = 235 dp 只差 1 dp。
        check(KeyboardGeometry.KEY_ROW_HEIGHT_DP == 52 && KeyboardHeightPolicy.KEYBOARD_ROW_COUNT == 4, "four 52 dp rows");
        check(KeyboardHeightPolicy.KEYBOARD_ROW_COUNT * (KeyboardGeometry.KEY_ROW_HEIGHT_DP
            + KeyboardSpacingPolicy.DEFAULT_ROW_SPACING_TENTHS / 10) == 236, "default key area stays 236 dp");
        // 键盘高度调整由四行均分：底行拿 floorDiv(调整量, 4)，三行键分其余部分，余数给前面的行。
        checkRows(0, 52, 52, 52, 52);       // 100%
        checkRows(-46, 41, 41, 40, 40);     // 75%
        checkRows(55, 66, 66, 66, 65);      // 130%
        checkRows(110, 80, 80, 79, 79);     // 160%
        check(KeyboardHeightPolicy.bottomRowAdjustment(Integer.MIN_VALUE) == 0
            && KeyboardHeightPolicy.keyRowsAdjustment(Integer.MIN_VALUE) == 0, "missing adjustment is zero");
        check(KeyboardHeightPolicy.bottomRowAdjustment(500) == 27 && KeyboardHeightPolicy.keyRowsAdjustment(500) == 83,
            "shares clamp to the design range");
        for (int adjustment = KeyboardHeightPolicy.MIN_DESIGN_HEIGHT_ADJUSTMENT_DP;
                adjustment <= KeyboardHeightPolicy.MAX_DESIGN_HEIGHT_ADJUSTMENT_DP; adjustment++) {
            int bottom = KeyboardGeometry.KEY_ROW_HEIGHT_DP + KeyboardHeightPolicy.bottomRowAdjustment(adjustment);
            int total = bottom;
            int shortest = bottom;
            int tallest = bottom;
            for (int row = 0; row < 3; row++) {
                int height = KeyboardHeightPolicy.adjustedRowHeight(KeyboardGeometry.KEY_ROW_HEIGHT_DP,
                    KeyboardHeightPolicy.keyRowsAdjustment(adjustment), 3, row);
                total += height;
                shortest = Math.min(shortest, height);
                tallest = Math.max(tallest, height);
            }
            // 三行字母加底行、三行高的键块加底行、连底行一起占四行的整块（日语、注音九键），总高都等于四行键高加整份调整。
            int expected = KeyboardGeometry.KEY_ROW_HEIGHT_DP * KeyboardHeightPolicy.KEYBOARD_ROW_COUNT + adjustment;
            check(total == expected, "four rows sum to the whole adjustment at " + adjustment);
            check(KeyboardHeightPolicy.adjustedRowHeight(KeyboardGeometry.KEY_ROW_HEIGHT_DP * 3,
                KeyboardHeightPolicy.keyRowsAdjustment(adjustment), 1, 0) + bottom == expected, "three-row block at " + adjustment);
            check(KeyboardHeightPolicy.adjustedRowHeight(KeyboardGeometry.KEY_ROW_HEIGHT_DP * KeyboardHeightPolicy.KEYBOARD_ROW_COUNT,
                adjustment, 1, 0) == expected, "block with the bottom row at " + adjustment);
            check(tallest - shortest <= 1, "rows differ by at most 1 dp at " + adjustment);
        }
        // 百分比换算 round(184 × (p − 100) / 100)：逐档钉住，往返不变。
        int[] percents = {75, 80, 85, 90, 95, 100, 105, 110, 115, 120, 125, 130, 140, 150, 160};
        int[] adjustments = {-46, -37, -28, -18, -9, 0, 9, 18, 28, 37, 46, 55, 74, 92, 110};
        for (int index = 0; index < percents.length; index++) {
            check(KeyboardHeightPolicy.heightPercentToAdjustment(percents[index]) == adjustments[index]);
            check(KeyboardHeightPolicy.heightAdjustmentToPercent(adjustments[index]) == percents[index]);
        }
        check(KeyboardHeightPolicy.heightPercentToAdjustment(60) == -46);
        check(KeyboardHeightPolicy.heightPercentToAdjustment(200) == 110);
        check(KeyboardHeightPolicy.designHeightAdjustment(Integer.MIN_VALUE) == 0);
        check(KeyboardHeightPolicy.designHeightAdjustment(-60) == -46);
        check(KeyboardHeightPolicy.designHeightAdjustment(60) == 60);
        check(KeyboardHeightPolicy.designHeightAdjustment(200) == 110);
        // #5564：130% 以内照画；更高的部分最多占窗口高度的 14%，但不低于 130% 的 55 dp。
        check(KeyboardHeightPolicy.windowHeightAdjustment(55, 360) == 55);
        check(KeyboardHeightPolicy.windowHeightAdjustment(-46, 360) == -46);
        check(KeyboardHeightPolicy.windowHeightAdjustment(Integer.MIN_VALUE, 800) == 0);
        check(KeyboardHeightPolicy.windowHeightAdjustment(110, 792) == 110, "portrait phone reaches 160%");
        check(KeyboardHeightPolicy.windowHeightAdjustment(110, 700) == 98, "shorter window caps the extra height");
        check(KeyboardHeightPolicy.windowHeightAdjustment(110, 360) == 55, "landscape phone stops at 130%");
        check(KeyboardHeightPolicy.windowHeightAdjustment(110, 0) == 55, "unknown window stops at 130%");
        check(KeyboardHeightPolicy.windowHeightAdjustment(500, 5000) == 110, "never beyond the design range");
        // 行高按新设计的范围钳制：-46 与 110 都要画出来，不再被共享偏好的 -12..48 截住。
        check(KeyboardHeightPolicy.adjustedRowHeight(56, 110, 3, 0) == 93
            && KeyboardHeightPolicy.adjustedRowHeight(56, 110, 3, 1) == 93
            && KeyboardHeightPolicy.adjustedRowHeight(56, 110, 3, 2) == 92);
        check(KeyboardHeightPolicy.adjustedRowHeight(56, -46, 3, 0) == 41);
        check(KeyboardHeightPolicy.adjustedRowHeight(168, 55, 1, 0) == 223);
        check(KeyboardHeightPolicy.designKeyHeight(100) == 46 && KeyboardHeightPolicy.designKeyHeight(127) == 58);
        check(KeyboardHeightPolicy.displayPercent(127).equals("127%"));
        check(KeyboardSpacingPolicy.keySpacing(29) == 30);
        check(KeyboardSpacingPolicy.keySpacing(61) == 60);
        check(KeyboardSpacingPolicy.rowSpacing(39) == 40);
        check(KeyboardSpacingPolicy.rowSpacing(101) == 100);
        check(KeyboardSpacingPolicy.keySpacing(35) == 35);
        check(KeyboardSpacingPolicy.rowSpacing(95) == 95);
        check(KeyboardSpacingPolicy.layoutKeySpacing(-1, KeyboardLayout.ZHUYIN_LAYOUT) == 40);
        check(KeyboardSpacingPolicy.layoutKeySpacing(35, KeyboardLayout.ZHUYIN_LAYOUT) == 35);
        check(KeyboardSpacingPolicy.layoutKeySpacing(-1, KeyboardLayout.STANDARD_TOUCH_LAYOUT) == 60);
        check(KeyboardSpacingPolicy.layoutKeySpacing(61, KeyboardLayout.KOREAN_LAYOUT) == 60);
        check(KeyboardHeightPolicy.heightAdjustment(Integer.MIN_VALUE) == 0);
        check(KeyboardGeometry.CANDIDATE_ROW_HEIGHT_DP == 48);
        check(KeyboardHeightPolicy.heightAdjustment(-13) == -12);
        check(KeyboardHeightPolicy.heightAdjustment(49) == 48);
        check(KeyboardHeightPolicy.heightAdjustment(24) == 24);
        check(KeyboardHeightPolicy.adjustedRowHeight(48, 0, 3, 0) == 48);
        check(KeyboardHeightPolicy.adjustedRowHeight(48, 24, 3, 2) == 56);
        check(KeyboardHeightPolicy.adjustedRowHeight(48, -12, 3, 1) == 44);
        check(KeyboardHeightPolicy.adjustedRowHeight(48, 1, 3, 0) == 49);
        check(KeyboardHeightPolicy.adjustedRowHeight(48, 1, 3, 1) == 48);
        check(KeyboardHeightPolicy.displayHeight(0).equals("0"));
        check(KeyboardHeightPolicy.displayHeight(24).equals("+24"));
        check(KeyboardSpacingPolicy.display(35).equals("3.5"));
        check(KeyboardSpacingPolicy.display(100).equals("10.0"));
        check(KeyboardSpacingPolicy.halfGapPixels(60, 1) == 3);
        check(KeyboardSpacingPolicy.halfGapPixels(35, 2) == 4);
        check(KeyboardSpacingPolicy.halfGapPixels(60, Float.NaN) == 0);
        check(NumberPolicy.strictInt(45.0, -1) == -1);
        check(NumberPolicy.strictLong(45.0, -1) == -1);
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
        check(KeyboardTypographyPolicy.keyboardFontScale(0.85f) == 0.85f);
        check(KeyboardTypographyPolicy.keyboardFontScale(1f) == 1f);
        check(KeyboardTypographyPolicy.keyboardFontScale(1.3f) == KeyboardTypographyPolicy.MAX_KEYBOARD_FONT_SCALE);
        check(KeyboardTypographyPolicy.keyboardFontScale(2f) == KeyboardTypographyPolicy.MAX_KEYBOARD_FONT_SCALE);
        check(KeyboardTypographyPolicy.keyboardFontScale(0f) == 1f);
        check(KeyboardTypographyPolicy.keyboardFontScale(Float.NaN) == 1f);
        check(KeyboardTypographyPolicy.keyboardFontScale(Float.POSITIVE_INFINITY) == 1f);
        System.out.println("Android keyboard geometry: Apple defaults, bounds and precision passed");
    }
}
