package app.msime.android;

import app.msime.android.LayoutPolicy;


import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.view.MotionEvent;
import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.widget.LinearLayout;

/**
 * 浮动键盘顶部的拖动条（#5621）：中间是 36×4 的横条，按住整条拖动移动键盘；右端是「停靠」键（Lucide maximize-2 图标），点它回到停靠在底部的完整键盘。只在浮动时由服务显示，颜色由调用方从皮肤传入。
 *
 * <p>两个子视图都不是 {@link Button}：键盘的整树样式通道会给每个 Button 套键帽、改字色，这一条不该长成按键。
 */
public final class FloatingKeyboardBar extends LinearLayout {
    /** 拖动与停靠的回调；位移是相对按下那一刻的原始像素，向右、向下为正。 */
    public interface Listener {
        void dragStarted();
        void dragged(float dx, float dy);
        void dragEnded();
        void dock();
    }

    public static final String DESCRIPTION = "浮动键盘：按住拖动移动位置";
    public static final String DOCK_DESCRIPTION = "停靠键盘";

    private final Grip grip;
    private final DockButton dock;
    private Listener listener;

    public FloatingKeyboardBar(Context context) {
        super(context);
        setOrientation(HORIZONTAL);
        ViewPolicy.setCenteredVertically(this);
        grip = new Grip(context, this);
        dock = new DockButton(context);
        addView(grip, LayoutPolicy.weightedMatchParentParams(1f));
        addView(dock, LayoutPolicy.linearParamsPx(DimensionPolicy.pixels(context, 40),
            LayoutParams.MATCH_PARENT));
        ViewPolicy.bindClick(dock, () -> { if (listener != null) listener.dock(); });
    }

    public void setListener(Listener value) { listener = value; }

    /** 横条与停靠图标的颜色（kbFg；横条绘制时取 35%）。 */
    public void setColors(int foreground) {
        grip.color = ColorPolicy.withAlpha(foreground, .35f);
        dock.color = foreground;
        grip.invalidate();
        dock.invalidate();
    }

    /** 拖动区：把按下后的位移原样交给监听者，由服务换算、钳制并移动面板。 */
    private static final class Grip extends View {
        private static final float BAR_WIDTH_DP = 36f;
        private static final float BAR_HEIGHT_DP = 4f;

        private final FloatingKeyboardBar bar;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF rect = new RectF();
        private int color = 0x59000000;
        private float downX;
        private float downY;

        Grip(Context context, FloatingKeyboardBar bar) {
            super(context);
            this.bar = bar;
            setContentDescription(DESCRIPTION);
        }

        // 拖动区只换算位移，没有点击语义。
        @SuppressLint("ClickableViewAccessibility")
        @Override public boolean onTouchEvent(MotionEvent event) {
            Listener listener = bar.listener;
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    downX = event.getRawX();
                    downY = event.getRawY();
                    getParent().requestDisallowInterceptTouchEvent(true);
                    if (listener != null) listener.dragStarted();
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (listener != null) listener.dragged(event.getRawX() - downX, event.getRawY() - downY);
                    return true;
                }
                case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                    if (listener != null) listener.dragEnded();
                    return true;
                }
                default -> {
                    return super.onTouchEvent(event);
                }
            }
        }

        @Override protected void onDraw(Canvas canvas) {
            float width = DimensionPolicy.floatPixels(getContext(), BAR_WIDTH_DP);
            float height = DimensionPolicy.floatPixels(getContext(), BAR_HEIGHT_DP);
            float cx = getWidth() / 2f;
            float cy = getHeight() / 2f;
            rect.set(cx - width / 2f, cy - height / 2f, cx + width / 2f, cy + height / 2f);
            paint.setColor(color);
            canvas.drawRoundRect(rect, height / 2f, height / 2f, paint);
        }
    }

    /** 停靠键：画 18 dp 的 maximize-2 图标，无障碍里是一个名为「停靠键盘」的按钮。 */
    private static final class DockButton extends View {
        private static final float ICON_DP = 18f;
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private int color = Color.GRAY;

        DockButton(Context context) {
            super(context);
            setClickable(true);
            setFocusable(true);
            setContentDescription(DOCK_DESCRIPTION);
        }

        @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
            super.onInitializeAccessibilityNodeInfo(info);
            info.setClassName(Button.class.getName());
        }

        @Override protected void onDraw(Canvas canvas) {
            float size = BoundsPolicy.atMost(
                DimensionPolicy.floatPixels(getContext(), ICON_DP),
                BoundsPolicy.atMost(getWidth(), getHeight()));
            KeyboardIconPaths.draw(canvas, paint, KeyboardIconPaths.Icon.EXIT_ONE_HAND,
                (getWidth() - size) / 2f, (getHeight() - size) / 2f, size, color);
        }
    }
}
