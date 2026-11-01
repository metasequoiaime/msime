import app.msime.android.SuperscriptShortcutPolicy;

public final class SuperscriptShortcutPolicySmoke {
    public static void main(String[] args) {
        check(SuperscriptShortcutPolicy.applies("!")
            && SuperscriptShortcutPolicy.applies("！"), "both punctuation faces use the shortcut");
        check(!SuperscriptShortcutPolicy.applies("?"), "question mark keeps its normal action");
        check(SuperscriptShortcutPolicy.swipedUp(100, 80), "an upward move past the threshold triggers");
        check(!SuperscriptShortcutPolicy.swipedUp(100, 90), "a short move does not trigger");
        check(SuperscriptShortcutPolicy.variants().equals(java.util.List.of("⁰", "₀")),
            "long press lists superscript before subscript");
        check(SuperscriptShortcutPolicy.ZERO.equals("0"), "swipe output is ordinary zero");

        // #10441：拼音九键的 ！ 滑出 0 的方向跟「九键滑动输入数字」，下滑时下滑，与 1–9 键一致。
        check(SuperscriptShortcutPolicy.zeroDirection("down").equals("down"), "down mode swipes zero down");
        check(SuperscriptShortcutPolicy.zeroDirection("up").equals("up"), "up mode keeps the up-swipe");
        check(SuperscriptShortcutPolicy.zeroDirection("off").equals("up"), "off keeps the original up-swipe");
        check(SuperscriptShortcutPolicy.zeroDirection("sideways").equals("up"), "unknown modes read as off");
        check(SuperscriptShortcutPolicy.zeroDirection(null).equals("up"), "a missing mode reads as off");
        check(SuperscriptShortcutPolicy.zeroThresholdDp("off", 30) == 14f,
            "off keeps the 14 dp threshold because the distance slider is disabled");
        check(SuperscriptShortcutPolicy.zeroThresholdDp("down", 30) == 30f
            && SuperscriptShortcutPolicy.zeroThresholdDp("up", 30) == 30f, "on modes share the nine-key swipe distance");
        check(SuperscriptShortcutPolicy.swiped("down", 24, 100, 125), "a downward move past the threshold triggers");
        check(!SuperscriptShortcutPolicy.swiped("down", 24, 100, 124), "the threshold is strictly greater than");
        check(!SuperscriptShortcutPolicy.swiped("down", 24, 100, 60), "an up-swipe does not trigger in down mode");
        check(SuperscriptShortcutPolicy.swiped("up", 24, 100, 75)
            && !SuperscriptShortcutPolicy.swiped("up", 24, 100, 140), "up mode only follows upward moves");
        check(SuperscriptShortcutPolicy.description("！", "up").equals("！；上滑输入 0；长按选择上标 ⁰ 或下标 ₀"),
            "the up description is unchanged");
        check(SuperscriptShortcutPolicy.description("！", "down").equals("！；下滑输入 0；长按选择上标 ⁰ 或下标 ₀"),
            "the down description names the down-swipe");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
