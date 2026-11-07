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
    private int surface;
    private int foreground;
    private int accent;
    private float radius;
    private boolean touching;
    private final Path outline = new Path();
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

    /**
     * 参数已经过共享 Rust 校验；此处只转换系统触摸单位，不再维护默认值或设置范围。
     *
     * <p>{@code surface} 是方向区的底色，{@code radius} 是方向区外框的圆角：触控板盖在方向区上三行，自己画同色底面、点阵和外框，上面两角跟外框一样圆，下边贴着底排。
     */
    public void configure(int horizontalDp, int verticalDp, boolean tapSelection, boolean trail,
            int surface, int foreground, int accent, float radius) {
        cancel();
        this.tapSelection = tapSelection;
        this.trail = trail;
        this.surface = surface;
        this.foreground = foreground;
        this.accent = accent;
        this.radius = radius;
        gesture = new TextTrackpadGesture(horizontalDp, verticalDp, ViewConfiguration.getLongPressTimeout(),
            ViewConfiguration.getDoubleTapTimeout(), ViewConfiguration.get(getContext()).getScaledTouchSlop() / density);
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
                touching = true;
                point(event.getX(), event.getY());
                listener.active(true);
                invalidate();
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

    /** 同一输入框重启连接时让进行中的手势接着走（{@link TextTrackpadGesture#retarget}）。 */
    public void retarget(Object previous, Object next) {
        if (gesture != null) gesture.retarget(previous, next);
    }

    public void cancel() {
        if (gesture != null) gesture.cancel();
        finishVisual();
    }

    private void finishVisual() {
        removeCallbacks(longPress);
        points.clear();
        touching = false;
        listener.active(false);
        invalidate();
    }

    @Override public boolean performClick() {
        super.performClick();
        return true;
    }

    /** 底面：方向区底色，上面两角圆、下边直，贴着底排。 */
    private void prepareOutline(float inset) {
        float corner = Math.max(0, radius - inset);
        outline.reset();
        outline.addRoundRect(inset, inset, getWidth() - inset, getHeight() - inset,
            new float[] {corner, corner, corner, corner, 0, 0, 0, 0}, Path.Direction.CW);
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        prepareOutline(0);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(surface);
        canvas.drawPath(outline, paint);
        // 点阵：表示这一块可以滑动，颜色压得很淡，不跟按键文字抢。
        float spacing = TextTrackpadSurface.DOT_SPACING_DP * density;
        int columns = TextTrackpadSurface.dotCount(getWidth(), spacing);
        int rows = TextTrackpadSurface.dotCount(getHeight(), spacing);
        float x0 = TextTrackpadSurface.firstDot(getWidth(), spacing);
        float y0 = TextTrackpadSurface.firstDot(getHeight(), spacing);
        paint.setColor(ColorPolicy.withAlpha(foreground, .22f));
        float dot = TextTrackpadSurface.DOT_RADIUS_DP * density;
        int saved = canvas.save();
        canvas.clipPath(outline);
        for (int row = 0; row < rows; row++)
            for (int column = 0; column < columns; column++)
                canvas.drawCircle(x0 + column * spacing, y0 + row * spacing, dot, paint);
        canvas.restoreToCount(saved);
        // 一句淡色提示，手指按下（放大镜和光标预览出现）时不画。
        if (!touching) drawHint(canvas);
        drawTrail(canvas);
        float line = Math.max(1, .5f * density);
        prepareOutline(line / 2);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(line);
        paint.setColor(ColorPolicy.withAlpha(foreground, .18f));
        canvas.drawPath(outline, paint);
    }

    private void drawHint(Canvas canvas) {
        String hint = "滑动移动光标";
        paint.setStyle(Paint.Style.FILL);
        paint.setTextAlign(Paint.Align.CENTER);
        android.util.DisplayMetrics display = getResources().getDisplayMetrics();
        float maxSize = android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
            TextTrackpadSurface.HINT_MAX_SP, display);
        float minSize = android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
            TextTrackpadSurface.HINT_MIN_SP, display);
        paint.setTextSize(maxSize);
        Paint.FontMetrics metrics = paint.getFontMetrics();
        float size = TextTrackpadSurface.hintSize(paint.measureText(hint),
            getWidth() - 2 * TextTrackpadSurface.HINT_SIDE_DP * density, metrics.descent - metrics.ascent,
            getHeight(), maxSize, minSize);
        if (size <= 0) return;
        paint.setTextSize(size);
        metrics = paint.getFontMetrics();
        paint.setColor(ColorPolicy.withAlpha(foreground, .45f));
        canvas.drawText(hint, getWidth() / 2f, getHeight() / 2f - (metrics.ascent + metrics.descent) / 2f, paint);
    }

    private void drawTrail(Canvas canvas) {
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
