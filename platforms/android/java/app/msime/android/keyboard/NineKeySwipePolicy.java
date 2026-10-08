package app.msime.android;

/**
 * 拼音九键网格键的滑动（#5580）：和 26 键的「滑动输入符号」同一个开关（`platform.android.swipe_down_symbols`）和方向（`platform.android.swipe_symbols_direction`）。沿设置的方向滑过 {@link SwipeHintPolicy#THRESHOLD_DP} 松手输入键上印的数字；往反方向滑弹出这个键的数字和字母选项，和长按一样。
 *
 * <p>九键的数字印在键的上沿，相当于 26 键右上角的角标，所以两种键盘共用一个方向设置：把方向改成「上滑」，26 键上滑出角标、九键上滑出数字，下滑弹出字母。
 */
public final class NineKeySwipePolicy {
    /** 一次按压在滑动判定上的结果。 */
    public enum Gesture {
        /** 还没滑过阈值，照常是点按或长按。 */
        NONE,
        /** 沿设置方向滑过阈值：松手输入键上的数字。 */
        DIGIT,
        /** 往反方向滑过阈值：弹出数字和字母选项。 */
        LETTERS
    }

    private NineKeySwipePolicy() {}

    /**
     * 当前位置构成哪种滑动。
     *
     * @param enabled 「滑动输入符号」是否开启；关闭时不响应滑动
     * @param direction {@link SwipeHintPolicy#DOWN} 或 {@link SwipeHintPolicy#UP}；其他取值按下滑处理
     * @param downYDp 按下时的纵坐标（dp，向下为正）
     * @param currentYDp 当前纵坐标（dp）
     * @param hasLetters 这个键有没有字母（1 键是「@#」，没有字母可弹）
     */
    public static Gesture gesture(boolean enabled, String direction, float downYDp, float currentYDp,
            boolean hasLetters) {
        if (!enabled) return Gesture.NONE;
        if (SwipeHintPolicy.swiped(direction, downYDp, currentYDp)) return Gesture.DIGIT;
        String opposite = SwipeHintPolicy.UP.equals(direction) ? SwipeHintPolicy.DOWN : SwipeHintPolicy.UP;
        if (hasLetters && SwipeHintPolicy.swiped(opposite, downYDp, currentYDp)) return Gesture.LETTERS;
        return Gesture.NONE;
    }
}
