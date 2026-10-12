package app.msime.android;

/**
 * 完整候选网格的列，安卓宿主的 {@link CandidateWrapLayout} 与 JVM 冒烟共用；列宽和占列的算法与 iOS（`KeyboardCandidatePanelView`）、鸿蒙（`ExpandedCandidateLayout`）相同。
 *
 * <p>可见宽度按每列至少 {@code minimumColumn}、列间 {@code spacing} 分成等宽的列，普通词占一列，长词按需占多列。一行剩下的列放不下下一个候选时换行，并把这一行最后一格拉宽到行尾：按字宽自由换行时每行右边都会剩一截空白，宽度再大一点就整列空着。最后一行不拉宽，一两个候选不会被撑成一整条。
 */
public final class CandidateGridPolicy {
    private CandidateGridPolicy() {}

    /** 宽 {@code available} 的网格有几列，至少一列。 */
    public static int columns(int available, int minimumColumn, int spacing) {
        if (available < 0 || minimumColumn <= 0 || spacing < 0)
            throw new IllegalArgumentException("Invalid candidate grid dimensions");
        return (int) BoundsPolicy.atLeast(1L, ((long) available + spacing) / ((long) minimumColumn + spacing));
    }

    /** 第 {@code column} 列左边缘的横坐标；{@code column == columns} 时是网格右边缘再加一个间隔，余下的像素摊到各列里，最后一列正好贴住右边。 */
    public static int columnStart(int column, int columns, int available, int spacing) {
        if (columns <= 0 || column < 0 || column > columns || available < 0 || spacing < 0)
            throw new IllegalArgumentException("Invalid candidate grid column");
        return (int) ((long) column * ((long) available + spacing) / columns);
    }

    /** 从 {@code column} 起占 {@code span} 列的格子宽度。 */
    public static int cellWidth(int column, int span, int columns, int available, int spacing) {
        if (span <= 0 || column + span > columns)
            throw new IllegalArgumentException("Invalid candidate grid span");
        return BoundsPolicy.nonNegative(columnStart(column + span, columns, available, spacing) - spacing
            - columnStart(column, columns, available, spacing));
    }

    /** 自然宽度为 {@code width} 的候选占几列：足够放下它，但不超过一行的列数。 */
    public static int span(int width, int columns, int available, int spacing) {
        if (width < 0 || columns <= 0 || available < 0 || spacing < 0)
            throw new IllegalArgumentException("Invalid candidate grid width");
        long pitch = ((long) available + spacing) / columns;
        if (pitch <= 0) return 1;
        long needed = ((long) width + spacing + pitch - 1) / pitch;
        return (int) BoundsPolicy.bounded(needed, 1L, columns);
    }

    /**
     * 按顺序摆放占 {@code spans} 列的格子，返回每格的 {@code {行, 起始列, 实际占列}}。行的分配与 {@link CandidateWrapPolicy#rows} 一致（列数当宽度、间隔为 0）；换行前那一格的实际占列延伸到行尾。
     */
    public static int[][] place(int columns, int[] spans) {
        if (columns <= 0 || spans == null)
            throw new IllegalArgumentException("Invalid candidate grid spans");
        for (int span : spans)
            if (span <= 0 || span > columns) throw new IllegalArgumentException("Invalid candidate grid span");
        int[] rows = CandidateWrapPolicy.rows(columns, 0, spans);
        int[][] cells = new int[spans.length][];
        int column = 0;
        for (int index = 0; index < spans.length; index++) {
            if (index > 0 && rows[index] != rows[index - 1]) column = 0;
            boolean rowEnds = index + 1 < spans.length && rows[index + 1] != rows[index];
            cells[index] = new int[] {rows[index], column, rowEnds ? columns - column : spans[index]};
            column += spans[index];
        }
        return cells;
    }
}
