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
 * <p>间距按设计令牌 §5：行距 16 dp、列距 4 dp、左右内边距 4 dp；行高由调用方给（功能面板条目 52 dp）。功能面板和输入方式面板改由网格按量到的高度定行数、行高和行距（{@link #setPickerFit}），磁贴按拿到的高度自己缩放图标和文字。页切换通过 {@link OnPageChangeListener} 告诉页点。无障碍上支持前后滚动动作，翻一页。
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
    /** 按量到的高度排行（{@link #setPickerFit}）：输入方式和功能面板用，皮肤面板仍是调用方给定的行数和行距。 */
    private boolean pickerFit;
    private boolean pickerPhoneLandscape;
    private float pickerRowHeightDp;
    /** 按量到的高度排行时，最近一次测量定下的行数和整像素行高、行距；onLayout 才把行数换成它，免得逐行四舍五入后最后一行超出网格。 */
    private int fittedRows = 2;
    private int fittedRowHeight;
    private int fittedRowGap;
    /** 下一次排版后要翻到的条目（{@link #revealOnLayout}），没有时为 -1。 */
    private int pendingReveal = -1;
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

    /** 三行时原尺寸的行距（dp）。 */
    public static final float PICKER_THREE_ROW_GAP_DP = 12f;
    /** 两行时原尺寸的行距（dp）。 */
    public static final float PICKER_TWO_ROW_GAP_DP = 16f;
    /** 磁贴缩小的下限（dp）：三行缩到比它还矮就改成两行。取触控目标的最小高度 44，与 iOS、鸿蒙相同。 */
    public static final float PICKER_MIN_TILE_DP = 44f;

    /** 输入方式和功能面板一页的排法：行数和磁贴的缩放比例（1 是原尺寸，不放大），见 {@link #pickerGrid}。 */
    public record PickerGrid(int rows, float scale) {
        /** 缩放后的行高（dp）。 */
        public float rowHeight(float fullRowHeightDp) { return fullRowHeightDp * scale; }

        /** 缩放后的行距（dp）：原尺寸的行距乘同一个比例。 */
        public float rowGap() { return pickerRowGap(rows) * scale; }
    }

    /** 原尺寸的行距：三行 12 dp，两行 16 dp。 */
    public static float pickerRowGap(int rows) {
        return rows >= 3 ? PICKER_THREE_ROW_GAP_DP : PICKER_TWO_ROW_GAP_DP;
    }

    /**
     * 输入方式和功能面板在网格实际能占的高度里每页几行、磁贴缩多少，与 iOS `KeyboardPagedGridView.pickerLayout`、鸿蒙 `FunctionPanelPolicy.pickerGrid` 同一条规则、同一组常数：手机横屏两行，其余三行；放得下原尺寸（三行 3 × 行高 + 2 × 12，两行 2 × 行高 + 16）就用原尺寸，放不下就把行高和行距按同一比例缩小，图标和文字跟着磁贴一起缩；三行缩到行高不足 {@link #PICKER_MIN_TILE_DP} 才改成两行，两行也不再缩到这个下限以下。
     *
     * <p>高度必须是面板排好后量到的，不能用预测的键区高度：原先按 {@link KeyboardHeightPolicy#keyAreaHeightDp} 收小行距，在 85%–90% 这几档把三行算得刚好放满，而真实面板因为各行键高、行距和面板边距分别取整，比预测矮两三个像素，网格溢出后外层 ScrollView 能上下拖动（用户录屏的 90%）。
     *
     * @param phoneLandscape 手机（`smallestScreenWidthDp` < 600）横屏
     * @param gridDp 网格能占的高度：面板高度减去网格以外的内边距和页点
     * @param rowHeightDp 磁贴原尺寸的行高
     */
    public static PickerGrid pickerGrid(boolean phoneLandscape, float gridDp, float rowHeightDp) {
        float minimumScale = rowHeightDp <= PICKER_MIN_TILE_DP ? 1f : PICKER_MIN_TILE_DP / rowHeightDp;
        if (!phoneLandscape) {
            float three = Math.min(1f, gridDp / (3 * rowHeightDp + 2 * PICKER_THREE_ROW_GAP_DP));
            if (three >= minimumScale - 1e-4f) return new PickerGrid(3, three);
        }
        float two = Math.min(1f, gridDp / (2 * rowHeightDp + PICKER_TWO_ROW_GAP_DP));
        return new PickerGrid(2, Math.max(minimumScale, two));
    }

    /** 最矮的排法（两行、磁贴缩到下限）占的高度（dp）：外层 ScrollView 不限高量网格时报这个高度，让 ScrollView 再按自己的高度量一遍。 */
    public static float pickerMinimumHeightDp(float rowHeightDp) {
        PickerGrid smallest = pickerGrid(true, 0f, rowHeightDp);
        return 2 * smallest.rowHeight(rowHeightDp) + smallest.rowGap();
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

    /**
     * 输入方式和功能面板：行数、行高和行距不再由调用方给，而是每次测量时按量到的高度用 {@link #pickerGrid} 定，所以网格永远不比给它的高度高。行数变了时停在的页退到新的最后一页，并经 {@link OnPageChangeListener} 报告新的页数，页点跟着重算。
     *
     * <p>网格放在 ScrollView 里（「更多」面板的子页需要竖向滚动），ScrollView 先不限高量一遍子视图，只有子视图比它矮时才按它的高度再量一遍；所以不限高时网格报最矮的排法，好让第二遍拿到真实高度。一次布局里会量好几遍（LinearLayout 的权重在不限高时还会拿 0 高再量一遍），所以测量只记下这一遍的排法，行数到 onLayout 才生效：只有最后一遍的结果会被排出来。
     *
     * @param rowHeightDp 磁贴原尺寸的行高
     * @param phoneLandscape 手机横屏：两行
     */
    public void setPickerFit(float rowHeightDp, boolean phoneLandscape) {
        if (pickerFit && pickerRowHeightDp == rowHeightDp && pickerPhoneLandscape == phoneLandscape) return;
        pickerFit = true;
        pickerRowHeightDp = rowHeightDp;
        pickerPhoneLandscape = phoneLandscape;
        requestLayout();
    }

    /** 下一次排版后翻到第 {@code index} 个条目所在的页（不带动画）：按量到的高度排行时，行数和每页条目数要等测量后才知道，打开面板时停在选中项上不能先按行数算好页码。 */
    public void revealOnLayout(int index) {
        pendingReveal = BoundsPolicy.nonNegative(index);
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
        int rowHeight;
        int desiredHeight;
        if (pickerFit) {
            int verticalPadding = getPaddingTop() + getPaddingBottom();
            // 不限高时按最矮的排法报高度（可用高度当 0 算）：外层 ScrollView 随后会按自己的高度再量一遍。
            float available = MeasureSpec.getMode(heightMeasureSpec) == MeasureSpec.UNSPECIFIED ? 0f
                : BoundsPolicy.nonNegative(MeasureSpec.getSize(heightMeasureSpec) - verticalPadding) / px(1f);
            PickerGrid grid = pickerGrid(pickerPhoneLandscape, available, pickerRowHeightDp);
            fittedRows = grid.rows();
            fittedRowHeight = (int) Math.floor(px(grid.rowHeight(pickerRowHeightDp)));
            fittedRowGap = (int) Math.floor(px(grid.rowGap()));
            desiredHeight = fittedRows * fittedRowHeight + (fittedRows - 1) * fittedRowGap + verticalPadding;
            rowHeight = fittedRowHeight;
        } else {
            rowHeight = Math.round(px(rowHeightDp));
            desiredHeight = rows * rowHeight + Math.round((rows - 1) * px(rowGapDp))
                + getPaddingTop() + getPaddingBottom();
        }
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
        boolean pagesChanged = false;
        if (pickerFit && fittedRows != rows) {
            // 最后一遍测量换了行数：每页条目数和页数跟着变，停在的页超出新页数时退到最后一页。
            rows = fittedRows;
            page = BoundsPolicy.atMost(page, pageCount() - 1);
            pagesChanged = true;
        }
        int perPage = perPage(columns, rows);
        float side = px(sidePaddingDp);
        float columnGap = px(columnGapDp);
        float rowGap = pickerFit ? fittedRowGap : px(rowGapDp);
        float rowHeight = pickerFit ? fittedRowHeight : px(rowHeightDp);
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
        if (pendingReveal >= 0) {
            int target = BoundsPolicy.bounded(pageOf(pendingReveal, perPage), 0, pageCount() - 1);
            pendingReveal = -1;
            scroller.forceFinished(true);
            scrollTo(target * width, 0);
            pagesChanged |= target != page;
            page = target;
        } else if (changed || (scroller.isFinished() && !dragging && getScrollX() != page * width)) {
            // 尺寸变了，或测量时行数变了、停在的页退到了新的最后一页：对齐到当前页。
            page = BoundsPolicy.atMost(page, pageCount() - 1);
            scrollTo(page * width, 0);
        }
        // 页点在下一帧更新：布局中途改它会再触发一次布局。
        if (pagesChanged) post(() -> {
            if (listener != null) listener.onPageChanged(page, pageCount());
        });
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
                    // 子控件在交接时收到取消，网格未必再收到同一 MOVE；现在就消费首段位移。
                    dragTo(event.getX());
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
                if (dragging) dragTo(event.getX());
                else lastX = event.getX();
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

    private void dragTo(float x) {
        float delta = lastX - x;
        int max = (pageCount() - 1) * getWidth();
        int next = Math.round(BoundsPolicy.bounded(getScrollX() + delta, 0f, max));
        scrollTo(next, 0);
        lastX = x;
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
