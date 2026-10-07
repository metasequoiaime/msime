package app.msime.android;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.util.TypedValue;
import android.view.Gravity;
import android.util.TypedValue;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.widget.LinearLayout;
import android.util.TypedValue;
import app.msime.android.KeyboardGeometry;

/**
 * 内联的键盘高度调整条，替换工具栏那一行：取消 | 拖动柄「上下拖动调整 · N%」| 重置 | 完成。
 *
 * <p>拖动柄是 44×5、圆角 3 的 kbFg@.35 横条加 12 sp 的说明；上拖变高、下拖变矮，范围 75–130%。「完成」是 32 dp 高的 accent 胶囊（14 sp 粗体，onAccent 字）。整条的描述是「键盘布局调整」，带 75–130 的 RangeInfo 和前后滚动动作（每次 5%），供 TalkBack 调整。颜色由调用方从皮肤传入。
 */
public final class InlineHeightBar extends LinearLayout {
    /** 高度变化与三个按钮的回调。 */
    public interface Listener {
        void onPercentChanged(int percent, boolean fromUser);
        void onCancel();
        void onReset();
        void onDone();
    }

    public static final int MIN_PERCENT = 75;
    public static final int MAX_PERCENT = 130;
    public static final int DEFAULT_PERCENT = 100;
    public static final int ACCESSIBILITY_STEP = 5;
    public static final String DESCRIPTION = "键盘布局调整";

    private final Button cancel;
    private final Button reset;
    private final Button done;
    private final Handle handle;
    private Listener listener;
    private int percent = DEFAULT_PERCENT;
    private float basePixels;

    public InlineHeightBar(Context context) {
        super(context);
        setOrientation(HORIZONTAL);
        ViewPolicy.setCenteredVertically(this);
        setContentDescription(DESCRIPTION);
        KeyboardGeometry.setHorizontalPaddingDp(this, context, 6);
        cancel = textButton(context, "取消");
        reset = textButton(context, "重置");
        done = textButton(context, "完成");
        ViewPolicy.setTypefaceStyle(done, Typeface.BOLD);
        KeyboardGeometry.setKeyTextSize(done, 14);
        handle = new Handle(context, this);
        int pill = KeyboardGeometry.pixels(context, 32);
        addView(cancel, KeyboardGeometry.linearParamsPx(LayoutParams.WRAP_CONTENT, pill));
        addView(handle, KeyboardGeometry.weightedMatchParentParams(1f));
        LinearLayout.LayoutParams resetParams = KeyboardGeometry.linearParamsPx(LayoutParams.WRAP_CONTENT, pill);
        resetParams.rightMargin = KeyboardGeometry.pixels(context, 4);
        addView(reset, resetParams);
        addView(done, KeyboardGeometry.linearParamsPx(LayoutParams.WRAP_CONTENT, pill));
        ViewPolicy.bindClick(cancel, () -> { if (listener != null) listener.onCancel(); });
        ViewPolicy.bindClick(done, () -> { if (listener != null) listener.onDone(); });
        ViewPolicy.bindClick(reset, () -> {
            updatePercent(DEFAULT_PERCENT, true);
            if (listener != null) listener.onReset();
        });
    }

    private static Button textButton(Context context, String label) {
        Button button = new BarButton(context);
        button.setText(label);
        ViewPolicy.setAllCapsFalse(button);
        ViewPolicy.clearBackground(button);
        ViewPolicy.clearMinimumSize(button);
        int horizontal = KeyboardGeometry.pixels(context, 12);
        ViewPolicy.setHorizontalPadding(button, horizontal);
        ViewPolicy.setCenteredKeyTextSizeSp(button, 14);
        return button;
    }

    /** 把百分比夹进 75–130。 */
    public static int clamp(int value) {
        return KeyboardGeometry.bounded(value, MIN_PERCENT, MAX_PERCENT);
    }

    /**
     * 拖动换算：从 {@code startPercent} 开始，手指竖直移动 {@code dy} 像素（向下为正）后的百分比。
     *
     * <p>{@code basePixels} 是 100% 时键区的高度；上拖 basePixels 的 1% 就加 1%。
     */
    public static int percentForDrag(int startPercent, float dy, float basePixels) {
        if (basePixels <= 0) return clamp(startPercent);
        return clamp(Math.round(startPercent - dy / basePixels * 100f));
    }

    /** 拖动柄下方的说明文字。 */
    public static String label(int percent) {
        return "上下拖动调整 · " + percent + "%";
    }

    public void setListener(Listener value) { listener = value; }

    /** 100% 时键区的像素高度，用于把拖动距离换成百分比。 */
    public void setBasePixels(float value) { basePixels = value; }

    public int percent() { return percent; }

    /** 外部设置当前百分比（不回调）。 */
    public void setPercent(int value) {
        int next = clamp(value);
        if (next == percent) return;
        percent = next;
        handle.invalidate();
    }

    /** 取消、重置的文字色（kbFg）、拖动柄色（kbFg，绘制时取 35%）、说明色（kbSub）、完成胶囊（accent / onAccent）。 */
    public void setColors(int foreground, int secondary, int accent, int onAccent) {
        ((BarButton) cancel).setColors(foreground, null);
        ((BarButton) reset).setColors(foreground, null);
        GradientDrawable pill = DrawablePolicy.rounded(accent,
            KeyboardGeometry.floatPixels(getContext(), 16));
        ((BarButton) done).setColors(onAccent, pill);
        handle.barColor = ColorPolicy.withAlpha(foreground, .35f);
        handle.textColor = secondary;
        handle.invalidate();
    }

    public Button cancelButton() { return cancel; }

    public Button resetButton() { return reset; }

    public Button doneButton() { return done; }

    void updatePercent(int value, boolean fromUser) {
        int next = clamp(value);
        if (next == percent) return;
        percent = next;
        handle.invalidate();
        if (listener != null) listener.onPercentChanged(percent, fromUser);
        sendAccessibilityEvent(android.view.accessibility.AccessibilityEvent.TYPE_VIEW_SCROLLED);
    }

    @SuppressWarnings("deprecation")
    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.setContentDescription(DESCRIPTION);
        info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(
            AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_INT, MIN_PERCENT, MAX_PERCENT, percent));
        info.setScrollable(true);
        if (percent < MAX_PERCENT) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD);
        }
        if (percent > MIN_PERCENT) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD);
        }
    }

    @Override public boolean performAccessibilityAction(int action, Bundle arguments) {
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_FORWARD && percent < MAX_PERCENT) {
            updatePercent(percent + ACCESSIBILITY_STEP, true);
            return true;
        }
        if (action == AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD && percent > MIN_PERCENT) {
            updatePercent(percent - ACCESSIBILITY_STEP, true);
            return true;
        }
        return super.performAccessibilityAction(action, arguments);
    }

    /**
     * 条上的文字按钮。键盘的整树样式通道每次 render 都会给每个 Button 套键帽、改字色，这里只认 {@link #setColors} 给的颜色和底，外部的改动一律挡回。
     */
    private static final class BarButton extends Button {
        private boolean own;
        private int color = Color.BLACK;
        private android.graphics.drawable.Drawable face;

        BarButton(Context context) {
            super(context);
        }

        void setColors(int text, android.graphics.drawable.Drawable background) {
            color = text;
            face = background;
            own = true;
            super.setTextColor(text);
            super.setBackground(background);
            own = false;
        }

        @Override public void setTextColor(int value) {
            super.setTextColor(own ? value : color);
        }

        @Override public void setTextColor(android.content.res.ColorStateList value) {
            super.setTextColor(color);
        }

        @Override public void setBackground(android.graphics.drawable.Drawable background) {
            super.setBackground(own ? background : face);
        }
    }

    /** 中间的拖动区：画横条与说明，把竖直拖动换成百分比。 */
    private static final class Handle extends View {
        private static final float BAR_WIDTH_DP = 44f;
        private static final float BAR_HEIGHT_DP = 5f;
        private static final float BAR_RADIUS_DP = 3f;
        private static final float LABEL_SP = 12f;

        private final InlineHeightBar bar;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF rect = new RectF();
        private int barColor = 0x59000000;
        private int textColor = Color.GRAY;
        private float downY;
        private int startPercent;

        Handle(Context context, InlineHeightBar bar) {
            super(context);
            this.bar = bar;
            text.setTextAlign(Paint.Align.CENTER);
            ViewPolicy.hideFromAccessibility(this);
        }

        // 拖动柄只换算拖动距离，没有点击语义；无障碍用户经整条的 RangeInfo 与滚动动作调整。
        @SuppressLint("ClickableViewAccessibility")
        @Override public boolean onTouchEvent(MotionEvent event) {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    downY = event.getRawY();
                    startPercent = bar.percent();
                    getParent().requestDisallowInterceptTouchEvent(true);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    bar.updatePercent(percentForDrag(startPercent, event.getRawY() - downY,
                        bar.basePixels), true);
                    return true;
                }
                case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    return true;
                }
                default -> {
                    return super.onTouchEvent(event);
                }
            }
        }

        @Override protected void onDraw(Canvas canvas) {
            text.setTextSize(KeyboardGeometry.keySp(getContext(), LABEL_SP));
            Paint.FontMetrics metrics = text.getFontMetrics();
            float textHeight = metrics.descent - metrics.ascent;
            float gap = KeyboardGeometry.floatPixels(getContext(), 6);
            float total = KeyboardGeometry.floatPixels(getContext(), BAR_HEIGHT_DP) + gap + textHeight;
            float top = (getHeight() - total) / 2f;
            float cx = getWidth() / 2f;
            float barWidth = KeyboardGeometry.floatPixels(getContext(), BAR_WIDTH_DP);
            float barHeight = KeyboardGeometry.floatPixels(getContext(), BAR_HEIGHT_DP);
            rect.set(cx - barWidth / 2f, top, cx + barWidth / 2f, top + barHeight);
            paint.setColor(barColor);
            float radius = KeyboardGeometry.floatPixels(getContext(), BAR_RADIUS_DP);
            canvas.drawRoundRect(rect, radius, radius, paint);
            text.setColor(textColor);
            canvas.drawText(label(bar.percent()), cx, rect.bottom + gap - metrics.ascent, text);
        }
    }
}
