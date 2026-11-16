package app.msime.android;

/** 空闲工具栏的排法：品牌键在行首，其余可见工具和收起键按固定格宽、固定间距聚到行尾，与 iOS 的工具栏相同（40 的格子、8 的间距）。间距不随可见工具的数量和工具栏尺寸变化。 */
public final class ToolbarClusterPolicy {
    /** 工具栏尺寸为 100% 时一个工具的格宽，也是工具的最小点击宽度。 */
    public static final int CELL_DP = 40;
    /** 相邻两个工具之间的间距。 */
    public static final int GAP_DP = 8;
    /** 滚动区里排在工具前面的固定子视图：算式结果胶囊、可伸缩空白。 */
    public static final int LEADING_CHILDREN = 2;

    private ToolbarClusterPolicy() {}

    /** 最后一个工具和收起键之间的间距。收起键（{@code edgeWidth}）比工具的格子（{@code cellWidth}）宽，图标各自居中，所以从 {@code gap} 里扣掉收起键多出来的那一半，两个图标的中心距就和工具之间相同；不会小于 0。三个参数单位相同。 */
    public static int edgeGap(int gap, int edgeWidth, int cellWidth) {
        return Math.max(0, gap - Math.max(0, edgeWidth - cellWidth) / 2);
    }

    /** 可见工具排成一串的宽度：每个工具前面留一个间距。结果胶囊不计。 */
    public static int clusterWidth(int visibleTools, int cellWidth, int gap) {
        return Math.max(0, visibleTools) * (cellWidth + gap);
    }

    /** 工具前面可伸缩空白的宽度：视口（{@code viewport}）放下结果胶囊（{@code chip}，隐藏时为 0）和全部可见工具后余下的宽度，工具因此靠行尾；放不下时为 0，内容改为横向滚动。 */
    public static int leadingSpace(int viewport, int chip, int visibleTools, int cellWidth, int gap) {
        return Math.max(0, viewport - chip - clusterWidth(visibleTools, cellWidth, gap));
    }

    /** 滚动区能滚动的最大距离；为 0 时全部工具都在视口内。滚动起始位置总是 0，那里露出结果胶囊和用户排在最前面的工具。 */
    public static int scrollRange(int viewport, int chip, int visibleTools, int cellWidth, int gap) {
        return Math.max(0, chip + clusterWidth(visibleTools, cellWidth, gap) - viewport);
    }

    /** 第 {@code index} 个可见工具（从 0 起）在滚动位置 0 时离视口起始边的距离。 */
    public static int toolOffset(int index, int viewport, int chip, int visibleTools, int cellWidth, int gap) {
        return chip + leadingSpace(viewport, chip, visibleTools, cellWidth, gap) + gap + index * (cellWidth + gap);
    }
}
