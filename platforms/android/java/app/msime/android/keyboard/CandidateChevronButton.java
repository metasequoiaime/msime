package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;
import android.widget.Button;
import app.msime.android.KeyboardGeometry;

/**
 * 候选条右端的展开键：左边一条 1×22 dp 的 kbHair 分隔线，右边 40 dp 见方的点按区，中间画 20 dp 的 chevron；展开时 chevron 立即翻转 180°。
 *
 * <p>节点 text「展开」保留（设备测试按它找）但不绘制，描述由调用方设为「展开候选」。颜色由调用方从皮肤传入。
 */
public final class CandidateChevronButton extends Button {
    public static final float DIVIDER_HEIGHT_DP = 22f;
    public static final float BUTTON_DP = 40f;
    public static final float CHEVRON_DP = 20f;
    /** 整个控件的宽度（dp）：分隔线 1 + 点按区 40。 */
    public static final float WIDTH_DP = 1f + BUTTON_DP;

    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint divider = new Paint();
    private int iconColor = Color.GRAY;
    private int hairlineColor = 0x1F000000;
    private boolean expanded;
    private boolean dividerShown = true;
    private float rotation;

    public CandidateChevronButton(Context context) {
        super(context);
        setText("展开");
        ViewPolicy.setAllCapsFalse(this);
        ViewPolicy.clearChrome(this);
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

    /** 左边的分隔线画不画：上屏后联想的关闭键（{@link CandidateCloseButton}）排在展开键左边时，分隔线由关闭键画在两个键的最左边，展开键这条不画。 */
    public void setDividerShown(boolean shown) {
        if (dividerShown == shown) return;
        dividerShown = shown;
        invalidate();
    }

    /** 切换展开状态，立即画出最终方向。 */
    public void setExpanded(boolean value) {
        if (expanded == value) return;
        expanded = value;
        rotation = value ? 180f : 0f;
        invalidate();
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int width = DimensionPolicy.pixels(getContext(), WIDTH_DP);
        int height = DimensionPolicy.pixels(getContext(), BUTTON_DP);
        setMeasuredDimension(resolveSize(width, widthMeasureSpec),
            resolveSize(height, heightMeasureSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        float height = getHeight();
        float dividerHeight = BoundsPolicy.atMost(height,
            DimensionPolicy.floatPixels(getContext(), DIVIDER_HEIGHT_DP));
        divider.setColor(hairlineColor);
        float lineWidth = BoundsPolicy.bounded(
            DimensionPolicy.floatPixels(getContext(), 1), 1f, Float.MAX_VALUE);
        if (dividerShown)
            canvas.drawRect(0, (height - dividerHeight) / 2f, lineWidth,
                (height + dividerHeight) / 2f, divider);
        float areaLeft = lineWidth;
        float areaWidth = getWidth() - areaLeft;
        float size = BoundsPolicy.atMost(DimensionPolicy.shorterSide(areaWidth, height),
            DimensionPolicy.floatPixels(getContext(), CHEVRON_DP));
        if (size <= 0) return;
        float centerX = areaLeft + areaWidth / 2f;
        float centerY = height / 2f;
        int saved = canvas.save();
        canvas.rotate(rotation, centerX, centerY);
        int color = isEnabled() ? iconColor
            : ColorPolicy.withAlpha(iconColor, 96f / 255f);
        KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.CHEVRON,
            centerX - size / 2f, centerY - size / 2f, size, color);
        canvas.restoreToCount(saved);
    }
}
