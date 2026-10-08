import app.msime.android.FloatingKeyboardPolicy;

public final class FloatingKeyboardPolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // 外接键盘的候选条模式里不浮动，候选条停在底部。
        check(FloatingKeyboardPolicy.active(true, false), "floating while touch typing");
        check(!FloatingKeyboardPolicy.active(true, true), "candidate bar stays docked");
        check(!FloatingKeyboardPolicy.active(false, false), "switch off");

        // 宽度：窗口的 80%，夹在 240–480 dp，且不超过窗口本身。
        check(FloatingKeyboardPolicy.widthDp(360) == 288, "phone portrait");
        check(FloatingKeyboardPolicy.widthDp(800) == 480, "phone landscape and tablets stop at 480");
        check(FloatingKeyboardPolicy.widthDp(280) == 240, "narrow split-screen window keeps 240");
        check(FloatingKeyboardPolicy.widthDp(200) == 200, "never wider than the window");
        check(FloatingKeyboardPolicy.widthDp(0) == FloatingKeyboardPolicy.MIN_WIDTH_DP, "unknown width");

        // 千分比与像素互换：两端、居中、放不下时贴在起点。
        check(FloatingKeyboardPolicy.offset(0, 600) == 0 && FloatingKeyboardPolicy.offset(1000, 600) == 600,
            "both ends");
        check(FloatingKeyboardPolicy.offset(500, 601) == 301, "rounded middle");
        check(FloatingKeyboardPolicy.offset(1500, 600) == 600 && FloatingKeyboardPolicy.offset(-5, 600) == 0,
            "fractions are clamped");
        check(FloatingKeyboardPolicy.offset(700, 0) == 0 && FloatingKeyboardPolicy.offset(700, -40) == 0,
            "a panel that does not fit sits at the start");
        check(FloatingKeyboardPolicy.fractionOf(300f, 600) == 500, "offset back to a fraction");
        check(FloatingKeyboardPolicy.fractionOf(900f, 600) == 1000 && FloatingKeyboardPolicy.fractionOf(-1f, 600) == 0,
            "fraction clamped");
        check(FloatingKeyboardPolicy.fractionOf(10f, 0) == 500 && FloatingKeyboardPolicy.fractionOf(Float.NaN, 600) == 500,
            "unknown range or offset centres");
        for (int fraction : new int[] {0, 250, 500, 999, 1000}) {
            check(FloatingKeyboardPolicy.fractionOf(FloatingKeyboardPolicy.offset(fraction, 1000), 1000) == fraction,
                "round trip " + fraction);
        }

        // 拖动：起点加位移后钳在可移动范围里。
        check(FloatingKeyboardPolicy.dragged(100f, 50f, 600) == 150f, "drag moves");
        check(FloatingKeyboardPolicy.dragged(100f, -500f, 600) == 0f, "drag stops at the top / left");
        check(FloatingKeyboardPolicy.dragged(100f, 900f, 600) == 600f, "drag stops at the bottom / right");
        check(FloatingKeyboardPolicy.dragged(100f, Float.NaN, 600) == 100f, "non-finite delta stays");
        check(FloatingKeyboardPolicy.dragged(100f, 10f, -20) == 0f, "no room pins the panel");
        check(FloatingKeyboardPolicy.DEFAULT_X_FRACTION == 500
            && FloatingKeyboardPolicy.DEFAULT_Y_FRACTION == FloatingKeyboardPolicy.MAX_FRACTION,
            "starts centred at the bottom");
        System.out.println("Android floating keyboard: activation, width, position and drag passed");
    }
}
