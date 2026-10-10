package app.msime.android;

/**
 * 日语九键按下时的五向提示的几何：一个略大于被按键的方框，压在键上，里面按 3×3 排出中间和左、上、右、下五格（方向序号同 {@link JapaneseNineKeyLayout#direction}）。提示不越出这个方框，所以只盖住被按的键和它四周几 dp，不再像原来那样每格一个键大、整个十字盖住周围 3×3 个键。方框夹在键盘里面，边上的键不会被裁掉。纯计算，不依赖 Android，由 JVM 冒烟覆盖。
 */
public final class JapaneseFlickGuideGeometry {
    private static final int[] COLUMNS = {1, 0, 1, 2, 1};
    private static final int[] ROWS = {1, 1, 0, 1, 2};

    /** 方框和五格，坐标都在键盘覆盖层里，单位为像素。 */
    public record Guide(float left, float top, float width, float height) {
        public float cellWidth() { return width / 3f; }
        public float cellHeight() { return height / 3f; }
        public float right() { return left + width; }
        public float bottom() { return top + height; }

        /** 方向 `direction`（0 中、1 左、2 上、3 右、4 下）那一格的左边。 */
        public float cellLeft(int direction) { return left + COLUMNS[checked(direction)] * cellWidth(); }

        /** 方向 `direction` 那一格的上边。 */
        public float cellTop(int direction) { return top + ROWS[checked(direction)] * cellHeight(); }
    }

    private JapaneseFlickGuideGeometry() {}

    /**
     * 被按的键在覆盖层里占 (`keyLeft`, `keyTop`, `keyWidth`, `keyHeight`)，覆盖层大小 `rootWidth` × `rootHeight`。方框以键为中心，四边各外扩 `outset`；每格至少 `minCell` 见方，键太小时方框按它放大。方框再平移进覆盖层；覆盖层本身比方框还小时贴左上角。
     */
    public static Guide layout(float keyLeft, float keyTop, float keyWidth, float keyHeight,
                               float rootWidth, float rootHeight, float outset, float minCell) {
        if (keyWidth < 0 || keyHeight < 0 || rootWidth < 0 || rootHeight < 0 || outset < 0 || minCell < 0)
            throw new IllegalArgumentException("Flick guide sizes cannot be negative");
        float width = BoundsPolicy.atLeast(keyWidth + 2 * outset, 3 * minCell);
        float height = BoundsPolicy.atLeast(keyHeight + 2 * outset, 3 * minCell);
        float left = keyLeft + keyWidth / 2f - width / 2f;
        float top = keyTop + keyHeight / 2f - height / 2f;
        return new Guide(clamp(left, width, rootWidth), clamp(top, height, rootHeight), width, height);
    }

    private static float clamp(float start, float length, float limit) {
        return BoundsPolicy.bounded(start, 0f, limit - length);
    }

    private static int checked(int direction) {
        if (direction < 0 || direction >= COLUMNS.length)
            throw new IllegalArgumentException("Flick direction must be 0 to 4");
        return direction;
    }
}
