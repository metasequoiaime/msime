package app.msime.android;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.ScrollView;

/**
 * 九键左侧的符号栏：一列符号键放在可以上下滚动的容器里，一屏最多显示 {@link NineKeySidebarPolicy#VISIBLE_ROWS} 个。
 *
 * <p>每个键的高度在测量时按栏高重新分配（{@link NineKeySidebarPolicy#rowHeight}），所以键盘高度偏好、行距变化后不必重建；键自己的上下外边距（行距）从分到的高度里扣掉，滚动一整屏正好是整数个键。
 */
public final class NineKeySymbolRail extends ScrollView {
    private final LinearLayout column;

    public NineKeySymbolRail(Context context) {
        super(context);
        column = new LinearLayout(context);
        column.setOrientation(LinearLayout.VERTICAL);
        setVerticalScrollBarEnabled(false);
        setOverScrollMode(View.OVER_SCROLL_NEVER);
        setVerticalFadingEdgeEnabled(true);
        setFadingEdgeLength(KeyboardGeometry.pixels(context, 12));
        addView(column, new ScrollView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT));
    }

    /** 加一个符号键；高度在测量时分配。 */
    public void addSymbol(View key) {
        column.addView(key, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0));
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int row = NineKeySidebarPolicy.rowHeight(MeasureSpec.getSize(heightMeasureSpec), column.getChildCount(),
            KeyboardGeometry.pixels(getContext(), NineKeySidebarPolicy.MIN_ROW_HEIGHT_DP));
        for (int index = 0; index < column.getChildCount(); index++) {
            View child = column.getChildAt(index);
            if (!(child.getLayoutParams() instanceof LinearLayout.LayoutParams params)) continue;
            params.height = Math.max(0, row - params.topMargin - params.bottomMargin);
        }
        super.onMeasure(widthMeasureSpec, heightMeasureSpec);
    }
}
