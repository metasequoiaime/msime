package app.msime.android;

/** 图标和自绘标签在大字体、额外字形留白和矮磁贴下都必须完整可见。 */
public final class FunctionTileGeometrySmoke {
    private FunctionTileGeometrySmoke() { }

    public static void main(String[] args) {
        var large = FunctionTileGeometry.layout(52, 52, 30, 7, -13.8f, 3.45f, -16.1f, 4.6f);
        check(large.labelBaseline() + 4.6f * large.scale() <= 52.001f,
            "52 dp 磁贴也会把大字体标签的底部裁掉");
        int checks = 0;
        for (float density : new float[] {1, 2.75f, 3})
            for (float height : new float[] {44, 46.8f, 52, 84})
                for (float fontScale : new float[] {.85f, 1, 1.15f}) {
                    float ascent = -12 * fontScale * density;
                    float descent = 3 * fontScale * density;
                    // 合成字体的墨迹可以越过 ascent/descent，不能把这部分裁掉。
                    float fontTop = -14 * fontScale * density;
                    float fontBottom = 4 * fontScale * density;
                    var layout = FunctionTileGeometry.layout(height * density, 52 * density,
                        30 * density, 7 * density, ascent, descent, fontTop, fontBottom);
                    check(layout.top() >= 0, "内容块不能越过磁贴顶部");
                    check(layout.labelBaseline() + fontBottom * layout.scale() <= height * density + .001f,
                        "大字体标签的底部被裁切");
                    check(layout.labelBaseline() + fontTop * layout.scale()
                            >= layout.top() + 37 * density * layout.scale() - .001f,
                        "标签的顶部不能侵入图标和间距");
                    check(layout.scale() <= 1 && layout.scale() > 0, "只缩小内容，不扩大或消失");
                    checks += 4;
                }
        var ordinary = FunctionTileGeometry.layout(52, 52, 30, 7, -11, 3, -11, 3);
        check(ordinary.scale() == 1 && ordinary.top() == .5f && ordinary.labelBaseline() == 48.5f,
            "本来放得下的常规字体保持原有尺寸和位置");
        System.out.println("FunctionTileGeometrySmoke passed: " + (checks + 2));
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
