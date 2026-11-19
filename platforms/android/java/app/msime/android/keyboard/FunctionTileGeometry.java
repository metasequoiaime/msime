package app.msime.android;

/** 自绘工具条目的图标、间距和字体必须作为一个完整内容块缩放。 */
public final class FunctionTileGeometry {
    public record Layout(float scale, float top, float labelBaseline) { }

    private FunctionTileGeometry() { }

    public static Layout layout(float height, float nominalHeight, float iconArea, float gap,
            float ascent, float descent, float fontTop, float fontBottom) {
        float labelTop = Math.min(ascent, fontTop);
        float labelBottom = Math.max(descent, fontBottom);
        float content = iconArea + gap + labelBottom - labelTop;
        float full = Math.max(nominalHeight, content);
        float available = height > 0 ? height : full;
        float scale = full <= 0 ? 1f : Math.min(1f, available / full);
        float top = Math.max(0, (available - content * scale) / 2f);
        return new Layout(scale, top, top + (iconArea + gap - labelTop) * scale);
    }
}
