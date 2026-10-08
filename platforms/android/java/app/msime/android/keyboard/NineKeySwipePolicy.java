package app.msime.android;

/**
 * 拼音九键网格键的滑动（#5580）：设置「九键滑动输入数字」（`platform.android.nine_key_swipe`）选了方向时，沿这个方向滑过 {@link SwipeHintPolicy#THRESHOLD_DP} 松手输入键上印的数字；往反方向滑弹出这个键的数字和字母选项，和长按一样。
 *
 * <p>这个设置和 26 键的「滑动输入符号」分开、默认关闭：九键原来没有滑动，默认打开会让点按时手指稍有上下偏移的老用户忽然打出数字或弹出选项。26 键那个开关默认开、方向默认下滑，而 #5580 要的是上滑出数字、下滑出字母，共用它就只能二选一。
 */
public final class NineKeySwipePolicy {
    /** 设置取值：关闭（默认）。另两个取值是 {@link SwipeHintPolicy#UP} 和 {@link SwipeHintPolicy#DOWN}，即输入数字的方向。 */
    public static final String OFF = "off";

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
     * @param mode {@link #OFF}、{@link SwipeHintPolicy#UP} 或 {@link SwipeHintPolicy#DOWN}；其他取值按关闭处理
     * @param downYDp 按下时的纵坐标（dp，向下为正）
     * @param currentYDp 当前纵坐标（dp）
     * @param hasLetters 这个键有没有字母（1 键是「@#」，没有字母可弹）
     */
    public static Gesture gesture(String mode, float downYDp, float currentYDp, boolean hasLetters) {
        if (!SwipeHintPolicy.UP.equals(mode) && !SwipeHintPolicy.DOWN.equals(mode)) return Gesture.NONE;
        if (SwipeHintPolicy.swiped(mode, downYDp, currentYDp)) return Gesture.DIGIT;
        String opposite = SwipeHintPolicy.UP.equals(mode) ? SwipeHintPolicy.DOWN : SwipeHintPolicy.UP;
        if (hasLetters && SwipeHintPolicy.swiped(opposite, downYDp, currentYDp)) return Gesture.LETTERS;
        return Gesture.NONE;
    }
}
