package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import java.util.function.IntSupplier;

/**
 * 绘制随键盘皮肤配色的透明水杉品牌标记。
 *
 * <p>品牌键只画 28 dp 的线条标记，面板打开时垫一块圆角背景。调用 {@link #setLogoColor} 后跟随工具栏图标配色；节点 text（如「更多」）照常保留给无障碍，不绘制。
 */
public final class KeyboardBrandButton extends KeyboardPressButton {
    /**
     * The mark's share of its touch target.
     *
     * <p>Larger than the stroked glyphs beside it because the drawn path leaves more slack inside
     * its own 110-unit box; the two read at the same weight in the flat toolbar.
     */
    private static final float MARK_SCALE = 0.72f;
    /** 品牌标记与旁边的工具栏图标同为 28 dp，使放大后的九键界面比例协调。 */
    private static final float MARK_DP = 28f;
    private static final float ACTIVE_SIDE_DP = 40f;
    private static final float ACTIVE_RADIUS_DP = 12f;
    private static final float ACTIVE_INSET_DP = 4f;

    private final IntSupplier accent;
    private final Paint mark = KeyboardBrandMark.newPaint();
    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF bounds = new RectF();
    private boolean designed;
    private int markColor = Color.TRANSPARENT;
    private boolean panelOpen;
    private int panelOpenFill = Color.TRANSPARENT;

    public KeyboardBrandButton(Context context, IntSupplier accent) {
        super(context);
        this.accent = accent;
        setKeyboardRole(KeyboardKeyRole.GLYPH);
        setContentDescription("更多快捷设置");
    }

    /** 与相邻工具栏图标使用同一种皮肤颜色，不垫固定的季节色圆盘或方框。 */
    public void setLogoColor(int color) {
        if (designed && markColor == color) return;
        designed = true;
        markColor = color;
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
        int width = DimensionPolicy.contentWidth(this);
        int height = DimensionPolicy.contentHeight(this);
        if (designed) {
            drawDesigned(canvas, width, height);
            return;
        }
        // The text remains available to accessibility and device smoke tests, while the visible
        // shortcut is the same mark Apple turns into an accent-tinted template.
        int color = KeyboardBrandMark.colorOrWhite(accent);
        mark.setColor(color);
        mark.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 255, 96));
        KeyboardBrandMark.draw(canvas, mark, getPaddingLeft(), getPaddingTop(), width, height,
            MARK_SCALE);
    }

    private void drawDesigned(Canvas canvas, int width, int height) {
        float shorter = DimensionPolicy.shorterSide(width, height);
        if (shorter <= 0) return;
        float centerX = getPaddingLeft() + width / 2f;
        float centerY = getPaddingTop() + height / 2f;
        if (panelOpen && Color.alpha(panelOpenFill) > 0) {
            // 背景与按钮边缘至少隔开 4 dp，整格仍可点击，标志保持原有位置和尺寸。
            float available = Math.max(0f, Math.min(getWidth(), getHeight())
                - DimensionPolicy.floatPixels(getContext(), ACTIVE_INSET_DP * 2));
            float side = BoundsPolicy.atMost(available,
                DimensionPolicy.floatPixels(getContext(), ACTIVE_SIDE_DP));
            bounds.set(centerX - side / 2f, centerY - side / 2f, centerX + side / 2f,
                centerY + side / 2f);
            fill.setColor(panelOpenFill);
            float radius = BoundsPolicy.atMost(side / 2f,
                DimensionPolicy.floatPixels(getContext(), ACTIVE_RADIUS_DP));
            canvas.drawRoundRect(bounds, radius, radius, fill);
        }
        float markSize = BoundsPolicy.atMost(shorter,
            DimensionPolicy.floatPixels(getContext(), MARK_DP));
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
     * 官方笔画按 116:132 等比居中，以皮肤图标色绘制，不填充 logo 方框。
     */
    private void drawOfficialLogo(Canvas canvas, float centerX, float centerY, float size) {
        float scale = size / LOGO_HEIGHT;
        int saved = canvas.save();
        canvas.translate(centerX - LOGO_WIDTH * scale / 2f, centerY - LOGO_HEIGHT * scale / 2f);
        canvas.scale(scale, scale);
        stroke.setStyle(Paint.Style.STROKE);
        stroke.setStrokeWidth(9f);
        stroke.setStrokeCap(Paint.Cap.ROUND);
        stroke.setStrokeJoin(Paint.Join.ROUND);
        stroke.setColor(markColor);
        stroke.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), Color.alpha(markColor), 96));
        canvas.drawPath(logoStroke, stroke);
        canvas.restoreToCount(saved);
    }
}
