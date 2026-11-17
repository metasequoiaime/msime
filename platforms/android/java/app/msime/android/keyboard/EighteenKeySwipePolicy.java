package app.msime.android;

/** 14/18 键两字母键横滑限定一个字母（#8930、#11040）；大写是共享网格的精确标记，不是直接上屏。类名沿用最早接入的十八键双拼。 */
public final class EighteenKeySwipePolicy {
    public static final float THRESHOLD_DP = 24;

    private EighteenKeySwipePolicy() {}

    /** 左滑取左字母，右滑取右字母；纵移、抖动和无效键面返回 0。 */
    public static char letter(String letters, float dx, float dy) {
        if (!twoLetters(letters)
                || !Float.isFinite(dx) || !Float.isFinite(dy)
                || Math.abs(dx) <= THRESHOLD_DP || Math.abs(dx) <= 1.5f * Math.abs(dy)) return 0;
        return Character.toUpperCase(letters.charAt(dx < 0 ? 0 : 1));
    }

    /** 这个 14/18 键字母键在方案 `scheme`（视图的 `scheme` 号）下横滑能限定的两个字母：全拼和双拼的两字母键是键上的两个小写字母；单字母键、五笔 14 键和其他方案为 null，只点按。 */
    public static String exactLetters(String keyLetters, int scheme) {
        if (scheme != InputSchemeTraits.QUANPIN && scheme != InputSchemeTraits.SHUANGPIN) return null;
        return twoLetters(keyLetters) ? keyLetters : null;
    }

    private static boolean twoLetters(String letters) {
        return letters != null && letters.length() == 2
            && letters.charAt(0) >= 'a' && letters.charAt(0) <= 'z'
            && letters.charAt(1) >= 'a' && letters.charAt(1) <= 'z';
    }
}
