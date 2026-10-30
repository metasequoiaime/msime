package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;
import android.widget.Button;

/**
 * 上屏后联想时候选条右端的关闭键（#10078）：与 {@link CandidateChevronButton} 同宽同高，左边一条 1×22 dp 的分隔线，右边 40 dp 见方的点按区，中间画 16 dp 的 ×。两个键并排时分隔线只画在 × 左边，展开键的那条由调用方关掉。
 *
 * <p>节点 text「关闭」保留（设备测试按它找）但不绘制，描述由调用方设为「关闭联想」。颜色由调用方从皮肤传入。
 */
public final class CandidateCloseButton extends Button {
    public static final float ICON_DP = 16f;
    /** 整个控件的宽度（dp），与展开键相同：分隔线 1 + 点按区 40。 */
    public static final float WIDTH_DP = CandidateChevronButton.WIDTH_DP;

    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint divider = new Paint();
    private int iconColor = Color.GRAY;
    private int hairlineColor = 0x1F000000;

    public CandidateCloseButton(Context context) {
        super(context);
        setText("关闭");
        ViewPolicy.setAllCapsFalse(this);
        ViewPolicy.clearChrome(this);
    }

    /** 关闭键只画分隔线和 ×；键盘的整树样式通道会给每个 Button 套键帽，这里挡掉。 */
    @Override public void setBackground(Drawable background) {
        super.setBackground(null);
    }

    public void setColors(int glyph, int hairline) {
        iconColor = glyph;
        hairlineColor = hairline;
        invalidate();
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        int width = DimensionPolicy.pixels(getContext(), WIDTH_DP);
        int height = DimensionPolicy.pixels(getContext(), CandidateChevronButton.BUTTON_DP);
        setMeasuredDimension(resolveSize(width, widthMeasureSpec),
            resolveSize(height, heightMeasureSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        float height = getHeight();
        float dividerHeight = BoundsPolicy.atMost(height,
            DimensionPolicy.floatPixels(getContext(), CandidateChevronButton.DIVIDER_HEIGHT_DP));
        divider.setColor(hairlineColor);
        float lineWidth = BoundsPolicy.bounded(
            DimensionPolicy.floatPixels(getContext(), 1), 1f, Float.MAX_VALUE);
        canvas.drawRect(0, (height - dividerHeight) / 2f, lineWidth,
            (height + dividerHeight) / 2f, divider);
        float areaWidth = getWidth() - lineWidth;
        float size = BoundsPolicy.atMost(DimensionPolicy.shorterSide(areaWidth, height),
            DimensionPolicy.floatPixels(getContext(), ICON_DP));
        if (size <= 0) return;
        float centerX = lineWidth + areaWidth / 2f;
        float centerY = height / 2f;
        int color = isEnabled() ? iconColor : ColorPolicy.withAlpha(iconColor, 96f / 255f);
        KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.CLOSE,
            centerX - size / 2f, centerY - size / 2f, size, color);
    }
}
