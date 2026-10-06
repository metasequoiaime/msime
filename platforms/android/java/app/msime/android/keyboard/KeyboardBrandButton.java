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
    /** 设计是 18 dp；圆盘与浅色键盘底色接近时 18 dp 的 logo 比旁边 24 dp 的工具栏图标小一圈，按用户要求放大到同为 24 dp。 */
    private static final float DISC_MARK_DP = 24f;
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

    /** 新设计的配色：圆盘 logoCirc、logo 方框 logoBg（调用方从应用主题的季节色取，与设计一致，不随键盘皮肤变）。 */
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
        int width = KeyboardGeometry.contentWidth(this);
        int height = KeyboardGeometry.contentHeight(this);
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
        float shorter = KeyboardGeometry.shorterSide(width, height);
        if (shorter <= 0) return;
        float centerX = getPaddingLeft() + width / 2f;
        float centerY = getPaddingTop() + height / 2f;
        if (panelOpen && Color.alpha(panelOpenFill) > 0) {
            float side = BoundsPolicy.atMost(shorter,
                KeyboardGeometry.floatPixels(getContext(), ACTIVE_SIDE_DP));
            bounds.set(centerX - side / 2f, centerY - side / 2f, centerX + side / 2f,
                centerY + side / 2f);
            fill.setColor(panelOpenFill);
            float radius = BoundsPolicy.atMost(side / 2f,
                KeyboardGeometry.floatPixels(getContext(), ACTIVE_RADIUS_DP));
            canvas.drawRoundRect(bounds, radius, radius, fill);
        }
        float disc = BoundsPolicy.atMost(shorter,
            KeyboardGeometry.floatPixels(getContext(), DISC_DP));
        fill.setColor(discColor);
        canvas.drawCircle(centerX, centerY, disc / 2f, fill);
        float markSize = disc * (DISC_MARK_DP / DISC_DP);
        drawOfficialLogo(canvas, centerX, centerY, markSize);
    }

    /** 官方 logo 的画布（msime_frame.svg 的 viewBox）。 */
    private static final float LOGO_WIDTH = 116f;
    private static final float LOGO_HEIGHT = 132f;
    private final Paint stroke = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final android.graphics.Path logoStroke = officialStroke();

    /** 官方笔画，逐段照抄设计与 packages/ui 用的 path：M80.394 18.8335 L34.3451 36.489 L80.394 49.7306 L34.3451 71.7999 C77.8789 79.1564 118.8 85.1887 31.8431 113.088。 */
    private static android.graphics.Path officialStroke() {
        android.graphics.Path path = new android.graphics.Path();
        path.moveTo(80.394f, 18.8335f);
        path.lineTo(34.3451f, 36.489f);
        path.lineTo(80.394f, 49.7306f);
        path.lineTo(34.3451f, 71.7999f);
        path.cubicTo(77.8789f, 79.1564f, 118.8f, 85.1887f, 31.8431f, 113.088f);
        return path;
    }

    /**
     * 画官方 logo，不再自己画一枚近似的标：logoBg 填满 msime_frame.svg 的方框（M5.84314 5.8335H109.843V125.833H5.84314Z，与 packages/ui 一样去掉笔刷纹理），上面是 9 宽、圆头的白色官方笔画；按 116:132 等比放进 {@code size} 见方的区域中央。
     */
    private void drawOfficialLogo(Canvas canvas, float centerX, float centerY, float size) {
        float scale = size / LOGO_HEIGHT;
        int saved = canvas.save();
        canvas.translate(centerX - LOGO_WIDTH * scale / 2f, centerY - LOGO_HEIGHT * scale / 2f);
        canvas.scale(scale, scale);
        fill.setColor(markColor);
        fill.setAlpha(isEnabled() ? Color.alpha(markColor) : 96);
        canvas.drawRect(5.84314f, 5.8335f, 109.843f, 125.833f, fill);
        fill.setAlpha(255);
        stroke.setStyle(Paint.Style.STROKE);
        stroke.setStrokeWidth(9f);
        stroke.setStrokeCap(Paint.Cap.ROUND);
        stroke.setStrokeJoin(Paint.Join.ROUND);
        stroke.setColor(Color.WHITE);
        stroke.setAlpha(isEnabled() ? 255 : 96);
        canvas.drawPath(logoStroke, stroke);
        canvas.restoreToCount(saved);
    }
}
