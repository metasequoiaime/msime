package app.msime.android;

/** 十八键双拼横滑限定一个字母；大写是共享网格的精确标记，不是直接上屏。 */
public final class EighteenKeySwipePolicy {
    public static final float THRESHOLD_DP = 24;

    private EighteenKeySwipePolicy() {}

    /** 左滑取左字母，右滑取右字母；纵移、抖动和无效键面返回 0。 */
    public static char letter(String letters, float dx, float dy) {
        if (letters == null || letters.length() != 2
                || letters.charAt(0) < 'a' || letters.charAt(0) > 'z'
                || letters.charAt(1) < 'a' || letters.charAt(1) > 'z'
                || !Float.isFinite(dx) || !Float.isFinite(dy)
                || Math.abs(dx) <= THRESHOLD_DP || Math.abs(dx) <= 1.5f * Math.abs(dy)) return 0;
        return Character.toUpperCase(letters.charAt(dx < 0 ? 0 : 1));
    }
}
