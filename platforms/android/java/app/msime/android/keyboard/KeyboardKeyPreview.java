package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.view.View;

/**
 * 按键气泡：字母键按下时浮在键上方的预览，放在 IME 窗口内的覆盖层（FrameLayout）里，不另开 PopupWindow。
 *
 * <p>尺寸按设计令牌 §5：宽为键宽的 138%（权重大于 1.2 的键 150%），高 54 dp，底边 = 键顶 + 6 dp，圆角上 12 下 8，底色 = 键色，字 30 sp，带两层阴影。位置用 translation 设置，覆盖层用 WRAP_CONTENT 把它加在左上角即可。
 */
public final class KeyboardKeyPreview extends View {
    public static final float HEIGHT_DP = 54f;
    public static final float OVERLAP_DP = 6f;
    public static final float TEXT_SP = 30f;
    private static final float TOP_RADIUS_DP = 12f;
    private static final float BOTTOM_RADIUS_DP = 8f;
    private static final float SHADOW_MARGIN_DP = 12f;

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint outline = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path shape = new Path();
    private final RectF rect = new RectF();
    private final float[] radii = new float[8];
    private String label = "";
    private int bubbleWidth;
    private int bubbleHeight;

    public KeyboardKeyPreview(Context context) {
        super(context);
        ViewPolicy.hideFromAccessibility(this);
        setWillNotDraw(false);
        ViewPolicy.setNonInteractive(this);
        text.setTextAlign(Paint.Align.CENTER);
        outline.setStyle(Paint.Style.STROKE);
        setVisibility(GONE);
    }

    /** 气泡宽度：键宽的 138%，权重大于 1.2 的键 150%。 */
    public static float bubbleWidth(float keyWidth, float weight) {
        return keyWidth * (weight > 1.2f ? 1.5f : 1.38f);
    }

    /** 气泡左边：以键中心对齐，再夹进父容器 [0, parentWidth - bubbleWidth]。 */
    public static float bubbleLeft(float keyLeft, float keyWidth, float bubbleWidth,
            float parentWidth) {
        float left = keyLeft + keyWidth / 2f - bubbleWidth / 2f;
        float max = BoundsPolicy.nonNegative(parentWidth - bubbleWidth);
        return KeyboardGeometry.bounded(left, 0f, max);
    }

    /** 气泡顶边：底边落在键顶以下 {@code overlap} 处。 */
    public static float bubbleTop(float keyTop, float height, float overlap) {
        return keyTop + overlap - height;
    }

    /** 气泡底色、文字色与描边色（设计里是 0.5 dp 的 rgba(0,0,0,.08)）。 */
    public void setColors(int keyBackground, int foreground, int ring) {
        fill.setColor(keyBackground);
        text.setColor(foreground);
        outline.setColor(ring);
        invalidate();
    }

    /**
     * 在键的上方显示 {@code value}。坐标都在覆盖层的坐标系里（像素）。
     *
     * @param weight 键在行里的权重，决定 138% 还是 150%
     */
    public void show(String value, float keyLeft, float keyTop, float keyWidth, float weight,
            float parentWidth) {
        label = TextPolicy.emptyIfNull(value);
        float width = bubbleWidth(keyWidth, weight);
        float height = KeyboardGeometry.floatPixels(getContext(), HEIGHT_DP);
        float margin = KeyboardGeometry.floatPixels(getContext(), SHADOW_MARGIN_DP);
        int nextWidth = Math.round(width + margin * 2);
        int nextHeight = Math.round(height + margin * 2);
        if (nextWidth != bubbleWidth || nextHeight != bubbleHeight) {
            bubbleWidth = nextWidth;
            bubbleHeight = nextHeight;
            requestLayout();
        }
        setTranslationX(bubbleLeft(keyLeft, keyWidth, width, parentWidth) - margin);
        setTranslationY(bubbleTop(keyTop, height,
            KeyboardGeometry.floatPixels(getContext(), OVERLAP_DP)) - margin);
        setVisibility(VISIBLE);
        invalidate();
    }

    /** 下滑输入提示时换成提示字符，位置不变。 */
    public void setLabel(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (label.equals(next)) return;
        label = next;
        invalidate();
    }

    public String label() { return label; }

    public void hide() {
        setVisibility(GONE);
    }

    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        setMeasuredDimension(bubbleWidth, bubbleHeight);
    }

    @Override protected void onDraw(Canvas canvas) {
        float margin = KeyboardGeometry.floatPixels(getContext(), SHADOW_MARGIN_DP);
        rect.set(margin, margin, getWidth() - margin, getHeight() - margin);
        if (rect.width() <= 0 || rect.height() <= 0) return;
        float top = KeyboardGeometry.floatPixels(getContext(), TOP_RADIUS_DP);
        float bottom = KeyboardGeometry.floatPixels(getContext(), BOTTOM_RADIUS_DP);
        radii[0] = radii[1] = radii[2] = radii[3] = top;
        radii[4] = radii[5] = radii[6] = radii[7] = bottom;
        shape.reset();
        shape.addRoundRect(rect, radii, Path.Direction.CW);
        fill.setShadowLayer(KeyboardGeometry.floatPixels(getContext(), 9), 0,
            KeyboardGeometry.floatPixels(getContext(), 6), ColorPolicy.withAlpha(Color.BLACK, 56));
        canvas.drawPath(shape, fill);
        outline.setStrokeWidth(BoundsPolicy.bounded(
            KeyboardGeometry.floatPixels(getContext(), .5f), 1f, Float.MAX_VALUE));
        canvas.drawPath(shape, outline);
        if (label.isEmpty()) return;
        text.setTextSize(KeyboardGeometry.keySp(getContext(), TEXT_SP));
        Paint.FontMetrics metrics = text.getFontMetrics();
        canvas.drawText(label, rect.centerX(), rect.centerY() - (metrics.ascent + metrics.descent) / 2f,
            text);
    }
}
