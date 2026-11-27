package app.msime.android;

/**
 * 按份额算一行里各格的左右边，供按键行量宽度用。纯计算，不依赖 Android，便于在 JVM 上测试。
 *
 * <p>`LinearLayout` 按权重分宽时，先从这一行的宽度里扣掉各个键的外边距（键距），再把剩下的按权重分给各键。键数不同的行扣掉的键距不同，同样份额的键就不一样宽，各行同一列的键边也对不齐：26 键三排字母差出约 1.5 dp，⌫ 和上面的键也错开。
 *
 * <p>这里先按份额切格，再从每一格里扣掉那个键自己的外边距：第 {@code i} 条边是
 * {@code round(width × 前 i 格份额之和 / 总份额)}，键宽是「格宽减左右外边距」。键距仍是外边距，所以皮肤、按下动画、键位预览和按下统计看到的键还是画出来的那个键；落在空隙里的按下照旧由 {@link KeyboardKeyArea} 交给拥有那段空隙的键，空隙在两个键之间对半分，正好是格的边界。
 *
 * <p>每条边各自取整，不是每个宽度各自取整：份额之和相同、宽度相同的两行，前缀份额相同的边落在同一像素上，与这一行有几个键无关。最后一条边恰好是 {@code width}，舍入误差不会在行尾累积。
 */
public final class KeyCellEdges {
    private KeyCellEdges() {}

    /**
     * 一行各格的边，共 {@code spans.length + 1} 条：第 0 条是 0，最后一条是 {@code width}。
     *
     * @param spans 各格的份额，不能为负；全为 0 时所有边都是 0
     * @param width 这一行可分的宽度（像素，不含内边距）
     */
    public static int[] edges(float[] spans, int width) {
        if (width < 0) throw new IllegalArgumentException("width " + width);
        double total = 0;
        for (float span : spans) {
            if (!(span >= 0) || Float.isInfinite(span)) throw new IllegalArgumentException("span " + span);
            total += span;
        }
        int[] edges = new int[spans.length + 1];
        if (total <= 0) return edges;
        double prefix = 0;
        for (int index = 0; index < spans.length; index++) {
            prefix += spans[index];
            edges[index + 1] = index == spans.length - 1 ? width : (int) Math.round(width * prefix / total);
        }
        return edges;
    }

    /** 第 {@code index} 格里键本身的宽度：格宽减去它的左右外边距，不小于 0。 */
    public static int keyWidth(int[] edges, int index, int leftMargin, int rightMargin) {
        return Math.max(0, edges[index + 1] - edges[index] - leftMargin - rightMargin);
    }
}
