package app.msime.android;

/** 字母键滑动输入提示字符：从按下点沿设置的方向（`platform.android.swipe_symbols_direction`，下滑或上滑）移动超过 14 dp 时，松手输入该键的提示字符（{@link LetterHintTable}），按键预览也改为显示提示字符。长按同样输入提示字符，不受这里的开关和方向影响。 */
public final class SwipeHintPolicy {
    /** 滑动判定阈值（dp），严格大于才算。 */
    public static final float THRESHOLD_DP = 14f;
    /** 方向设置的取值：下滑。 */
    public static final String DOWN = "down";
    /** 方向设置的取值：上滑。 */
    public static final String UP = "up";

    private SwipeHintPolicy() { }

    /**
     * 是否已经沿 `direction` 构成滑动。
     *
     * @param direction {@link #DOWN} 或 {@link #UP}；其它取值按下滑处理
     * @param downYDp 按下时的纵坐标（dp，向下为正）
     * @param currentYDp 当前纵坐标（dp）
     */
    public static boolean swiped(String direction, float downYDp, float currentYDp) {
        float moved = UP.equals(direction) ? downYDp - currentYDp : currentYDp - downYDp;
        return moved > THRESHOLD_DP;
    }

    /**
     * 松手时这个键应输入的文本：沿设置方向滑过阈值且有提示时是提示字符，否则是键本身。
     *
     * @param letter 键的字母
     * @param swipeEnabled 「滑动输入符号」是否开启；关闭时不响应滑动
     * @param direction {@link #DOWN} 或 {@link #UP}
     */
    public static String output(String letter, boolean swipeEnabled, String direction, float downYDp,
            float currentYDp) {
        if (swipeEnabled && swiped(direction, downYDp, currentYDp)) {
            String hint = LetterHintTable.hint(letter);
            if (hint != null) return hint;
        }
        return letter;
    }
}
