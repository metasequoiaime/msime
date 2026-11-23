package app.msime.android;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;

/**
 * 按键的一行（或九键外框这类横向的一块）：各键按权重切格（{@link KeyCellEdges}），键宽是格宽减去它自己的外边距。
 *
 * <p>普通 `LinearLayout` 先从整行扣掉所有键的外边距再按权重分，键数不同的行同样权重的键不一样宽，各行同一列的边对不齐。这里先让 `LinearLayout` 照常量一遍（高度、对齐方式都由它定），再按格把每个子视图的宽度重新量成「格宽减左右外边距」，摆放时把它放回自己的格里。外边距（键距）和以前一样画在键外，皮肤、按下动画、键位预览、按下统计看到的仍是画出来的那个键。
 *
 * <p>只有这一行可见的子视图都带正的权重时才按格量；有固定宽度的子视图时整行照 `LinearLayout` 原样排。`GONE` 的子视图份额为 0。横向的 {@link KeyboardKeyArea}（底部功能行）用同一套量法（{@link #measureCells}、{@link #layoutCells}）。
 */
public final class KeyRowLayout extends LinearLayout {
    public KeyRowLayout(Context context) {
        super(context);
        setOrientation(HORIZONTAL);
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        super.onMeasure(widthMeasureSpec, heightMeasureSpec);
        measureCells(this);
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        super.onLayout(changed, left, top, right, bottom);
        layoutCells(this);
    }

    /** 各子视图的份额；不能按格量（不是横向、有子视图没有正的权重）时返回 null。 */
    private static float[] spans(LinearLayout row) {
        if (row.getOrientation() != HORIZONTAL) return null;
        int count = row.getChildCount();
        float[] spans = new float[count];
        boolean any = false;
        for (int index = 0; index < count; index++) {
            View child = row.getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            if (!(child.getLayoutParams() instanceof LinearLayout.LayoutParams params) || !(params.weight > 0))
                return null;
            spans[index] = params.weight;
            any = true;
        }
        return any ? spans : null;
    }

    private static int innerWidth(LinearLayout row, int width) {
        return Math.max(0, width - row.getPaddingLeft() - row.getPaddingRight());
    }

    /** 在 `super.onMeasure` 之后调用：把每个子视图的宽度重新量成它那一格减去左右外边距，高度不变。 */
    static void measureCells(LinearLayout row) {
        float[] spans = spans(row);
        if (spans == null) return;
        int[] edges = KeyCellEdges.edges(spans, innerWidth(row, row.getMeasuredWidth()));
        for (int index = 0; index < spans.length; index++) {
            View child = row.getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            ViewGroup.MarginLayoutParams params = (ViewGroup.MarginLayoutParams) child.getLayoutParams();
            int width = KeyCellEdges.keyWidth(edges, index, params.leftMargin, params.rightMargin);
            if (child.getMeasuredWidth() == width) continue;
            child.measure(MeasureSpec.makeMeasureSpec(width, MeasureSpec.EXACTLY),
                MeasureSpec.makeMeasureSpec(child.getMeasuredHeight(), MeasureSpec.EXACTLY));
        }
    }

    /** 在 `super.onLayout` 之后调用：把每个子视图横向放回自己的格里（格的左边加左外边距），纵向位置不变。从右到左排版时格从右边数起。 */
    static void layoutCells(LinearLayout row) {
        float[] spans = spans(row);
        if (spans == null) return;
        int inner = innerWidth(row, row.getWidth());
        int[] edges = KeyCellEdges.edges(spans, inner);
        boolean rtl = row.getLayoutDirection() == LAYOUT_DIRECTION_RTL;
        for (int index = 0; index < spans.length; index++) {
            View child = row.getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            ViewGroup.MarginLayoutParams params = (ViewGroup.MarginLayoutParams) child.getLayoutParams();
            int cellLeft = rtl ? inner - edges[index + 1] : edges[index];
            int left = row.getPaddingLeft() + cellLeft + params.leftMargin;
            child.layout(left, child.getTop(), left + child.getMeasuredWidth(), child.getBottom());
        }
    }
}
