package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import java.util.function.IntSupplier;

/**
 * Draws the tintable MSIME brand mark without baking a white square into the shortcut bar.
 *
 * <p>新设计里品牌键是 30 dp 的 logoCirc 圆盘托着 18 dp 的 logoBg 标记，功能面板或任何面板打开时整个 40 dp 按钮垫一块圆角 12 dp 的 accentSoft。调用 {@link #setLogoColors} 之后按新设计画；没调用时保持原来只画一枚 accent 标记的样子，旧调用点不受影响。节点 text（如「更多」）照常保留给无障碍，不绘制。
 */
public final class KeyboardBrandButton extends KeyboardPressButton {
    /**
     * The mark's share of its touch target.
     *
     * <p>Larger than the stroked glyphs beside it because the drawn path leaves more slack inside
     * its own 110-unit box; the two read at the same weight in the flat toolbar.
     */
    private static final float MARK_SCALE = 0.72f;
    private static final float DISC_DP = 30f;
    private static final float DISC_MARK_DP = 18f;
    private static final float ACTIVE_SIDE_DP = 40f;
    private static final float ACTIVE_RADIUS_DP = 12f;

    private final IntSupplier accent;
    private final Paint mark = KeyboardBrandMark.newPaint();
    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF bounds = new RectF();
    private boolean designed;
    private int discColor = Color.TRANSPARENT;
    private int markColor = Color.TRANSPARENT;
    private boolean panelOpen;
    private int panelOpenFill = Color.TRANSPARENT;

    public KeyboardBrandButton(Context context, IntSupplier accent) {
        super(context);
        this.accent = accent;
        setKeyboardRole(KeyboardKeyRole.GLYPH);
        setContentDescription("更多快捷设置");
    }

    /** 新设计的配色：圆盘 logoCirc、标记 logoBg（都由调用方从皮肤取）。 */
    public void setLogoColors(int disc, int markFill) {
        if (designed && discColor == disc && markColor == markFill) return;
        designed = true;
        discColor = disc;
        markColor = markFill;
        invalidate();
    }

    /** 面板打开时按钮底垫 accentSoft；{@code open} 为假时不垫。 */
    public void setPanelOpen(boolean open, int accentSoft) {
        if (panelOpen == open && panelOpenFill == accentSoft) return;
        panelOpen = open;
        panelOpenFill = accentSoft;
        invalidate();
    }

    public boolean isPanelOpen() { return panelOpen; }

    @Override protected void onDraw(Canvas canvas) {
        int width = Math.max(0, getWidth() - getPaddingLeft() - getPaddingRight());
        int height = Math.max(0, getHeight() - getPaddingTop() - getPaddingBottom());
        if (designed) {
            drawDesigned(canvas, width, height);
            return;
        }
        // The text remains available to accessibility and device smoke tests, while the visible
        // shortcut is the same mark Apple turns into an accent-tinted template.
        int color;
        try {
            color = accent.getAsInt();
        } catch (RuntimeException error) {
            color = Color.WHITE;
        }
        mark.setColor(color);
        mark.setAlpha(isEnabled() ? 255 : 96);
        KeyboardBrandMark.draw(canvas, mark, getPaddingLeft(), getPaddingTop(), width, height,
            MARK_SCALE);
    }

    private void drawDesigned(Canvas canvas, int width, int height) {
        float density = getResources().getDisplayMetrics().density;
        float shorter = Math.min(width, height);
        if (shorter <= 0) return;
        float centerX = getPaddingLeft() + width / 2f;
        float centerY = getPaddingTop() + height / 2f;
        if (panelOpen && Color.alpha(panelOpenFill) > 0) {
            float side = Math.min(shorter, ACTIVE_SIDE_DP * density);
            bounds.set(centerX - side / 2f, centerY - side / 2f, centerX + side / 2f,
                centerY + side / 2f);
            fill.setColor(panelOpenFill);
            float radius = Math.min(side / 2f, ACTIVE_RADIUS_DP * density);
            canvas.drawRoundRect(bounds, radius, radius, fill);
        }
        float disc = Math.min(shorter, DISC_DP * density);
        fill.setColor(discColor);
        canvas.drawCircle(centerX, centerY, disc / 2f, fill);
        float markSize = disc * (DISC_MARK_DP / DISC_DP);
        mark.setColor(markColor);
        mark.setAlpha(isEnabled() ? Color.alpha(markColor) : 96);
        KeyboardBrandMark.draw(canvas, mark, centerX - markSize / 2f, centerY - markSize / 2f,
            markSize, markSize, 1f);
    }
}
