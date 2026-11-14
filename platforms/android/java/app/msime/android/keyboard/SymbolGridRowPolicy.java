package app.msime.android;

/** 符号网格按可见高度分配整数行：保留最小格高，能放下多行时均分剩余空间。 */
public final class SymbolGridRowPolicy {
    private SymbolGridRowPolicy() { }

    /**
     * 返回单个网格格子的内容高度（像素）。格子上下间距由调用方另行加入。
     * 可见区域不足一行时返回最小高度，让外层滚动容器承载溢出内容。
     */
    public static int cellHeight(int visibleHeightPx, int minimumCellHeightPx, int verticalMarginPx) {
        if (minimumCellHeightPx <= 0) return 0;
        if (visibleHeightPx <= 0) return minimumCellHeightPx;
        int rowExtent = minimumCellHeightPx + Math.max(0, verticalMarginPx) * 2;
        int rows = visibleHeightPx / rowExtent;
        if (rows <= 0) return minimumCellHeightPx;
        int candidate = visibleHeightPx / rows - Math.max(0, verticalMarginPx) * 2;
        return Math.max(minimumCellHeightPx, candidate);
    }
}
