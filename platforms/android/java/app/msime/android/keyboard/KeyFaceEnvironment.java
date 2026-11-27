package app.msime.android;

/**
 * 一次建键盘时对所有键组都相同的输入：屏幕密度、键盘用的字体缩放（已按 {@link KeyboardTypographyPolicy#keyboardFontScale} 封顶），以及皮肤的键帽轮廓（{@link KeyOutline}，墨迹要落在里面）。字体由量字的 {@link KeyFaceMeasurer} 带着。
 */
public record KeyFaceEnvironment(float density, float fontScale, KeyOutline outline) {
    public KeyFaceEnvironment {
        if (!(density > 0) || !(fontScale > 0) || outline == null)
            throw new IllegalArgumentException("bad environment " + density + " " + fontScale + " " + outline);
    }

    /** 没有键帽轮廓：只受键格限制。 */
    public KeyFaceEnvironment(float density, float fontScale) {
        this(density, fontScale, KeyOutline.NONE);
    }

    /** dp 换像素。 */
    public float px(float dp) { return dp * density; }

    /** 键盘字号（sp）换像素，跟随封顶后的字体缩放。 */
    public float sp(float sp) { return sp * density * fontScale; }
}
