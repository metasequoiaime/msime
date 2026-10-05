package app.msime.android;

/** 字母键下滑输入提示字符：从按下点向下移动超过 14 dp 时，松手输入该键的提示字符（{@link LetterHintTable}），按键预览也改为显示提示字符。 */
public final class SwipeDownHintPolicy {
    /** 下滑判定阈值（dp），严格大于才算。 */
    public static final float THRESHOLD_DP = 14f;

    private SwipeDownHintPolicy() { }

    /**
     * 是否已经构成下滑。
     *
     * @param downYDp 按下时的纵坐标（dp，向下为正）
     * @param currentYDp 当前纵坐标（dp）
     */
    public static boolean swiped(float downYDp, float currentYDp) {
        return currentYDp - downYDp > THRESHOLD_DP;
    }

    /**
     * 松手时这个键应输入的文本：下滑且有提示时是提示字符，否则是键本身。
     *
     * @param letter 键的字母
     * @param hintsEnabled 「更多符号提示」是否开启；关闭时不画提示，也不响应下滑
     */
    public static String output(String letter, boolean hintsEnabled, float downYDp,
            float currentYDp) {
        if (hintsEnabled && swiped(downYDp, currentYDp)) {
            String hint = LetterHintTable.hint(letter);
            if (hint != null) return hint;
        }
        return letter;
    }
}
