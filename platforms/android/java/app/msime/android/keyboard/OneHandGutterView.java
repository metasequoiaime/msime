package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.view.Gravity;
import android.widget.Button;
import android.widget.LinearLayout;
import app.msime.android.KeyboardGeometry;

/**
 * 单手模式的侧栏：键区缩到 85% 宽，剩下 15% 放两个按钮——‹ 换到另一侧（40 dp 的 kbKey 圆）和 ⤢ 退出单手模式。
 *
 * <p>键盘靠右时侧栏在左、箭头朝左；靠左时侧栏在右、箭头朝右（{@link #setKeyboardOnRight}）。颜色由调用方从皮肤传入。
 */
public final class OneHandGutterView extends LinearLayout {
    /** 侧栏占键盘总宽的比例。 */
    public static final float GUTTER_FRACTION = 0.15f;
    public static final float BUTTON_DP = 40f;
    public static final float ICON_DP = 22f;

    private final GutterButton swap;
    private final GutterButton exit;

    public OneHandGutterView(Context context) {
        super(context);
        setOrientation(VERTICAL);
        setGravity(Gravity.CENTER);
        swap = new GutterButton(context, KeyboardIconPaths.Icon.SWAP_SIDE, true);
        swap.setContentDescription("单手键盘换到另一侧");
        exit = new GutterButton(context, KeyboardIconPaths.Icon.EXIT_ONE_HAND, false);
        exit.setContentDescription("退出单手模式");
        int size = KeyboardGeometry.pixels(context, BUTTON_DP);
        LinearLayout.LayoutParams swapParams = new LinearLayout.LayoutParams(size, size);
        LinearLayout.LayoutParams exitParams = new LinearLayout.LayoutParams(size, size);
        exitParams.topMargin = KeyboardGeometry.pixels(context, 24);
        addView(swap, swapParams);
        addView(exit, exitParams);
    }

    /** 侧栏宽度（像素）：总宽的 15%。 */
    public static int gutterWidth(int totalWidth) {
        return Math.round(BoundsPolicy.nonNegative(totalWidth) * GUTTER_FRACTION);
    }

    /** 键区宽度（像素）：总宽减去侧栏。 */
    public static int keysWidth(int totalWidth) {
        return BoundsPolicy.nonNegative(totalWidth) - gutterWidth(totalWidth);
    }

    public void setOnSwap(Runnable action) { swap.setOnClickListener(view -> action.run()); }

    public void setOnExit(Runnable action) { exit.setOnClickListener(view -> action.run()); }

    /** 键盘在右侧时箭头朝左（指向要换去的一侧）。 */
    public void setKeyboardOnRight(boolean onRight) {
        swap.mirrored = !onRight;
        swap.invalidate();
    }

    /** 圆按钮底色（kbKey）与图标色（kbSub）。 */
    public void setColors(int keyBackground, int iconColor) {
        swap.fillColor = keyBackground;
        swap.iconColor = iconColor;
        exit.fillColor = Color.TRANSPARENT;
        exit.iconColor = iconColor;
        swap.invalidate();
        exit.invalidate();
    }

    public Button swapButton() { return swap; }

    public Button exitButton() { return exit; }

    private static final class GutterButton extends Button {
        private final KeyboardIconPaths.Icon icon;
        private final boolean round;
        private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Paint stroke = new Paint(Paint.ANTI_ALIAS_FLAG);
        private int fillColor = Color.WHITE;
        private int iconColor = Color.GRAY;
        private boolean mirrored;

        GutterButton(Context context, KeyboardIconPaths.Icon icon, boolean round) {
            super(context);
            this.icon = icon;
            this.round = round;
            setBackground(null);
            setPadding(0, 0, 0, 0);
            setMinWidth(0);
            setMinimumWidth(0);
            setMinHeight(0);
            setMinimumHeight(0);
        }

        @Override public void setPressed(boolean pressed) {
            boolean changed = pressed != isPressed();
            super.setPressed(pressed);
            if (changed) {
                setScaleX(pressed ? .92f : 1f);
                setScaleY(pressed ? .92f : 1f);
            }
        }

        @Override protected void onDraw(Canvas canvas) {
            float cx = getWidth() / 2f;
            float cy = getHeight() / 2f;
            if (round && Color.alpha(fillColor) > 0) {
                fill.setColor(fillColor);
            canvas.drawCircle(cx, cy, KeyboardGeometry.shorterSide(getWidth(), getHeight()) / 2f, fill);
            }
            float size = KeyboardGeometry.floatPixels(getContext(), ICON_DP);
            int saved = canvas.save();
            if (mirrored) canvas.scale(-1f, 1f, cx, cy);
            KeyboardIconPaths.draw(canvas, stroke, icon, cx - size / 2f, cy - size / 2f, size,
                iconColor);
            canvas.restoreToCount(saved);
        }
    }
}
