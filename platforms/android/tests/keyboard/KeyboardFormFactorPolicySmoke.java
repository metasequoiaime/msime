import app.msime.android.KeyboardFormFactorPolicy;
import app.msime.android.OneHandGutterView;

public final class KeyboardFormFactorPolicySmoke {
    public static void main(String[] args) {
        check(!KeyboardFormFactorPolicy.expanded(599), "a phone stays compact");
        check(KeyboardFormFactorPolicy.expanded(600), "the Android large-screen boundary expands");
        check(!KeyboardFormFactorPolicy.showLargeScreenOptions(599),
            "a phone hides split and physical-keyboard settings");
        check(KeyboardFormFactorPolicy.showLargeScreenOptions(600),
            "a large screen keeps split and physical-keyboard settings");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(411, 915) == 0,
            "rotating a phone does not turn it into a convertible");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(600, 600) == 600,
            "a narrow foldable uses the available large-screen width");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(720, 1280) == 720,
            "a two-in-one gets a centered bounded keyboard");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(720, 0) == 720,
            "an unknown expanded width has a safe bound");
        // #10202：大屏单手模式不再沿用手机的 85%，「侧栏 + 键区」缩到键区 360 dp 加同比例的侧栏。
        int block = KeyboardFormFactorPolicy.oneHandedBlockWidthDp(800, 1280);
        check(block == 424, "a tablet's one-handed block is a phone-sized 424 dp, not " + block);
        check(OneHandGutterView.keysWidth(block) == KeyboardFormFactorPolicy.ONE_HANDED_KEYS_MAX_WIDTH_DP
                && OneHandGutterView.gutterWidth(block) == 64,
            "the tablet block splits into 360 dp of keys and the usual 15% gutter");
        check(KeyboardFormFactorPolicy.oneHandedBlockWidthDp(411, 915) == 0,
            "a phone keeps the full-width 85/15 split, even in landscape");
        check(KeyboardFormFactorPolicy.oneHandedBlockWidthDp(600, 400) == 400,
            "a window narrower than the block gives it the whole row");
        check(KeyboardFormFactorPolicy.oneHandedBlockWidthDp(800, 0) == 424,
            "an unknown width still gets the phone-sized block");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(800, 1280, false, true) == 0,
            "a one-handed tablet keyboard fills the window so the block can reach the screen edge");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(800, 1280, false, false) == 720,
            "with one-handed off the tablet surface keeps its 720 dp cap");
        check(KeyboardFormFactorPolicy.surfaceWidthDp(411, 915, false, true) == 0,
            "a one-handed phone fills either way");
        // #10203：大屏单手空出来那一侧的侧边数字键盘，宽度取空出来的宽度，150–300 dp。
        check(KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(411, 915) == 0,
            "a phone has no side pad, even in landscape");
        check(KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(600, 600) == 176,
            "the smallest tablet gives the side pad the 176 dp left beside the block");
        check(KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(800, 800) == 300,
            "a portrait tablet caps the side pad at 300 dp");
        check(KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(800, 1280) == 300,
            "a landscape tablet keeps the 300 dp cap and leaves the rest blank");
        check(KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(800, 0) == 0,
            "an unknown width draws no side pad");
        check(KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(600, 573) == 0
                && KeyboardFormFactorPolicy.oneHandedSidePadWidthDp(600, 574) == 150,
            "less than 150 dp beside the block draws no side pad");
        System.out.println("Android keyboard form factors: handset fill, bounded large-screen surface, phone-sized one-handed block and side pad passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
