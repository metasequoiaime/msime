package app.msime.android;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;

/**
 * 从左到右排子视图、一行放不下就换到下一行的容器：剪贴板分词把词片排成这样（#5645）。
 *
 * <p>只做这一件事：子视图按自己的大小测量（宽度不超过一整行），行内间距和行距固定。宿主不能在 `core/` 里用 Material 的 `ChipGroup`：`check-host.sh` 的 JVM 冒烟会跳过任何引用 androidx 或 Material 的源文件，面板代码就编不进去了。
 */
final class WrapRowLayout extends ViewGroup {
    private final int horizontalGap;
    private final int verticalGap;

    WrapRowLayout(Context context, int horizontalGap, int verticalGap) {
        super(context);
        this.horizontalGap = BoundsPolicy.nonNegative(horizontalGap);
        this.verticalGap = BoundsPolicy.nonNegative(verticalGap);
    }

    /** 宽 `width` 的子视图放在当前行 `x` 处会不会越过可用宽度；行首的那一个再宽也不换行。 */
    static boolean wraps(int x, int width, int available) {
        return x > 0 && x + width > available;
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int widthMode = MeasureSpec.getMode(widthMeasureSpec);
        boolean bounded = widthMode != MeasureSpec.UNSPECIFIED;
        int available = BoundsPolicy.nonNegative(MeasureSpec.getSize(widthMeasureSpec)
            - getPaddingLeft() - getPaddingRight());
        int childWidthSpec = bounded
            ? MeasureSpec.makeMeasureSpec(available, MeasureSpec.AT_MOST)
            : MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED);
        int childHeightSpec = MeasureSpec.makeMeasureSpec(0, MeasureSpec.UNSPECIFIED);
        int x = 0;
        int y = 0;
        int rowHeight = 0;
        int widest = 0;
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            child.measure(childWidthSpec, childHeightSpec);
            int width = child.getMeasuredWidth();
            if (bounded && wraps(x, width, available)) {
                y += rowHeight + verticalGap;
                x = 0;
                rowHeight = 0;
            }
            widest = BoundsPolicy.atLeast(widest, x + width);
            x += width + horizontalGap;
            rowHeight = BoundsPolicy.atLeast(rowHeight, child.getMeasuredHeight());
        }
        int width = widthMode == MeasureSpec.EXACTLY ? MeasureSpec.getSize(widthMeasureSpec)
            : widest + getPaddingLeft() + getPaddingRight();
        int height = y + rowHeight + getPaddingTop() + getPaddingBottom();
        setMeasuredDimension(resolveSize(width, widthMeasureSpec), resolveSize(height, heightMeasureSpec));
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        int available = BoundsPolicy.nonNegative(right - left - getPaddingLeft() - getPaddingRight());
        int x = 0;
        int y = 0;
        int rowHeight = 0;
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            if (child.getVisibility() == GONE) continue;
            int width = child.getMeasuredWidth();
            int height = child.getMeasuredHeight();
            if (wraps(x, width, available)) {
                y += rowHeight + verticalGap;
                x = 0;
                rowHeight = 0;
            }
            int childLeft = getPaddingLeft() + x;
            int childTop = getPaddingTop() + y;
            child.layout(childLeft, childTop, childLeft + width, childTop + height);
            x += width + horizontalGap;
            rowHeight = BoundsPolicy.atLeast(rowHeight, height);
        }
    }
}
