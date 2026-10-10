package app.msime.android;

/**
 * 键距留出的空隙归哪个键：纯几何，不依赖 Android，便于在 JVM 上测试。
 *
 * <p>键距设置是用布局外边距实现的，键帽之间那几 dp 不属于任何按钮，落在那里的按下没有视图接收，整个手势就丢了。{@link KeyboardKeyArea} 用这里的规则把这段空隙交还给外边距所属的那个键。坐标都在键自己的坐标系里：键帽是 `[0, width) x [0, height)`，外边距向四周各扩出对应的宽度。
 */
public final class KeyboardGapPolicy {
    private KeyboardGapPolicy() { }

    /**
     * 点到键帽的距离：在键帽里返回 0，落在这个键的外边距里返回到键帽的直线距离，外边距之外返回 -1。
     */
    public static float gapDistance(float x, float y, int width, int height,
                                    int leftMargin, int topMargin, int rightMargin, int bottomMargin) {
        if (x < -leftMargin || y < -topMargin || x >= width + rightMargin || y >= height + bottomMargin)
            return -1f;
        float dx = x < 0 ? -x : x >= width ? x - width + 1 : 0;
        float dy = y < 0 ? -y : y >= height ? y - height + 1 : 0;
        return (float) Math.sqrt(dx * dx + dy * dy);
    }

    /**
     * 按下落在让给左邻的那一段里：`x` 是让出的键自己的坐标，`leftMargin` 是它左侧的外边距，`yield` 是键帽让出的宽度。这一段是左侧整段外边距（原本按最近的键归它）加上键帽左侧 `yield` 宽，见 {@link KeyboardKeyArea#setYield}。
     */
    public static boolean yieldsToLeft(float x, int leftMargin, float yield) {
        return yield > 0 && x >= -Math.max(0, leftMargin) && x < yield;
    }

    /**
     * 把一个坐标移进键帽内部，离边缘留 1px，保证框架的命中测试（`0 <= v < size`）一定落在键上；移动的距离就是空隙的宽度，所以按下位置几乎不变，空格的拖动光标、日文假名的滑动方向这类按位移计算的手势不受影响。
     */
    public static float inside(float value, int size) {
        return BoundsPolicy.bounded(value, 1f, size - 1f);
    }
}
