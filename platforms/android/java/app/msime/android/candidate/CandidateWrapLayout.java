package app.msime.android;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;

/** Minimal dependency-free flow layout for measured candidate chip widths. */
public final class CandidateWrapLayout extends ViewGroup {
    private final int spacing;

    public CandidateWrapLayout(Context context, int spacing) {
        super(context);
        if (spacing < 0) throw new IllegalArgumentException("Invalid candidate spacing");
        this.spacing = spacing;
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int widthMode = MeasureSpec.getMode(widthMeasureSpec);
        int widthLimit = MeasureSpec.getSize(widthMeasureSpec);
        int available = widthMode == MeasureSpec.UNSPECIFIED
            ? Integer.MAX_VALUE : BoundsPolicy.nonNegative(widthLimit - getPaddingLeft() - getPaddingRight());
        int occupied = 0;
        int rowHeight = 0;
        int contentHeight = 0;
        int contentWidth = 0;
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            measureChild(child, widthMeasureSpec, heightMeasureSpec);
            int childWidth = child.getMeasuredWidth();
            int childHeight = child.getMeasuredHeight();
            if (CandidateWrapPolicy.shouldWrap(occupied, childWidth, available, spacing)) {
                contentWidth = Math.max(contentWidth, occupied);
                contentHeight += rowHeight + spacing;
                occupied = childWidth;
                rowHeight = childHeight;
            } else {
                occupied = occupied == 0 ? childWidth : occupied + spacing + childWidth;
                rowHeight = Math.max(rowHeight, childHeight);
            }
        }
        contentWidth = Math.max(contentWidth, occupied);
        contentHeight += rowHeight;
        int desiredWidth = getPaddingLeft() + contentWidth + getPaddingRight();
        int desiredHeight = getPaddingTop() + contentHeight + getPaddingBottom();
        setMeasuredDimension(resolveSize(desiredWidth, widthMeasureSpec),
            resolveSize(desiredHeight, heightMeasureSpec));
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        int contentLeft = getPaddingLeft();
        int contentRight = Math.max(contentLeft, right - left - getPaddingRight());
        int available = contentRight - contentLeft;
        int x = contentLeft;
        int y = getPaddingTop();
        int rowHeight = 0;
        int occupied = 0;
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            int childWidth = child.getMeasuredWidth();
            int childHeight = child.getMeasuredHeight();
            if (CandidateWrapPolicy.shouldWrap(occupied, childWidth, available, spacing)) {
                x = contentLeft;
                y += rowHeight + spacing;
                occupied = 0;
                rowHeight = 0;
            }
            child.layout(x, y, x + childWidth, y + childHeight);
            x += childWidth + spacing;
            occupied = occupied == 0 ? childWidth : occupied + spacing + childWidth;
            rowHeight = Math.max(rowHeight, childHeight);
        }
    }

    @Override protected LayoutParams generateDefaultLayoutParams() {
        return new LayoutParams(LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT);
    }
}
