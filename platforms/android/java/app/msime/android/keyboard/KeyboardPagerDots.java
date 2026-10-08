package app.msime.android;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.view.View;
import app.msime.android.KeyboardGeometry;
import app.msime.android.BoundsPolicy;

/**
 * 分页面板下方的页点：6 dp 高，当前页 16 dp 宽 accent，其余 6 dp 宽 kbHair，间距 6 dp，切页时宽度与颜色用 200 ms 过渡。
 */
public final class KeyboardPagerDots extends View {
    public static final float DOT_DP = 6f;
    public static final float ACTIVE_DP = 16f;
    public static final float GAP_DP = 6f;
    private static final long TRANSITION_MS = 200L;

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF rect = new RectF();
    private int count;
    private int active;
    private int previous;
    private float progress = 1f;
    private int activeColor = 0xFF000000;
    private int inactiveColor = 0x1F000000;
    private ValueAnimator animator;

    public KeyboardPagerDots(Context context) {
        super(context);
        ViewPolicy.hideFromAccessibility(this);
    }

    /** 页点总宽（dp）：一个活动点加 count-1 个普通点和间距。 */
    public static float totalWidthDp(int count) {
        if (count <= 0) return 0f;
        return ACTIVE_DP + (count - 1) * DOT_DP + (count - 1) * GAP_DP;
    }

    public void setColors(int activeDot, int inactiveDot) {
        activeColor = activeDot;
        inactiveColor = inactiveDot;
        invalidate();
    }

    public void setCount(int value) {
        int next = BoundsPolicy.nonNegative(value);
        if (count == next) return;
        count = next;
        active = KeyboardGeometry.bounded(active, 0, BoundsPolicy.nonNegative(count - 1));
        previous = active;
        progress = 1f;
        ViewPolicy.setVisible(this, count > 1);
        requestLayout();
        invalidate();
    }

    public int count() { return count; }

    public int active() { return active; }

    public void setActive(int value, boolean animate) {
        int next = KeyboardGeometry.bounded(value, 0, BoundsPolicy.nonNegative(count - 1));
        if (next == active) return;
        previous = active;
        active = next;
        if (animator != null) animator.cancel();
        if (!animate || !isAttachedToWindow()) {
            progress = 1f;
            invalidate();
            return;
        }
        progress = 0f;
        ValueAnimator nextAnimator = ValueAnimator.ofFloat(0f, 1f);
        nextAnimator.setDuration(TRANSITION_MS);
        nextAnimator.addUpdateListener(update -> {
            progress = (Float) update.getAnimatedValue();
            invalidate();
        });
        animator = nextAnimator;
        nextAnimator.start();
    }

    @Override protected void onDetachedFromWindow() {
        if (animator != null) animator.cancel();
        animator = null;
        progress = 1f;
        super.onDetachedFromWindow();
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int width = KeyboardGeometry.pixels(getContext(), totalWidthDp(count));
        int height = KeyboardGeometry.pixels(getContext(), DOT_DP);
        setMeasuredDimension(resolveSize(width, widthMeasureSpec),
            resolveSize(height, heightMeasureSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        if (count <= 0) return;
        float dot = KeyboardGeometry.floatPixels(getContext(), DOT_DP);
        float wide = KeyboardGeometry.floatPixels(getContext(), ACTIVE_DP);
        float gap = KeyboardGeometry.floatPixels(getContext(), GAP_DP);
        float total = KeyboardGeometry.floatPixels(getContext(), totalWidthDp(count));
        float x = (getWidth() - total) / 2f;
        float top = (getHeight() - dot) / 2f;
        for (int index = 0; index < count; index++) {
            float weight = index == active ? progress : index == previous ? 1f - progress : 0f;
            float width = dot + (wide - dot) * weight;
            paint.setColor(ColorPolicy.blend(inactiveColor, activeColor,
                KeyboardGeometry.bounded(weight, 0f, 1f)));
            rect.set(x, top, x + width, top + dot);
            canvas.drawRoundRect(rect, dot / 2f, dot / 2f, paint);
            x += width + gap;
        }
    }

}
