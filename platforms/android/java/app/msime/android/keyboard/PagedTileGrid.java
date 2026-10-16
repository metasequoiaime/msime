package app.msime.android;


import android.annotation.SuppressLint;
import android.content.Context;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.VelocityTracker;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.animation.DecelerateInterpolator;
import android.widget.OverScroller;
import app.msime.android.KeyboardGeometry;

/**
 * 横向分页的网格：默认 4 列 × 2 行一页，子视图按顺序逐页排布，左右滑动按页吸附。功能面板、皮肤面板、输入方式面板共用它。
 *
 * <p>间距按设计令牌 §5：行距 16 dp、列距 4 dp、左右内边距 4 dp；行高由调用方给（功能面板条目 52 dp）。页切换通过 {@link OnPageChangeListener} 告诉页点。无障碍上支持前后滚动动作，翻一页。
 */
public final class PagedTileGrid extends ViewGroup {
    /** 页切换回调。 */
    public interface OnPageChangeListener {
        void onPageChanged(int page, int pageCount);
    }

    private static final int SNAP_MS = 260;

    private final OverScroller scroller;
    private final int touchSlop;
    private final int minFlingVelocity;
    private int columns = 4;
    private int rows = 2;
    private float rowHeightDp = 52f;
    private float rowGapDp = 16f;
    private float columnGapDp = 4f;
    private float sidePaddingDp = 4f;
    private int page;
    private OnPageChangeListener listener;
    private VelocityTracker velocity;
    private float downX;
    private float downY;
    private float lastX;
    private boolean dragging;

    public PagedTileGrid(Context context) {
        super(context);
        scroller = new OverScroller(context, new DecelerateInterpolator());
        ViewConfiguration configuration = ViewConfiguration.get(context);
        touchSlop = configuration.getScaledTouchSlop();
        minFlingVelocity = configuration.getScaledMinimumFlingVelocity() * 4;
        setClipToPadding(false);
    }

    /** 每页条目数。 */
    public static int perPage(int columns, int rows) {
        long product = (long) BoundsPolicy.bounded(columns, 1, Integer.MAX_VALUE)
            * BoundsPolicy.bounded(rows, 1, Integer.MAX_VALUE);
        return product > Integer.MAX_VALUE ? Integer.MAX_VALUE : (int) product;
    }

    /** 页数：至少一页。 */
    public static int pageCount(int items, int perPage) {
        if (items <= 0) return 1;
        int capacity = BoundsPolicy.atLeast(perPage, 1);
        return items / capacity + (items % capacity == 0 ? 0 : 1);
    }

    /** 输入方式和功能面板一页的行数和行距（dp），见 {@link #pickerGrid}。 */
    public record PickerGrid(int rows, float rowGap) {}

    /**
     * 输入方式和功能面板每页几行、行距多少，与 iOS `KeyboardPagedGridView.pickerRows` / `pickerRowGap`、鸿蒙 `FunctionPanelPolicy.pickerGrid` 同一条规则：手机横屏两行、行距 16，其余三行、行距 12。安卓的键区横竖屏一样高，最矮时（高度 75%、行距 4 dp）只有 178 dp，三行磁贴连同面板的边距和页点放不进去：键区放不下行距 12 时先把行距收小，收到 4 还放不下就退回两行，不裁掉磁贴。
     *
     * @param phoneLandscape 手机（`smallestScreenWidthDp` < 600）横屏
     * @param keyAreaDp 面板能占的键区高度（{@link KeyboardHeightPolicy#keyAreaHeightDp}）
     * @param rowHeightDp 磁贴的行高
     * @param chromeDp 面板里网格以外的高度：上下内边距、页点和它的边距
     */
    public static PickerGrid pickerGrid(boolean phoneLandscape, float keyAreaDp, float rowHeightDp, float chromeDp) {
        if (phoneLandscape) return new PickerGrid(2, 16f);
        float gap = Math.min(12f, (float) Math.floor((keyAreaDp - chromeDp - 3 * rowHeightDp) / 2));
        return gap >= 4f ? new PickerGrid(3, gap) : new PickerGrid(2, 16f);
    }

    /** 第 {@code index} 个条目所在的页。 */
    public static int pageOf(int index, int perPage) {
        return BoundsPolicy.nonNegative(index)
            / BoundsPolicy.bounded(perPage, 1, Integer.MAX_VALUE);
    }

    /**
     * 松手后吸附到哪一页：快速甩动按方向翻一页，否则按滚动位置四舍五入。
     *
     * @param velocityX 正值表示手指向右甩（看前一页）
     */
    public static int settlePage(int current, float scrollX, float pageWidth, float velocityX,
            float minFling, int pageCount) {
        int target;
        if (pageWidth <= 0) {
            target = current;
        } else if (Math.abs(velocityX) >= minFling) {
            target = velocityX < 0 ? current + 1 : current - 1;
        } else {
            target = Math.round(scrollX / pageWidth);
        }
        return BoundsPolicy.bounded(target, 0, pageCount - 1);
    }

    public void setGrid(int columnCount, int rowCount) {
        columns = BoundsPolicy.bounded(columnCount, 1, Integer.MAX_VALUE);
        rows = BoundsPolicy.bounded(rowCount, 1, Integer.MAX_VALUE);
        requestLayout();
    }

    /** 行高、行距、列距与左右内边距，单位 dp。 */
    public void setSpacing(float rowHeight, float rowGap, float columnGap, float sidePadding) {
        rowHeightDp = rowHeight;
        rowGapDp = rowGap;
        columnGapDp = columnGap;
        sidePaddingDp = sidePadding;
        requestLayout();
    }

    public void setOnPageChangeListener(OnPageChangeListener value) { listener = value; }

    public int page() { return page; }

    public int pageCount() { return pageCount(getChildCount(), perPage(columns, rows)); }

    public void setPage(int value, boolean animate) {
        int target = BoundsPolicy.bounded(value, 0, pageCount() - 1);
        int width = getWidth();
        if (!animate || width <= 0) {
            scroller.forceFinished(true);
            scrollTo(target * width, 0);
        } else {
            int dx = target * width - getScrollX();
            scroller.startScroll(getScrollX(), 0, dx, 0, SNAP_MS);
            postInvalidateOnAnimation();
        }
        if (target != page) {
            page = target;
            if (listener != null) listener.onPageChanged(page, pageCount());
        }
    }

    private float px(float dp) { return DimensionPolicy.floatPixels(getContext(), dp); }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int width = MeasureSpec.getSize(widthMeasureSpec);
        int rowHeight = Math.round(px(rowHeightDp));
        int desiredHeight = rows * rowHeight + Math.round((rows - 1) * px(rowGapDp))
            + getPaddingTop() + getPaddingBottom();
        int height = resolveSize(desiredHeight, heightMeasureSpec);
        float inner = width - px(sidePaddingDp) * 2 - px(columnGapDp) * (columns - 1);
        int cellWidth = BoundsPolicy.nonNegative(Math.round(inner / columns));
        int childWidth = MeasureSpec.makeMeasureSpec(cellWidth, MeasureSpec.EXACTLY);
        int childHeight = MeasureSpec.makeMeasureSpec(rowHeight, MeasureSpec.EXACTLY);
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            if (child.getVisibility() != GONE) child.measure(childWidth, childHeight);
        }
        setMeasuredDimension(width, height);
    }

    @Override protected void onLayout(boolean changed, int l, int t, int r, int b) {
        int width = r - l;
        int perPage = perPage(columns, rows);
        float side = px(sidePaddingDp);
        float columnGap = px(columnGapDp);
        float rowGap = px(rowGapDp);
        float rowHeight = px(rowHeightDp);
        float cellWidth = (width - side * 2 - columnGap * (columns - 1)) / columns;
        for (int index = 0; index < getChildCount(); index++) {
            View child = getChildAt(index);
            int pageIndex = index / perPage;
            int slot = index % perPage;
            int row = slot / columns;
            int column = slot % columns;
            int left = Math.round(pageIndex * width + side + column * (cellWidth + columnGap));
            int top = Math.round(getPaddingTop() + row * (rowHeight + rowGap));
            child.layout(left, top, left + child.getMeasuredWidth(), top + child.getMeasuredHeight());
        }
        if (changed) {
            page = BoundsPolicy.atMost(page, pageCount() - 1);
            scrollTo(page * width, 0);
        }
    }

    @Override public void computeScroll() {
        if (scroller.computeScrollOffset()) {
            scrollTo(scroller.getCurrX(), 0);
            postInvalidateOnAnimation();
        }
    }

    @Override public boolean onInterceptTouchEvent(MotionEvent event) {
        if (pageCount() <= 1) return false;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                downX = lastX = event.getX();
                downY = event.getY();
                dragging = !scroller.isFinished();
                if (dragging) scroller.forceFinished(true);
                trackVelocity(event);
            }
            case MotionEvent.ACTION_MOVE -> {
                trackVelocity(event);
                float dx = Math.abs(event.getX() - downX);
                float dy = Math.abs(event.getY() - downY);
                if (dx > touchSlop && dx > dy) {
                    dragging = true;
                    lastX = event.getX();
                    getParent().requestDisallowInterceptTouchEvent(true);
                }
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (!dragging) recycleVelocity();
            }
            default -> { }
        }
        return dragging;
    }

    // 点击仍由子视图处理；这里只把横向拖动变成翻页，自身不可点击。
    @SuppressLint("ClickableViewAccessibility")
    @Override public boolean onTouchEvent(MotionEvent event) {
        if (pageCount() <= 1) return false;
        trackVelocity(event);
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                downX = lastX = event.getX();
                downY = event.getY();
                return true;
            }
            case MotionEvent.ACTION_MOVE -> {
                if (!dragging && Math.abs(event.getX() - downX) > touchSlop) dragging = true;
                if (dragging) {
                    float delta = lastX - event.getX();
                    int max = (pageCount() - 1) * getWidth();
                    int next = Math.round(BoundsPolicy.bounded(getScrollX() + delta, 0f, max));
                    scrollTo(next, 0);
                }
                lastX = event.getX();
                return true;
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                float vx = 0f;
                if (velocity != null) {
                    velocity.computeCurrentVelocity(1000);
                    vx = velocity.getXVelocity();
                }
                int target = settlePage(page, getScrollX(), getWidth(), vx, minFlingVelocity,
                    pageCount());
                dragging = false;
                recycleVelocity();
                setPage(target, true);
                return true;
            }
            default -> {
                return true;
            }
        }
    }

    private void trackVelocity(MotionEvent event) {
        if (velocity == null) velocity = VelocityTracker.obtain();
        velocity.addMovement(event);
    }

    private void recycleVelocity() {
        if (velocity != null) {
            velocity.recycle();
            velocity = null;
        }
    }

    @Override protected void onDetachedFromWindow() {
        scroller.forceFinished(true);
        // 翻页动画中途被移出窗口（「更多」面板重画时先 removeAllViews 再挂回同一个网格）：停下的同时对齐到当前页。否则滚动位置留在两页之间，再挂回来时尺寸没变、onLayout 不会重新对齐，面板一直卡在半页上，左右各露出半截磁贴。
        scrollTo(page * getWidth(), 0);
        recycleVelocity();
        super.onDetachedFromWindow();
    }

    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.setScrollable(pageCount() > 1);
        if (page < pageCount() - 1) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD);
        }
        if (page > 0) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD);
        }
    }

    @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD && page < pageCount() - 1) {
            setPage(page + 1, true);
            return true;
        }
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD && page > 0) {
            setPage(page - 1, true);
            return true;
        }
        return super.performAccessibilityAction(action, arguments);
    }
}
