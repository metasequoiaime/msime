package app.msime.android;

/**
 * 一次建键盘时对所有键组都相同的输入：屏幕密度和键盘用的字体缩放（已按 {@link KeyboardTypographyPolicy#keyboardFontScale} 封顶）。字体由量字的 {@link KeyFaceMeasurer} 带着。
 */
public record KeyFaceEnvironment(float density, float fontScale) {
    public KeyFaceEnvironment {
        if (!(density > 0) || !(fontScale > 0)) throw new IllegalArgumentException("bad environment " + density + " " + fontScale);
    }

    /** dp 换像素。 */
    public float px(float dp) { return dp * density; }

    /** 键盘字号（sp）换像素，跟随封顶后的字体缩放。 */
    public float sp(float sp) { return sp * density * fontScale; }
}
