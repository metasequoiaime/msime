package app.msime.android;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;

/** 完整候选网格：按 {@link CandidateGridPolicy} 把可见宽度分成等宽的列，每个候选按自然宽度占一列或多列，同一行的格子等高。 */
public final class CandidateWrapLayout extends ViewGroup {
    private final int spacing;
    private final int minimumColumn;
    /** 上一次测量算出的各子视图左上角，onLayout 照此摆放；GONE 的子视图不占位置。 */
    private int[] lefts = new int[0];
    private int[] tops = new int[0];

    public CandidateWrapLayout(Context context, int spacing, int minimumColumn) {
        super(context);
        if (spacing < 0 || minimumColumn <= 0) throw new IllegalArgumentException("Invalid candidate spacing");
        this.spacing = spacing;
        this.minimumColumn = minimumColumn;
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int count = getChildCount();
        int horizontalPadding = getPaddingLeft() + getPaddingRight();
        int widthMode = MeasureSpec.getMode(widthMeasureSpec);
        int limit = widthMode == MeasureSpec.UNSPECIFIED ? Integer.MAX_VALUE
            : BoundsPolicy.nonNegative(MeasureSpec.getSize(widthMeasureSpec) - horizontalPadding);
        int[] visible = new int[count];
        int[] natural = new int[count];
        int visibleCount = 0;
        int widest = 0;
        int naturalSpec = MeasureSpec.makeMeasureSpec(
            widthMode == MeasureSpec.UNSPECIFIED ? 0 : limit, widthMode == MeasureSpec.UNSPECIFIED
                ? MeasureSpec.UNSPECIFIED : MeasureSpec.AT_MOST);
        for (int index = 0; index < count; index++) {
            View child = getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            child.measure(naturalSpec, childHeightSpec(child, heightMeasureSpec));
            natural[visibleCount] = child.getMeasuredWidth();
            widest = BoundsPolicy.atLeast(widest, natural[visibleCount]);
            visible[visibleCount++] = index;
        }
        // 没有宽度约束时（不在网格的正常宿主里）按最宽的候选定宽，排成一列。
        int available = widthMode == MeasureSpec.UNSPECIFIED ? widest : limit;
        int columns = CandidateGridPolicy.columns(available, minimumColumn, spacing);
        int[] spans = new int[visibleCount];
        for (int position = 0; position < visibleCount; position++)
            spans[position] = CandidateGridPolicy.span(natural[position], columns, available, spacing);
        int[][] cells = CandidateGridPolicy.place(columns, spans);
        lefts = new int[count];
        tops = new int[count];
        int y = getPaddingTop();
        int rowStart = 0;
        while (rowStart < visibleCount) {
            int rowEnd = rowStart;
            while (rowEnd < visibleCount && cells[rowEnd][0] == cells[rowStart][0]) rowEnd++;
            int rowHeight = 0;
            for (int position = rowStart; position < rowEnd; position++) {
                View child = getChildAt(visible[position]);
                child.measure(exactly(cellWidth(cells[position], columns, available)),
                    childHeightSpec(child, heightMeasureSpec));
                rowHeight = BoundsPolicy.atLeast(rowHeight, child.getMeasuredHeight());
            }
            for (int position = rowStart; position < rowEnd; position++) {
                View child = getChildAt(visible[position]);
                if (child.getMeasuredHeight() != rowHeight)
                    child.measure(exactly(child.getMeasuredWidth()), exactly(rowHeight));
                lefts[visible[position]] = getPaddingLeft()
                    + CandidateGridPolicy.columnStart(cells[position][1], columns, available, spacing);
                tops[visible[position]] = y;
            }
            y += rowHeight + spacing;
            rowStart = rowEnd;
        }
        int contentHeight = visibleCount == 0 ? 0 : y - spacing - getPaddingTop();
        setMeasuredDimension(resolveSize(horizontalPadding + available, widthMeasureSpec),
            resolveSize(getPaddingTop() + contentHeight + getPaddingBottom(), heightMeasureSpec));
    }

    private int cellWidth(int[] cell, int columns, int available) {
        return CandidateGridPolicy.cellWidth(cell[1], cell[2], columns, available, spacing);
    }

    private int childHeightSpec(View child, int heightMeasureSpec) {
        return getChildMeasureSpec(heightMeasureSpec, getPaddingTop() + getPaddingBottom(),
            child.getLayoutParams().height);
    }

    private static int exactly(int size) {
        return MeasureSpec.makeMeasureSpec(size, MeasureSpec.EXACTLY);
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        for (int index = 0; index < getChildCount() && index < lefts.length; index++) {
            View child = getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            child.layout(lefts[index], tops[index], lefts[index] + child.getMeasuredWidth(),
                tops[index] + child.getMeasuredHeight());
        }
    }

    @Override protected LayoutParams generateDefaultLayoutParams() {
        return new LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT);
    }
}
