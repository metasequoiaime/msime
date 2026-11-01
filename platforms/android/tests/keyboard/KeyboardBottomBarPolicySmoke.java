package app.msime.android;

/**
 * 键盘底栏的纯逻辑：什么时候画（设置 × 导航栏高度 × 浮动 × 外接键盘 × 手机横屏），以及底栏下面让出多少（导航栏落在窗口里的那截加上手势区高出导航栏的部分）。
 */
public final class KeyboardBottomBarPolicySmoke {
    public static void main(String[] args) {
        navigationHeight();
        otherStates();
        bottomMargin();
        bottomPadding();
        System.out.println("KeyboardBottomBarPolicySmoke ok");
    }

    private static void navigationHeight() {
        // 手势条（含隐藏手势条的 0）画；三键导航的 48 dp 不画。AOSP 模拟器手势导航下给输入法的导航栏是 63 px（24 dp）。
        for (float dp : new float[] {0f, 13.5f, 16f, 24f, 31.9f}) {
            check(KeyboardBottomBarPolicy.shown(true, false, true, false, false, false, dp), "thin bar " + dp + " dp");
        }
        for (float dp : new float[] {32f, 42f, 48f, 56f}) {
            check(!KeyboardBottomBarPolicy.shown(true, false, true, false, false, false, dp), "tall bar " + dp + " dp");
        }
        for (float dp : new float[] {Float.NaN, Float.POSITIVE_INFINITY, -1f}) {
            check(!KeyboardBottomBarPolicy.shown(true, false, true, false, false, false, dp), "invalid height " + dp);
        }
    }

    private static void otherStates() {
        check(!KeyboardBottomBarPolicy.shown(false, false, true, false, false, false, 16f), "off when the setting is off");
        check(!KeyboardBottomBarPolicy.shown(true, false, false, false, false, false, 0f),
            "not drawn when the navigation height cannot be read (Android 11 and earlier)");
        check(!KeyboardBottomBarPolicy.shown(true, true, true, false, false, false, 16f),
            "not when the system draws its own IME buttons on the gesture strip (Android 13+ AOSP)");
        check(!KeyboardBottomBarPolicy.shown(true, false, true, true, false, false, 16f), "not under a floating keyboard");
        check(!KeyboardBottomBarPolicy.shown(true, false, true, false, true, false, 16f),
            "not while a hardware keyboard collapses the keys");
        check(!KeyboardBottomBarPolicy.shown(true, false, true, false, false, true, 16f), "not on a phone in landscape");
    }

    private static void bottomMargin() {
        // Android 15 edge-to-edge：导航栏整截在窗口里，手势区与导航栏一样高。
        check(KeyboardBottomBarPolicy.barBottomPx(48, 48, 48) == 48, "inset only");
        // 手势区比导航栏高：再让出高出的部分。
        check(KeyboardBottomBarPolicy.barBottomPx(48, 48, 80) == 80, "gesture area above the bar");
        // Android 14 及以前窗口停在导航栏上方：导航栏不在窗口里，只让出手势区高出导航栏的部分。
        check(KeyboardBottomBarPolicy.barBottomPx(0, 48, 48) == 0, "window above the navigation bar");
        check(KeyboardBottomBarPolicy.barBottomPx(0, 48, 72) == 24, "window above, taller gesture area");
        // 隐藏手势条：导航栏 0，手势区仍在。
        check(KeyboardBottomBarPolicy.barBottomPx(0, 0, 60) == 60, "hidden gesture handle");
        // 部分厂商在 WindowMetrics 报告无导航栏时仍把整块底部区域放进 view inset；不能因此把底栏抬离屏幕留出一大片空白。
        check(KeyboardBottomBarPolicy.barBottomPx(4, 0, 0) == 0,
            "only known navigation contributes while the bar is shown");
        check(KeyboardBottomBarPolicy.barBottomPx(144, 48, 80) == 80,
            "extra system-bars inset is capped without losing gesture clearance");
        check(KeyboardBottomBarPolicy.barBottomPx(144, 0, 0) == 0,
            "an oversized OEM inset with no reported system bar is ignored");
        check(KeyboardBottomBarPolicy.barBottomPx(-5, -5, -5) == 0, "negative values clamp to zero");
    }

    private static void bottomPadding() {
        // #6392：开了就垫，不看导航方式；关着（默认）时什么都不变。
        check(KeyboardBottomBarPolicy.paddingShown(true, false, false, false, false), "padding when enabled");
        check(!KeyboardBottomBarPolicy.paddingShown(false, false, false, false, false), "off by default leaves the keyboard as is");
        // 底栏画着时不叠加：底栏已经把键区抬高了同样的距离。
        check(!KeyboardBottomBarPolicy.paddingShown(true, true, false, false, false), "not stacked under the bottom bar");
        check(!KeyboardBottomBarPolicy.paddingShown(true, false, true, false, false), "not under a floating keyboard");
        check(!KeyboardBottomBarPolicy.paddingShown(true, false, false, true, false),
            "not while a hardware keyboard collapses the keys");
        check(!KeyboardBottomBarPolicy.paddingShown(true, false, false, false, true), "not on a phone in landscape");
        // 设置页滑块的步进要整除取值范围，两端都能选到（#10447）。
        check((KeyboardBottomBarPolicy.MAX_PADDING_DP - KeyboardBottomBarPolicy.MIN_PADDING_DP)
            % KeyboardBottomBarPolicy.PADDING_STEP_DP == 0, "slider step divides the padding range");
        for (int height : new int[] {0, 40, 80, 160}) {
            check(KeyboardBottomBarPolicy.extraPaddingDp(true, height, false, false, false, false) == height,
                "independent target without bottom bar");
            check(KeyboardBottomBarPolicy.extraPaddingDp(true, height, true, false, false, false)
                    == Math.max(0, height - 40), "bottom bar counts toward target");
            check(KeyboardBottomBarPolicy.extraPaddingDp(false, height, false, false, false, false) == 0,
                "disabled padding preserves saved target");
            check(KeyboardBottomBarPolicy.extraPaddingDp(true, height, false, true, false, false) == 0,
                "floating suppresses custom target");
            check(KeyboardBottomBarPolicy.extraPaddingDp(true, height, false, false, true, false) == 0,
                "collapsed keys suppress custom target");
            check(KeyboardBottomBarPolicy.extraPaddingDp(true, height, false, false, false, true) == 0,
                "phone landscape suppresses custom target");
        }
        check(KeyboardBottomBarPolicy.extraPaddingDp(true, -1, false, false, false, false) == 0,
            "negative target clamps");
        check(KeyboardBottomBarPolicy.extraPaddingDp(true, 161, false, false, false, false) == 160,
            "oversized target clamps");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
