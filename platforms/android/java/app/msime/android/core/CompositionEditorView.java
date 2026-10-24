package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.widget.FrameLayout;
import java.util.function.IntConsumer;

/** 放大拼写与偏移手柄；拖动使用相对位移，边缘持续移动时滚动到长拼写两端。 */
final class CompositionEditorView extends FrameLayout {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint pointerPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path pointerTip = new Path();
    private final IntConsumer move;
    private final View text;
    private final View handle;
    private final int lineHeight;
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
        setWillNotDraw(false);
        paint.setTextSize(android.util.TypedValue.applyDimension(android.util.TypedValue.COMPLEX_UNIT_SP,
            26, getResources().getDisplayMetrics()));
        Paint.FontMetrics metrics = paint.getFontMetrics();
        lineHeight = Math.max(dp(50), (int) Math.ceil(metrics.descent - metrics.ascent) + dp(12));
        text = new View(context) {
            @Override protected void onDraw(Canvas canvas) {
                canvas.save();
                canvas.clipRect(dp(12), 0, getWidth() - dp(12), getHeight());
                canvas.drawText(display, dp(16) - scroll, dp(6) - paint.getFontMetrics().ascent, paint);
                if (SystemClock.uptimeMillis() % 1000 < 600) {
                    int color = paint.getColor();
                    paint.setColor(accent);
                    float x = dp(16) + widthTo(offset) - scroll;
                    canvas.drawRect(x, dp(4), x + dp(2), lineHeight - dp(4), paint);
                    paint.setColor(color);
                }
                canvas.restore();
                if (isShown()) postInvalidateDelayed(180);
            }

            @Override public boolean onTouchEvent(MotionEvent event) {
                if (event.getActionMasked() == MotionEvent.ACTION_UP) {
                    request(nearest(event.getX() - dp(16) + scroll));
                    performClick();
                }
                return true;
            }

            @Override public boolean performClick() { super.performClick(); return true; }

            @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(info);
                info.setText(display);
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
        };
        text.setContentDescription("放大拼写");
        text.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        addView(text, new FrameLayout.LayoutParams(LayoutParams.MATCH_PARENT, lineHeight));
        handle = new View(context) {
            @Override protected void onDraw(Canvas canvas) {
                pointerPaint.setStyle(Paint.Style.FILL);
                pointerPaint.setColor(accent);
                float center = getWidth() / 2f;
                canvas.drawCircle(center, getHeight() / 2f, dp(isPressed() || isFocused() ? 26 : 24), pointerPaint);
                pointerPaint.setColor(paint.getColor());
                pointerPaint.setStrokeWidth(dp(2));
                float y = getHeight() / 2f;
                canvas.drawLine(center - dp(11), y, center + dp(11), y, pointerPaint);
                canvas.drawLine(center - dp(11), y, center - dp(6), y - dp(5), pointerPaint);
                canvas.drawLine(center - dp(11), y, center - dp(6), y + dp(5), pointerPaint);
                canvas.drawLine(center + dp(11), y, center + dp(6), y - dp(5), pointerPaint);
                canvas.drawLine(center + dp(11), y, center + dp(6), y + dp(5), pointerPaint);
            }

            @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(info);
                info.setClassName(Button.class.getName());
            }

            @Override public boolean performClick() { super.performClick(); return true; }
        };
        handle.setFocusable(true);
        handle.setContentDescription("拼写光标拖动手柄");
        handle.setOnTouchListener((view, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    dragging = true;
                    handle.setPressed(true);
                    downX = touchX = event.getRawX();
                    dragOrigin = widthTo(offset);
                    getParent().requestDisallowInterceptTouchEvent(true);
                    postDelayed(edge, 90);
                }
                case MotionEvent.ACTION_MOVE -> {
                    touchX = event.getRawX();
                    request(nearest(dragOrigin + touchX - downX));
                }
                case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    dragging = false;
                    handle.setPressed(false);
                    removeCallbacks(edge);
                    getParent().requestDisallowInterceptTouchEvent(false);
                    if (event.getActionMasked() == MotionEvent.ACTION_UP) view.performClick();
                }
                default -> { }
            }
            return true;
        });
        handle.setOnClickListener(ignored -> text.announceForAccessibility("光标位置 " + offset));
        FrameLayout.LayoutParams params = new FrameLayout.LayoutParams(dp(56), dp(56));
        params.topMargin = lineHeight + dp(32);
        params.bottomMargin = dp(4);
        addView(handle, params);
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (display.isEmpty()) return;
        float x = dp(16) + widthTo(offset) - scroll;
        float y = lineHeight - dp(4);
        pointerPaint.setColor(accent);
        pointerPaint.setStyle(Paint.Style.FILL);
        pointerPaint.setStrokeWidth(dp(3));
        // 手柄中心与光标下缘在两个方向等距，右端通过滚动预留完整触摸区。
        canvas.drawLine(x, y, x + dp(64), y + dp(64), pointerPaint);
        pointerTip.rewind();
        pointerTip.moveTo(x, y);
        pointerTip.lineTo(x + dp(16), y + dp(5));
        pointerTip.lineTo(x + dp(5), y + dp(16));
        pointerTip.close();
        canvas.drawPath(pointerTip, pointerPaint);
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
        text.invalidate();
        if (textChanged || previousOffset != offset) text.sendAccessibilityEvent(textChanged
            ? android.view.accessibility.AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED
            : android.view.accessibility.AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        super.onSizeChanged(width, height, oldWidth, oldHeight);
        place();
    }

    @Override protected void onVisibilityChanged(View changed, int visibility) {
        super.onVisibilityChanged(changed, visibility);
        if (visibility != View.VISIBLE) {
            dragging = false;
            if (handle != null) handle.setPressed(false);
            removeCallbacks(edge);
        }
    }

    @Override protected void onDetachedFromWindow() {
        dragging = false;
        removeCallbacks(edge);
        super.onDetachedFromWindow();
    }

    private void place() {
        if (getWidth() <= 0 || handle == null) return;
        float caret = widthTo(offset);
        float right = Math.max(dp(20), getWidth() - dp(112));
        scroll = Math.max(0, Math.min(scroll, caret));
        if (caret - scroll > right) scroll = caret - right;
        if (caret - scroll < dp(12)) scroll = Math.max(0, caret - dp(12));
        handle.setTranslationX(dp(16) + caret - scroll + dp(36));
        handle.invalidate();
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
