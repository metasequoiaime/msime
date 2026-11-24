package app.msime.android;

/**
 * 量字形。键组只在建组时用它把内容域量一遍（{@link KeyFaceDomain#measure}），之后键格尺寸再怎么变都只做算术：字宽和墨迹随字号线性缩放。
 *
 * <p>宿主用画笔实现；JVM 冒烟用给定数值的假实现，所以排版规则可以脱离字体检查。
 */
public interface KeyFaceMeasurer {
    /** {@code text} 在 {@code sizePx} 字号下的排版宽度（像素）。 */
    float advance(KeyFaceSlot slot, String text, float sizePx);

    /** {@code text} 在 {@code sizePx} 字号下的墨迹框，相对基线原点（{@code top} 在基线以上，为负）。 */
    Ink ink(KeyFaceSlot slot, String text, float sizePx);

    /** 这个位置的字体在 {@code sizePx} 字号下的上行线（字体度量的 ascent，相对基线，为负）：右上角提示按文字框（上行线、排版宽度）定位，与原来的角标一样。 */
    float ascent(KeyFaceSlot slot, float sizePx);

    /** 墨迹框，像素。 */
    record Ink(float left, float top, float right, float bottom) {
        public float width() { return right - left; }

        public float height() { return bottom - top; }
    }
}
