package app.msime.android;

/** 字母键滑动输入提示字符：从按下点沿设置的方向（`platform.android.swipe_symbols_direction`，下滑或上滑）移动超过 14 dp 时，松手输入该键的提示字符（{@link LetterHintTable}），按键预览也改为显示右上角的键面（中文标点生效时 ( ) " ? ! 显示中文形式，见 {@link LetterHintTable#face}）。长按同样输入提示字符，不受这里的开关和方向影响。 */
public final class SwipeHintPolicy {
    /** 滑动判定阈值（dp），严格大于才算。 */
    public static final float THRESHOLD_DP = 14f;
    /** 方向设置的取值：下滑。 */
    public static final String DOWN = "down";
    /** 方向设置的取值：上滑。 */
    public static final String UP = "up";

    private SwipeHintPolicy() { }

    /** 26 键滑动输入符号是否应接管本次触摸；候选栏显示时保留普通点按选词。 */
    public static boolean enabled(boolean swipeEnabled, boolean candidateVisible) {
        return swipeEnabled && !candidateVisible;
    }

    /**
     * 26 键中文字母键这次触摸是否把长按和滑动都当成普通点按：组字中候选栏可见时是，免得选词时误出符号或字母选项。上屏后联想（`View.prediction`）也让候选栏可见，但它不算组字，长按和滑动照常输入键角字符，和空闲时一样；联想由运行时在下一个键时清掉（#10611）。
     *
     * @param candidateVisible 候选栏此刻可见，且当前方案的长按会打开中文字母选项
     * @param prediction 当前视图是上屏后联想
     */
    public static boolean tapsOnly(boolean candidateVisible, boolean prediction) {
        return candidateVisible && !prediction;
    }

    /**
     * 是否已经沿 `direction` 构成滑动。
     *
     * @param direction {@link #DOWN} 或 {@link #UP}；其它取值按下滑处理
     * @param downYDp 按下时的纵坐标（dp，向下为正）
     * @param currentYDp 当前纵坐标（dp）
     */
    public static boolean swiped(String direction, float downYDp, float currentYDp) {
        return swiped(direction, THRESHOLD_DP, downYDp, currentYDp);
    }

    /**
     * 同 {@link #swiped(String, float, float)}，阈值由调用方给出：拼音九键的滑动用自己的「九键滑动距离」（{@link NineKeySwipePolicy}），26 键仍用 {@link #THRESHOLD_DP}。
     *
     * @param thresholdDp 滑动判定阈值（dp），严格大于才算
     */
    public static boolean swiped(String direction, float thresholdDp, float downYDp, float currentYDp) {
        float moved = UP.equals(direction) ? downYDp - currentYDp : currentYDp - downYDp;
        return moved > thresholdDp;
    }

    /**
     * 松手时这个键应输入的文本：沿设置方向滑过阈值且有提示时是规范表里的提示字符，否则是键本身。中文标点生效时，( ) " ? ! 这几个提示实际上屏的是 Engine 转写的中文标点，那一步由 `ImeLetterRows` 交给 Engine 的标点路由（{@link LetterHintTable#routesThroughEngine}），这里仍返回 ASCII 原字符。
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
