package app.msime.android;

import java.util.List;

/** 叹号键的数字与上下角标快捷手势：拼音九键字母层右列（及右侧大回车接到符号栏末尾）的 ！，以及 123 / #+= 符号页的 ！。 */
public final class SuperscriptShortcutPolicy {
    public static final String ZERO = "0";
    private static final List<String> VARIANTS = List.of("⁰", "₀");

    private SuperscriptShortcutPolicy() { }

    /** 中文、英文标点页的叹号都提供同一组快捷手势。 */
    public static boolean applies(String face) {
        return "!".equals(face) || "！".equals(face);
    }

    /** 123 / #+= 符号页的叹号上滑输入普通数字 0，阈值与 26 键滑动相同。 */
    public static boolean swipedUp(float downYDp, float currentYDp) {
        return SwipeHintPolicy.swiped(SwipeHintPolicy.UP, downYDp, currentYDp);
    }

    /**
     * 拼音九键 ！ 滑出 0 的方向（#10441）：跟「九键滑动输入数字」（{@link NineKeySwipePolicy#mode}）走，选了下滑时下滑输入 0，与 1–9 键一致；上滑或关闭时仍是原来的上滑。
     *
     * @param mode {@link NineKeySwipePolicy#OFF}、{@link SwipeHintPolicy#UP} 或 {@link SwipeHintPolicy#DOWN}；其他取值按关闭处理
     */
    public static String zeroDirection(String mode) {
        return SwipeHintPolicy.DOWN.equals(mode) ? SwipeHintPolicy.DOWN : SwipeHintPolicy.UP;
    }

    /**
     * 拼音九键 ！ 滑出 0 的阈值（dp）：「九键滑动输入数字」开着时与 1–9 键共用「九键滑动距离」，同一排键的手感一致；关闭时那个滑块置灰、用户调不了，保持原来的 {@link SwipeHintPolicy#THRESHOLD_DP}。
     *
     * @param mode 同 {@link #zeroDirection(String)}
     * @param nineKeyDistanceDp 「九键滑动距离」本机设置（dp）
     */
    public static float zeroThresholdDp(String mode, float nineKeyDistanceDp) {
        return NineKeySwipePolicy.OFF.equals(NineKeySwipePolicy.mode(mode)) ? SwipeHintPolicy.THRESHOLD_DP
            : nineKeyDistanceDp;
    }

    /** 是否已沿 {@code direction} 滑过 {@code thresholdDp}（严格大于）；坐标都是 dp，向下为正。 */
    public static boolean swiped(String direction, float thresholdDp, float downYDp, float currentYDp) {
        return SwipeHintPolicy.swiped(direction, thresholdDp, downYDp, currentYDp);
    }

    /** 读屏描述：键面、滑出 0 的方向和长按选项。 */
    public static String description(String face, String direction) {
        return face + "；" + (SwipeHintPolicy.DOWN.equals(direction) ? "下滑" : "上滑") + "输入 0；长按选择上标 ⁰ 或下标 ₀";
    }

    /** 长按弹出的顺序：上标、下标。 */
    public static List<String> variants() { return VARIANTS; }
}
