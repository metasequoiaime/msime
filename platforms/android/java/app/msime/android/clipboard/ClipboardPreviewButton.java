package app.msime.android;

import android.annotation.SuppressLint;
import android.content.Context;
import android.os.Bundle;
import android.view.MotionEvent;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;

/** 在限高卡片内浏览全文，点按、长按与既有横滑监听仍由键帽处理。 */
public final class ClipboardPreviewButton extends KeyboardPressButton {
    private final int touchSlop;
    private float downX;
    private float downY;
    private int startScroll;
    private boolean scrolling;
    private boolean delegated;

    public ClipboardPreviewButton(Context context) {
        super(context);
        touchSlop = ViewConfiguration.get(context).getScaledTouchSlop();
        setAllCaps(false);
        setVerticalScrollBarEnabled(true);
        setMaxLines(Integer.MAX_VALUE);
        setEllipsize(null);
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        super.onMeasure(widthMeasureSpec, heightMeasureSpec);
        int lines = Math.max(1, Math.min(3, getLineCount()));
        int height = getCompoundPaddingTop() + getCompoundPaddingBottom() + lines * getLineHeight();
        setMeasuredDimension(getMeasuredWidth(), resolveSize(height, heightMeasureSpec));
    }

    private int scrollLimit() {
        return getLayout() == null ? 0 : Math.max(0,
            getLayout().getHeight() - getHeight() + getCompoundPaddingTop() + getCompoundPaddingBottom());
    }

    private void keepGesture(boolean keep) {
        if (getParent() != null) getParent().requestDisallowInterceptTouchEvent(keep);
    }

    // 点击由父类发出；纵向预览超过移动阈值后先取消父类按压，不会在松手时误粘贴。
    @SuppressLint("ClickableViewAccessibility")
    @Override public boolean dispatchTouchEvent(MotionEvent event) {
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_DOWN) {
            downX = event.getX();
            downY = event.getY();
            startScroll = getScrollY();
            scrolling = false;
            delegated = scrollLimit() == 0;
            if (!delegated) keepGesture(true);
        } else if (action == MotionEvent.ACTION_MOVE && !delegated) {
            float dx = event.getX() - downX;
            float dy = event.getY() - downY;
            if (!scrolling && Math.max(Math.abs(dx), Math.abs(dy)) > touchSlop) {
                // 横向判定与删除共用阈值；决定方向之后不在同一次手势中反复切换。
                if (Math.abs(dx) >= Math.abs(dy) * ClipboardSwipePolicy.HORIZONTAL_DOMINANCE) {
                    delegated = true;
                } else {
                    MotionEvent cancel = MotionEvent.obtain(event);
                    cancel.setAction(MotionEvent.ACTION_CANCEL);
                    super.dispatchTouchEvent(cancel);
                    cancel.recycle();
                    scrolling = dy < 0 ? startScroll < scrollLimit() : startScroll > 0;
                    if (!scrolling) {
                        delegated = true;
                        keepGesture(false);
                        return true;
                    }
                }
            }
            if (scrolling) {
                scrollTo(0, Math.max(0, Math.min(scrollLimit(), startScroll - Math.round(dy))));
                return true;
            }
        } else if (action == MotionEvent.ACTION_UP || action == MotionEvent.ACTION_CANCEL) {
            keepGesture(false);
            boolean handled = scrolling;
            scrolling = false;
            if (handled) return true;
        }
        return super.dispatchTouchEvent(event);
    }

    @SuppressWarnings("deprecation")
    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.setScrollable(scrollLimit() > 0);
        if (getScrollY() < scrollLimit()) info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
        if (getScrollY() > 0) info.addAction(AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
    }

    @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                || action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD) {
            int page = Math.max(1, getHeight() - getCompoundPaddingTop() - getCompoundPaddingBottom());
            int next = Math.max(0, Math.min(scrollLimit(), getScrollY()
                + (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD ? page : -page)));
            if (next == getScrollY()) return false;
            scrollTo(0, next);
            return true;
        }
        return super.performAccessibilityAction(action, arguments);
    }
}
