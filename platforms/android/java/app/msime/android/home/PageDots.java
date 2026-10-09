package app.msime.android.home;

import android.animation.ValueAnimator;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.ColorPolicy;
import app.msime.android.BoundsPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.ViewPolicy;

/**
 * 分页指示点：6dp 高、圆角 3，未选中 6dp 宽、选中拉长到 16dp 并换成 accent，切换时宽度和颜色在 0.2 秒里过渡。
 *
 * <p>未选中点是正文色的 12 % 透明度，和设计的 kbHair 一致，深浅模式都只靠主题属性。
 */
public final class PageDots extends View {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF rect = new RectF();
    private final int dot;
    private final int active;
    private final int gap;
    private int count;
    private int selected;
    /** 0–1：从上一个选中点过渡到当前选中点的进度。 */
    private float progress = 1f;
    private int previous;
    @Nullable private ValueAnimator animator;

    public PageDots(Context context) {
        this(context, null);
    }

    public PageDots(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        dot = Ui.dp(context, Ui.DOT_SIZE);
        active = Ui.dp(context, Ui.DOT_ACTIVE_WIDTH);
        gap = Ui.dp(context, Ui.DOT_GAP);
        ViewPolicy.setImportantForAccessibility(this, IMPORTANT_FOR_ACCESSIBILITY_YES);
    }

    public void setCount(int count) {
        this.count = BoundsPolicy.nonNegative(count);
        selected = KeyboardGeometry.bounded(selected, 0, BoundsPolicy.nonNegative(this.count - 1));
        previous = selected;
        progress = 1f;
        updateDescription();
        requestLayout();
        invalidate();
    }

    /** 换选中页；`animate` 为假时直接跳到终态（例如首次布局或恢复状态）。 */
    public void setCurrent(int index, boolean animate) {
        if (index < 0 || index >= count || index == selected) return;
        if (animator != null) animator.cancel();
        previous = selected;
        selected = index;
        updateDescription();
        if (!animate) {
            progress = 1f;
            invalidate();
            return;
        }
        animator = ValueAnimator.ofFloat(0f, 1f);
        animator.setDuration(Ui.DOT_MILLIS);
        animator.setInterpolator(Ui.emphasized());
        animator.addUpdateListener(animation -> {
            progress = (float) animation.getAnimatedValue();
            invalidate();
        });
        animator.start();
    }

    private void updateDescription() {
        setContentDescription(count == 0 ? null : "第 " + (selected + 1) + " 页，共 " + count + " 页");
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        int width = count == 0 ? 0 : active + (count - 1) * (dot + gap);
        setMeasuredDimension(resolveSize(width + getPaddingLeft() + getPaddingRight(), widthSpec),
            resolveSize(dot + getPaddingTop() + getPaddingBottom(), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        if (count == 0) return;
        Context context = getContext();
        int accent = Ui.accent(context);
        int rest = ColorPolicy.withAlpha(Ui.text(context), 0.12f);
        float total = active + (count - 1) * (dot + gap);
        float x = getPaddingLeft() + (getWidth() - getPaddingLeft() - getPaddingRight() - total) / 2f;
        float top = getPaddingTop() + (getHeight() - getPaddingTop() - getPaddingBottom() - dot) / 2f;
        for (int i = 0; i < count; i++) {
            float weight = i == selected ? progress : i == previous ? 1f - progress : 0f;
            float width = dot + (active - dot) * weight;
            rect.set(x, top, x + width, top + dot);
            paint.setColor(ColorPolicy.blend(rest, accent, weight));
            canvas.drawRoundRect(rect, dot / 2f, dot / 2f, paint);
            x += width + gap;
        }
    }

    @Override protected void onDetachedFromWindow() {
        if (animator != null) animator.cancel();
        super.onDetachedFromWindow();
    }
}
