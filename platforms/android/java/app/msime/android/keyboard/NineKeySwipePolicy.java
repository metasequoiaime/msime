package app.msime.android;

/**
 * 拼音九键网格键的滑动（#5580）：设置「九键滑动输入数字」（共享偏好 {@link #PREFERENCE}，随账号同步，三端相同，#10229）选了方向时，沿这个方向滑过「九键滑动距离」（Android 本机设置 `platform.android.nine_key_swipe_distance`，默认 {@link #DEFAULT_THRESHOLD_DP} dp；iOS 和鸿蒙固定 24）松手输入键上印的数字；往反方向滑弹出这个键的数字和字母选项，和长按一样；弹出后手指不抬起接着滑到某个选项上松手就输入它，没滑进过选项就松手则弹窗留着等点选（#10443，接线在 `ImeLayoutRows.bindNineKeySwipe`）。这一项原先是 Android 本机设置 `platform.android.nine_key_swipe`，由 {@link KeyLayoutStyleMigration} 迁进共享偏好后删掉，写法 `off`、`up`、`down` 原样不变。
 *
 * <p>这个设置和 26 键的「滑动输入符号」分开、默认关闭：九键原来没有滑动，默认打开会让点按时手指稍有上下偏移的老用户忽然打出数字或弹出选项。26 键的滑动开关默认开启、方向可以选择，而九键需要独立控制上滑出数字、下滑出字母，不能共用同一个设置。
 *
 * <p>阈值也和 26 键分开（#6208）：九键的键比 26 键高得多，快速连打时最后一键手指往上带一下就会超过 26 键的 {@link SwipeHintPolicy#THRESHOLD_DP}（14 dp），被判成上滑出数字；组字时这一下还会先把整串拼音按原始数字上屏。所以九键默认要求滑得更远，并允许用户在 {@link #MIN_THRESHOLD_DP}..{@link #MAX_THRESHOLD_DP} 之间按 {@link #THRESHOLD_STEP_DP} 调整；最小值等于 26 键的阈值，调到最小就是旧版的手感。
 */
public final class NineKeySwipePolicy {
    /** 设置取值：关闭（默认）。另两个取值是 {@link SwipeHintPolicy#UP} 和 {@link SwipeHintPolicy#DOWN}，即输入数字的方向。 */
    public static final String OFF = "off";
    /** 共享偏好的字段名（crates/client-core 的 `Preferences.touch_nine_key_swipe_digits`），缺键时按 {@link #OFF}。 */
    public static final String PREFERENCE = "touch_nine_key_swipe_digits";
    /** 「九键滑动距离」的默认值（dp）。 */
    public static final int DEFAULT_THRESHOLD_DP = 24;
    /** 「九键滑动距离」的下限（dp），与 26 键的 {@link SwipeHintPolicy#THRESHOLD_DP} 相同。 */
    public static final int MIN_THRESHOLD_DP = 14;
    /** 「九键滑动距离」的上限（dp）。 */
    public static final int MAX_THRESHOLD_DP = 48;
    /** 「九键滑动距离」的步长（dp）。 */
    public static final int THRESHOLD_STEP_DP = 2;

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

    /** 把共享偏好里的原值读成 {@link #OFF}、{@link SwipeHintPolicy#UP} 或 {@link SwipeHintPolicy#DOWN}：缺键、不是字符串或不认识的写法（较新版本加的方向）一律按关闭，与 client-core 的宽松读取一致。 */
    public static String mode(String stored) {
        return SwipeHintPolicy.UP.equals(stored) || SwipeHintPolicy.DOWN.equals(stored) ? stored : OFF;
    }

    /**
     * 当前位置构成哪种滑动。
     *
     * @param mode {@link #OFF}、{@link SwipeHintPolicy#UP} 或 {@link SwipeHintPolicy#DOWN}；其他取值按关闭处理
     * @param downYDp 按下时的纵坐标（dp，向下为正）
     * @param currentYDp 当前纵坐标（dp）
     * @param hasLetters 这个键有没有字母（1 键是「@#」，没有字母可弹）
     * @param thresholdDp 滑动判定阈值（dp），严格大于才算；两个方向共用，取自「九键滑动距离」
     */
    public static Gesture gesture(String mode, float downYDp, float currentYDp, boolean hasLetters,
            float thresholdDp) {
        if (!SwipeHintPolicy.UP.equals(mode) && !SwipeHintPolicy.DOWN.equals(mode)) return Gesture.NONE;
        if (SwipeHintPolicy.swiped(mode, thresholdDp, downYDp, currentYDp)) return Gesture.DIGIT;
        String opposite = SwipeHintPolicy.UP.equals(mode) ? SwipeHintPolicy.DOWN : SwipeHintPolicy.UP;
        if (hasLetters && SwipeHintPolicy.swiped(opposite, thresholdDp, downYDp, currentYDp)) return Gesture.LETTERS;
        return Gesture.NONE;
    }
}
