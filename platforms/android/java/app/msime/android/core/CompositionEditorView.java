package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.view.accessibility.AccessibilityNodeInfo;
import java.util.function.IntConsumer;

/**
 * 放大拼写条：一行放大的拼写和闪动光标。点一下把光标放到点中的字母前；在条上左右拖动按手指的位移移动光标，手指不用盖在光标上，拖到条的两端时持续移动并滚动长拼写。
 */
final class CompositionEditorView extends View {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final IntConsumer move;
    private final int lineHeight;
    private final int touchSlop;
    private String display = "";
    private String editing = "";
    private int offset;
    private int accent;
    private float scroll;
    private float downX;
    private float dragOrigin;
    private float touchX;
    private boolean dragging;
    private final Runnable edge = new Runnable() {
        @Override public void run() {
            if (!dragging || !isShown()) return;
            int[] location = new int[2];
            getLocationOnScreen(location);
            float x = touchX - location[0];
            int direction = x < dp(28) ? -1 : x > getWidth() - dp(28) ? 1 : 0;
            if (direction != 0) {
                // 音节分隔符不占按键，按显示下标逐格后退会停在同一个分隔符上。
                int current = CompositionCaretPolicy.tapTarget(0, display, editing, -1, offset);
                int next = Math.max(0, Math.min(editing.length(), current + direction));
                if (next != current) move.accept(next);
                dragOrigin = widthTo(offset);
                downX = touchX;
            }
            postDelayed(this, 90);
        }
    };

    CompositionEditorView(Context context, IntConsumer move) {
        super(context);
        this.move = move;
        touchSlop = ViewConfiguration.get(context).getScaledTouchSlop();
        paint.setTextSize(android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
            22, getResources().getDisplayMetrics()));
        Paint.FontMetrics metrics = paint.getFontMetrics();
        lineHeight = Math.max(dp(44), (int) Math.ceil(metrics.descent - metrics.ascent) + dp(10));
        setContentDescription("放大拼写");
        setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        setMeasuredDimension(getDefaultSize(getSuggestedMinimumWidth(), widthMeasureSpec),
            resolveSize(lineHeight, heightMeasureSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        canvas.save();
        canvas.clipRect(dp(4), 0, getWidth() - dp(4), getHeight());
        float baseline = (getHeight() - paint.getFontMetrics().ascent - paint.getFontMetrics().descent) / 2f;
        canvas.drawText(display, dp(8) - scroll, baseline, paint);
        if (dragging || SystemClock.uptimeMillis() % 1000 < 600) {
            int color = paint.getColor();
            paint.setColor(accent);
            float x = dp(8) + widthTo(offset) - scroll;
            canvas.drawRect(x, dp(8), x + dp(2), getHeight() - dp(8), paint);
            paint.setColor(color);
        }
        canvas.restore();
        if (isShown()) postInvalidateDelayed(180);
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                downX = touchX = event.getRawX();
                dragOrigin = widthTo(offset);
                getParent().requestDisallowInterceptTouchEvent(true);
            }
            case MotionEvent.ACTION_MOVE -> {
                touchX = event.getRawX();
                if (!dragging && Math.abs(touchX - downX) > touchSlop) {
                    dragging = true;
                    postDelayed(edge, 90);
                }
                if (dragging) request(nearest(dragOrigin + touchX - downX));
            }
            case MotionEvent.ACTION_UP -> {
                // 没拖动过就是一次点按：光标放到点中的字母前。
                if (!dragging) request(nearest(event.getX() - dp(8) + scroll));
                stopDragging();
                performClick();
            }
            case MotionEvent.ACTION_CANCEL -> stopDragging();
            default -> { }
        }
        return true;
    }

    @Override public boolean performClick() { super.performClick(); return true; }

    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.setText(display);
        info.setHintText("左右拖动移动光标");
        info.setTextSelection(offset, offset);
        info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_SELECTION);
    }

    @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
        if (action == AccessibilityNodeInfo.ACTION_SET_SELECTION && arguments != null) {
            int start = arguments.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, -1);
            int end = arguments.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, -1);
            if (start == end && start >= 0 && start <= display.length()) {
                request(start);
                return true;
            }
        }
        return super.performAccessibilityAction(action, arguments);
    }

    void update(String display, String editing, int caret, int foreground, int accent) {
        boolean textChanged = !this.display.equals(display);
        int previousOffset = offset;
        this.display = display;
        this.editing = editing;
        this.accent = accent;
        offset = Math.max(0, CompositionCaretPolicy.displayOffset(display, editing, caret));
        paint.setColor(foreground);
        place();
        if (textChanged || previousOffset != offset) sendAccessibilityEvent(textChanged
            ? android.view.accessibility.AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED
            : android.view.accessibility.AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        super.onSizeChanged(width, height, oldWidth, oldHeight);
        place();
    }

    @Override protected void onVisibilityChanged(View changed, int visibility) {
        super.onVisibilityChanged(changed, visibility);
        if (visibility != View.VISIBLE) stopDragging();
    }

    @Override protected void onDetachedFromWindow() {
        stopDragging();
        super.onDetachedFromWindow();
    }

    private void stopDragging() {
        dragging = false;
        removeCallbacks(edge);
        if (getParent() != null) getParent().requestDisallowInterceptTouchEvent(false);
    }

    /** 让光标留在条内：右边留出边缘拖动区，左边留一点余量。 */
    private void place() {
        if (getWidth() <= 0) return;
        float caret = widthTo(offset);
        float right = Math.max(dp(20), getWidth() - dp(40));
        scroll = Math.max(0, Math.min(scroll, caret));
        if (caret - scroll > right) scroll = caret - right;
        if (caret - scroll < dp(12)) scroll = Math.max(0, caret - dp(12));
        invalidate();
    }

    private void request(int shownOffset) {
        int target = CompositionCaretPolicy.tapTarget(0, display, editing, -1, shownOffset);
        int current = CompositionCaretPolicy.tapTarget(0, display, editing, -1, offset);
        if (target >= 0 && target != current) move.accept(target);
    }

    private float widthTo(int index) { return paint.measureText(display, 0, index); }

    private int nearest(float x) {
        for (int index = 0; index < display.length(); index++) {
            if (x < (widthTo(index) + widthTo(index + 1)) / 2) return index;
        }
        return display.length();
    }

    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
}
