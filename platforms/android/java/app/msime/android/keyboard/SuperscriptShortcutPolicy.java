package app.msime.android;

import java.util.List;

/** 123 符号页「！」键的数字与上下角标快捷手势。 */
public final class SuperscriptShortcutPolicy {
    public static final String ZERO = "0";
    private static final List<String> VARIANTS = List.of("⁰", "₀");

    private SuperscriptShortcutPolicy() { }

    /** 中文、英文标点页的叹号都提供同一组快捷手势。 */
    public static boolean applies(String face) {
        return "!".equals(face) || "！".equals(face);
    }

    /** 叹号键上滑输入普通数字 0。 */
    public static boolean swipedUp(float downYDp, float currentYDp) {
        return SwipeHintPolicy.swiped(SwipeHintPolicy.UP, downYDp, currentYDp);
    }

    /** 长按弹出的顺序：上标、下标。 */
    public static List<String> variants() { return VARIANTS; }
}
