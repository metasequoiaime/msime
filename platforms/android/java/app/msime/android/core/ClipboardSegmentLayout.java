package app.msime.android;

import android.content.Context;
import android.graphics.Rect;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.ScrollView;

/** 词片起手拖选连续范围，空白起手仍由外层滚动；普通点按交给原按钮。 */
final class ClipboardSegmentLayout extends WrapRowLayout {
    interface SelectionListener {
        void changed(View child, boolean selected);
        void refreshed();
    }

    private final SelectionListener listener;
    private final int slop;
    private final Rect visible = new Rect();
    private final int[] location = new int[2];
    private boolean[] baseline;
    private int anchor = -1;
    private boolean dragging;
    private float downX;
    private float downY;
    private float rawX;
    private float rawY;
    private final Runnable scrollStep = this::scrollSelection;

    ClipboardSegmentLayout(Context context, int gap, SelectionListener listener) {
        super(context, gap, gap);
        this.listener = listener;
        slop = ViewConfiguration.get(context).getScaledTouchSlop();
    }

    @Override public boolean onInterceptTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                finish(false);
                anchor = hit(event.getX(), event.getY(), false);
                if (anchor >= 0) {
                    downX = event.getX();
                    downY = event.getY();
                    baseline = new boolean[getChildCount()];
                    for (int i = 0; i < baseline.length; i++) baseline[i] = getChildAt(i).isSelected();
                    getParent().requestDisallowInterceptTouchEvent(true);
                }
            }
            case MotionEvent.ACTION_MOVE -> {
                if (anchor >= 0 && Math.hypot(event.getX() - downX, event.getY() - downY) > slop) {
                    dragging = true;
                    return true;
                }
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> finish(false);
            default -> { }
        }
        return false;
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (!dragging) return super.onTouchEvent(event);
        rawX = event.getRawX();
        rawY = event.getRawY();
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_MOVE -> {
                selectAt(event.getX(), event.getY());
                removeCallbacks(scrollStep);
                postDelayed(scrollStep, 32);
            }
            case MotionEvent.ACTION_UP -> {
                selectAt(event.getX(), event.getY());
                finish(false);
            }
            case MotionEvent.ACTION_CANCEL, MotionEvent.ACTION_POINTER_DOWN -> finish(true);
            default -> { }
        }
        return true;
    }

    /** 先找最近的行，再找该行最近的词；跨行时按原文索引取连续区间。 */
    private int hit(float x, float y, boolean nearest) {
        int found = -1;
        float bestY = Float.MAX_VALUE;
        float bestX = Float.MAX_VALUE;
        for (int i = 0; i < getChildCount(); i++) {
            View child = getChildAt(i);
            if (child.getVisibility() != VISIBLE) continue;
            float dy = Math.max(child.getTop() - y, Math.max(y - child.getBottom(), 0));
            float dx = Math.max(child.getLeft() - x, Math.max(x - child.getRight(), 0));
            if (!nearest && (dx > 0 || dy > 0)) continue;
            if (dy < bestY || dy == bestY && dx < bestX) {
                found = i;
                bestY = dy;
                bestX = dx;
            }
        }
        return found;
    }

    private void selectAt(float x, float y) {
        int end = hit(x, y, true);
        if (baseline == null || end < 0) return;
        for (int i = 0; i < baseline.length; i++) {
            boolean selected = i >= Math.min(anchor, end) && i <= Math.max(anchor, end)
                ? !baseline[anchor] : baseline[i];
            View child = getChildAt(i);
            if (child.isSelected() != selected) listener.changed(child, selected);
        }
        listener.refreshed();
    }

    /** 拖到可见区上下边缘时持续滚动，手指不必松开就能跨越多屏。 */
    private void scrollSelection() {
        if (!dragging || !getLocalVisibleRect(visible)) return;
        getLocationOnScreen(location);
        float y = rawY - location[1];
        int edge = Math.max(slop * 3, 1);
        int step = y < visible.top + edge ? -edge / 2 : y > visible.bottom - edge ? edge / 2 : 0;
        if (step != 0 && getParent() instanceof android.view.View content
                && content.getParent() instanceof ScrollView scroll) {
            scroll.scrollBy(0, step);
            getLocationOnScreen(location);
            selectAt(rawX - location[0], rawY - location[1]);
        }
        postDelayed(scrollStep, 32);
    }

    private void finish(boolean restore) {
        removeCallbacks(scrollStep);
        if (restore && dragging && baseline != null) {
            for (int i = 0; i < baseline.length; i++) listener.changed(getChildAt(i), baseline[i]);
            listener.refreshed();
        }
        baseline = null;
        anchor = -1;
        dragging = false;
        if (getParent() != null) getParent().requestDisallowInterceptTouchEvent(false);
    }

    @Override protected void onDetachedFromWindow() {
        finish(false);
        super.onDetachedFromWindow();
    }
}
