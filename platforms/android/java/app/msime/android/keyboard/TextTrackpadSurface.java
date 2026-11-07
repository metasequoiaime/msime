package app.msime.android;

/**
 * 文本编辑触控板底面的几何：点阵怎么居中排、提示文字放不放得下。只做算术，绘制在 {@link TextTrackpadView}。
 */
public final class TextTrackpadSurface {
    /** 点阵间距（dp）。 */
    public static final float DOT_SPACING_DP = 16;
    /** 点的半径（dp）。 */
    public static final float DOT_RADIUS_DP = 1;
    /** 提示文字的字号上下限（sp）；放不下下限就不画。 */
    public static final float HINT_MAX_SP = 14;
    public static final float HINT_MIN_SP = 10;
    /** 提示文字左右至少留的空白（dp）。 */
    public static final float HINT_SIDE_DP = 12;

    private TextTrackpadSurface() { }

    /** 一条边上能排几个点：两端各留至少半个间距。 */
    public static int dotCount(float length, float spacing) {
        if (!(spacing > 0) || !(length >= spacing)) return 0;
        return (int) Math.floor(length / spacing);
    }

    /** 第一个点的位置，让整排点在这条边上居中。 */
    public static float firstDot(float length, float spacing) {
        int count = dotCount(length, spacing);
        return count == 0 ? 0 : (length - (count - 1) * spacing) / 2f;
    }

    /**
     * 提示文字用多大的字号：按 {@code widthAtMax}（用上限字号量出来的宽度）等比缩到放得下 {@code available}，
     * 不小于下限；下限也放不下、或高度不够一行时返回 0，表示不画。
     */
    public static float hintSize(float widthAtMax, float available, float lineHeightAtMax, float height,
                                 float maxSize, float minSize) {
        if (!(widthAtMax > 0) || !(available > 0) || !(maxSize > 0)) return 0;
        float size = widthAtMax <= available ? maxSize : maxSize * available / widthAtMax;
        if (size < minSize) return 0;
        if (lineHeightAtMax * size / maxSize > height) return 0;
        return size;
    }
}
