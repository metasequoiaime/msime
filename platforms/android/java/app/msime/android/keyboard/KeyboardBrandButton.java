package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;

/**
 * 工具栏的品牌键：水杉标志（{@link KeyboardBrandMark}：深色圆角正方形、绿边、白色折线，与各平台的标志相同，配色固定，不随皮肤变色）。
 *
 * <p>标志本身就是唯一的外框，不再垫圆盘：调用 {@link #useToolbarDesign} 之后，标志画在 34 dp 的标志位里 24 dp 见方的那块，功能面板或任何面板打开时整个 40 dp 按钮垫一块圆角 12 dp 的 accentSoft。没调用时标志占按钮短边的 {@link #MARK_SCALE}，也没有面板底，旧调用点不受影响。节点 text（如「更多」）照常保留给无障碍，不绘制。
 */
public final class KeyboardBrandButton extends KeyboardPressButton {
    /** 没调用 {@link #useToolbarDesign} 时，标志占按钮内容区短边的比例。 */
    private static final float MARK_SCALE = 0.72f;
    /** 标志位：以前垫圆盘的那块，随工具栏大小缩放，按钮小于它时取按钮短边。 */
    private static final float SLOT_DP = 34f;
    /** 标志在标志位里的边长，与以前圆盘里的标志同大。 */
    private static final float MARK_DP = 24f;
    private static final float ACTIVE_SIDE_DP = 40f;
    private static final float ACTIVE_RADIUS_DP = 12f;
    private static final float ACTIVE_INSET_DP = 4f;

    private final Paint markFill = KeyboardBrandMark.newFillPaint();
    private final Paint markStroke = KeyboardBrandMark.newStrokePaint();
    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF bounds = new RectF();
    private boolean designed;
    private boolean panelOpen;
    private int panelOpenFill = Color.TRANSPARENT;
    private int sizePercent = 100;

    public void setSizePercent(int percent) {
        int valid = ToolbarSizePolicy.drawPercent(percent);
        if (sizePercent == valid) return;
        sizePercent = valid;
        invalidate();
    }

    public KeyboardBrandButton(Context context) {
        super(context);
        setKeyboardRole(KeyboardKeyRole.GLYPH);
        setContentDescription("更多快捷设置");
        // 与相邻图标按钮一致，清除系统按钮内边距，避免固定触控区把品牌标记挤小。
        ViewPolicy.clearPadding(this);
    }

    /** 按工具栏的设计稿画：标志位里的 24 dp 标志，面板打开时垫底（见 {@link #setPanelOpen}）。 */
    public void useToolbarDesign() {
        if (designed) return;
        designed = true;
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
        float shorter = DimensionPolicy.shorterSide(width, height);
        if (shorter <= 0) return;
        float centerX = getPaddingLeft() + width / 2f;
        float centerY = getPaddingTop() + height / 2f;
        int alpha = ColorPolicy.enabledAlpha(isEnabled(), 255, 96);
        if (!designed) {
            // The text remains available to accessibility and device smoke tests; the visible shortcut is the mark.
            KeyboardBrandMark.draw(canvas, markFill, markStroke, centerX, centerY, shorter * MARK_SCALE, alpha);
            return;
        }
        if (panelOpen && Color.alpha(panelOpenFill) > 0) {
            // 背景与按钮边缘至少隔开 4 dp，整格仍可点击，标志保持原有位置和尺寸。
            float available = Math.max(0f, Math.min(getWidth(), getHeight())
                - DimensionPolicy.floatPixels(getContext(), ACTIVE_INSET_DP * 2));
            float side = BoundsPolicy.atMost(available,
                DimensionPolicy.floatPixels(getContext(), ACTIVE_SIDE_DP * sizePercent / 100f));
            bounds.set(centerX - side / 2f, centerY - side / 2f, centerX + side / 2f,
                centerY + side / 2f);
            fill.setColor(panelOpenFill);
            float radius = BoundsPolicy.atMost(side / 2f,
                DimensionPolicy.floatPixels(getContext(), ACTIVE_RADIUS_DP));
            canvas.drawRoundRect(bounds, radius, radius, fill);
        }
        float slot = BoundsPolicy.atMost(shorter,
            DimensionPolicy.floatPixels(getContext(), SLOT_DP * sizePercent / 100f));
        KeyboardBrandMark.draw(canvas, markFill, markStroke, centerX, centerY, slot * (MARK_DP / SLOT_DP), alpha);
    }
}
