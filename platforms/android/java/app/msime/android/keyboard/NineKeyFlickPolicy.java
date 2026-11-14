package app.msime.android;

/** 拼音九键字母层的三向 flick：上、左、右依次输入这一键的前三个字母。 */
public final class NineKeyFlickPolicy {
    public static final int DEFAULT_THRESHOLD_DP = 24;
    public static final int MIN_THRESHOLD_DP = 14;
    public static final int MAX_THRESHOLD_DP = 48;
    public static final int THRESHOLD_STEP_DP = 2;
    public static final String PREFERENCE = "touch_nine_key_flick_threshold";

    public enum Letter { NONE, FIRST, SECOND, THIRD }

    private NineKeyFlickPolicy() {}

    /** 返回方向对应的字母序号；下滑和字母不足的方向没有动作。 */
    public static Letter letter(float downX, float downY, float currentX, float currentY,
            String letters, float thresholdDp) {
        if (letters == null || letters.isEmpty() || !Float.isFinite(downX) || !Float.isFinite(downY)
                || !Float.isFinite(currentX) || !Float.isFinite(currentY) || !Float.isFinite(thresholdDp)
                || thresholdDp < 0) return Letter.NONE;
        float dx = currentX - downX;
        float dy = currentY - downY;
        float horizontal = Math.abs(dx);
        float vertical = Math.abs(dy);
        if (Math.max(horizontal, vertical) <= thresholdDp) return Letter.NONE;
        if (vertical >= horizontal) return dy < 0 ? pick(letters, 0) : Letter.NONE;
        return dx < 0 ? pick(letters, 1) : pick(letters, 2);
    }

    private static Letter pick(String letters, int index) {
        if (index >= letters.length()) return Letter.NONE;
        return switch (index) {
            case 0 -> Letter.FIRST;
            case 1 -> Letter.SECOND;
            default -> Letter.THIRD;
        };
    }

    public static String character(String letters, Letter letter) {
        if (letters == null || letter == Letter.NONE) return "";
        int index = switch (letter) {
            case FIRST -> 0;
            case SECOND -> 1;
            case THIRD -> 2;
            default -> -1;
        };
        return index >= 0 && index < letters.length() ? String.valueOf(letters.charAt(index)) : "";
    }
}
