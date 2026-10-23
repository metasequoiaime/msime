package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.ArrayDeque;

/** 文本编辑面板的移动区域；所有移动都由宿主交给真实编辑器。 */
public final class TextTrackpadView extends View {
    public interface Listener {
        Object target();
        boolean move(TextTrackpadGesture.Steps steps);
        void select(TextTrackpadGesture.Tap tap);
        void active(boolean active);
    }

    private record Point(float x, float y) { }
    private final Listener listener;
    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final ArrayDeque<Point> points = new ArrayDeque<>();
    private TextTrackpadGesture gesture;
    private boolean tapSelection;
    private boolean trail;
    private int foreground;
    private int accent;
    private final Runnable longPress;

    public TextTrackpadView(Context context, Listener listener) {
        super(context);
        this.listener = listener;
        longPress = () -> {
            if (gesture != null && gesture.longPress(listener.target(), SystemClock.uptimeMillis())) {
                announceForAccessibility("拖动延伸选区");
                invalidate();
            }
        };
        density = context.getResources().getDisplayMetrics().density;
        setContentDescription("文本编辑触控板");
        ViewPolicy.setInteractive(this, true);
    }

    /** 参数已经过共享 Rust 校验；此处只转换系统触摸单位，不再维护默认值或设置范围。 */
    public void configure(int horizontalDp, int verticalDp, boolean tapSelection, boolean trail,
            int background, int foreground, int accent) {
        cancel();
        this.tapSelection = tapSelection;
        this.trail = trail;
        this.foreground = foreground;
        this.accent = accent;
        gesture = new TextTrackpadGesture(horizontalDp, verticalDp, ViewConfiguration.getLongPressTimeout(),
            ViewConfiguration.getDoubleTapTimeout(), ViewConfiguration.get(getContext()).getScaledTouchSlop() / density);
        ViewPolicy.setBackgroundColor(this, background);
        invalidate();
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (gesture == null) return false;
        if (event.getPointerCount() != 1 || event.getActionMasked() == MotionEvent.ACTION_CANCEL
                || event.getActionMasked() == MotionEvent.ACTION_OUTSIDE) {
            cancel();
            return true;
        }
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                gesture.down(listener.target(), event.getX() / density, event.getY() / density, event.getEventTime());
                if (!gesture.active()) return false;
                points.clear();
                point(event.getX(), event.getY());
                listener.active(true);
                postDelayed(longPress, ViewConfiguration.getLongPressTimeout());
                getParent().requestDisallowInterceptTouchEvent(true);
            }
            case MotionEvent.ACTION_MOVE -> {
                for (int at = Math.max(0, event.getHistorySize() - 32); at < event.getHistorySize(); at++)
                    move(event.getHistoricalX(at), event.getHistoricalY(at));
                move(event.getX(), event.getY());
            }
            case MotionEvent.ACTION_UP -> {
                move(event.getX(), event.getY());
                TextTrackpadGesture.Tap tap = gesture.up(listener.target(), event.getX() / density,
                    event.getY() / density, event.getEventTime(), tapSelection);
                finishVisual();
                if (tap != TextTrackpadGesture.Tap.NONE) listener.select(tap);
                performClick();
            }
            default -> { }
        }
        return true;
    }

    private void move(float x, float y) {
        TextTrackpadGesture.Steps steps = gesture.move(listener.target(), 1, x / density, y / density);
        if (!gesture.active()) {
            finishVisual();
            return;
        }
        point(x, y);
        if ((steps.horizontal() != 0 || steps.vertical() != 0) && !listener.move(steps)) cancel();
    }

    private void point(float x, float y) {
        if (!trail) return;
        if (points.size() >= 64) points.removeFirst();
        points.addLast(new Point(x, y));
        invalidate();
    }

    public void cancel() {
        if (gesture != null) gesture.cancel();
        finishVisual();
    }

    private void finishVisual() {
        removeCallbacks(longPress);
        points.clear();
        listener.active(false);
        invalidate();
    }

    @Override public boolean performClick() {
        super.performClick();
        return true;
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(foreground);
        paint.setTextAlign(Paint.Align.CENTER);
        paint.setTextSize(16 * density);
        canvas.drawText("左右移动字符，上下移动行", getWidth() / 2f, getHeight() / 2f - 6 * density, paint);
        paint.setTextSize(12 * density);
        canvas.drawText(tapSelection ? "长按拖动选择 · 双击选词 · 三击选行" : "长按后拖动延伸选区",
            getWidth() / 2f, getHeight() / 2f + 20 * density, paint);
        if (points.isEmpty()) return;
        paint.setColor(accent);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(3 * density);
        Path path = new Path();
        boolean first = true;
        for (Point point : points) {
            if (first) path.moveTo(point.x(), point.y());
            else path.lineTo(point.x(), point.y());
            first = false;
        }
        canvas.drawPath(path, paint);
        paint.setStyle(Paint.Style.FILL);
        Point last = points.getLast();
        canvas.drawCircle(last.x(), last.y(), 4 * density, paint);
    }

    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.setScrollable(true);
        info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_LEFT);
        info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_RIGHT);
        info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_UP);
        info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_DOWN);
    }

    @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
        int x = action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_LEFT.getId() ? -1
            : action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_RIGHT.getId() ? 1 : 0;
        int y = action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_UP.getId() ? -1
            : action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_DOWN.getId() ? 1 : 0;
        if (x != 0 || y != 0) return listener.move(new TextTrackpadGesture.Steps(x, y, false));
        return super.performAccessibilityAction(action, arguments);
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (!focused) cancel();
    }

    @Override protected void onDetachedFromWindow() {
        cancel();
        super.onDetachedFromWindow();
    }
}
