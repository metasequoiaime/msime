package app.msime.android;

/** 文本编辑面板行级动作的上滑判定：保留原键点击，只有明显向上且超过阈值才接管。 */
public final class TextEditSwipePolicy {
    /** 与 18 键横滑保持同一数量级，严格超过才算上滑。 */
    public static final float THRESHOLD_DP = 24f;
    /** 纵向位移至少是横向位移的 1.5 倍，避免斜着点击误触。 */
    public static final float VERTICAL_RATIO = 1.5f;

    private TextEditSwipePolicy() { }

    public static boolean swipedUp(float threshold, float downX, float downY,
            float currentX, float currentY) {
        float vertical = downY - currentY;
        float horizontal = Math.abs(currentX - downX);
        return vertical > threshold && vertical > horizontal * VERTICAL_RATIO;
    }

    public static boolean swipedUp(float downX, float downY, float currentX, float currentY) {
        return swipedUp(THRESHOLD_DP, downX, downY, currentX, currentY);
    }
}
