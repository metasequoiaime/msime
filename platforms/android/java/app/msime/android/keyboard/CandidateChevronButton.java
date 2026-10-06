package app.msime.android;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;
import android.view.animation.DecelerateInterpolator;
import android.widget.Button;
import app.msime.android.KeyboardGeometry;

/**
 * 候选条右端的展开键：左边一条 1×22 dp 的 kbHair 分隔线，右边 40 dp 见方的点按区，中间画 20 dp 的 chevron；展开时 chevron 用 200 ms 转 180°。
 *
 * <p>节点 text「展开」保留（设备测试按它找）但不绘制，描述由调用方设为「展开候选」。颜色由调用方从皮肤传入。
 */
public final class CandidateChevronButton extends Button {
    public static final float DIVIDER_HEIGHT_DP = 22f;
    public static final float BUTTON_DP = 40f;
    public static final float CHEVRON_DP = 20f;
    /** 整个控件的宽度（dp）：分隔线 1 + 点按区 40。 */
    public static final float WIDTH_DP = 1f + BUTTON_DP;
    private static final long ROTATE_MS = 200L;

    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint divider = new Paint();
    private int iconColor = Color.GRAY;
    private int hairlineColor = 0x1F000000;
    private boolean expanded;
    private float rotation;
    private ValueAnimator animator;

    public CandidateChevronButton(Context context) {
        super(context);
        setText("展开");
        setAllCaps(false);
        setBackground(null);
        setPadding(0, 0, 0, 0);
        setMinWidth(0);
        setMinimumWidth(0);
        setMinHeight(0);
        setMinimumHeight(0);
    }

    /** 展开键只画分隔线和 chevron；键盘的整树样式通道会给每个 Button 套键帽，这里挡掉。 */
    @Override public void setBackground(Drawable background) {
        super.setBackground(null);
    }

    public void setColors(int chevron, int hairline) {
        iconColor = chevron;
        hairlineColor = hairline;
        invalidate();
    }

    public boolean isExpanded() { return expanded; }

    /** 切换展开状态；{@code animate} 为假时直接到终点（例如重建视图时恢复状态）。 */
    public void setExpanded(boolean value, boolean animate) {
        if (expanded == value && animator == null) return;
        expanded = value;
        float target = value ? 180f : 0f;
        if (animator != null) {
            animator.cancel();
            animator = null;
        }
        if (!animate || !isAttachedToWindow()) {
            rotation = target;
            invalidate();
            return;
        }
        ValueAnimator next = ValueAnimator.ofFloat(rotation, target);
        next.setDuration(ROTATE_MS);
        next.setInterpolator(new DecelerateInterpolator());
        next.addUpdateListener(update -> {
            rotation = (Float) update.getAnimatedValue();
            invalidate();
        });
        animator = next;
        next.start();
    }

    @Override protected void onDetachedFromWindow() {
        if (animator != null) {
            animator.cancel();
            animator = null;
            rotation = expanded ? 180f : 0f;
        }
        super.onDetachedFromWindow();
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int width = KeyboardGeometry.pixels(getContext(), WIDTH_DP);
        int height = KeyboardGeometry.pixels(getContext(), BUTTON_DP);
        setMeasuredDimension(resolveSize(width, widthMeasureSpec),
            resolveSize(height, heightMeasureSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        float height = getHeight();
        float dividerHeight = Math.min(height,
            KeyboardGeometry.floatPixels(getContext(), DIVIDER_HEIGHT_DP));
        divider.setColor(hairlineColor);
        float lineWidth = Math.max(1f,
            KeyboardGeometry.floatPixels(getContext(), 1));
        canvas.drawRect(0, (height - dividerHeight) / 2f, lineWidth,
            (height + dividerHeight) / 2f, divider);
        float areaLeft = lineWidth;
        float areaWidth = getWidth() - areaLeft;
        float size = Math.min(Math.min(areaWidth, height),
            KeyboardGeometry.floatPixels(getContext(), CHEVRON_DP));
        if (size <= 0) return;
        float centerX = areaLeft + areaWidth / 2f;
        float centerY = height / 2f;
        int saved = canvas.save();
        canvas.rotate(rotation, centerX, centerY);
        int color = isEnabled() ? iconColor
            : Color.argb(96, Color.red(iconColor), Color.green(iconColor), Color.blue(iconColor));
        KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.CHEVRON,
            centerX - size / 2f, centerY - size / 2f, size, color);
        canvas.restoreToCount(saved);
    }
}
