package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.view.View;

/**
 * The MSIME brand mark as a decorative, non-interactive view, and the one place its geometry and colours are defined.
 *
 * <p>The mark is the same on every platform: a #252525 rounded square with a #A8DF8E edge and the white stroke on it (packages/ui/src/assets/msime.svg). It is the only container - nothing draws a disc, ring or plate around it - and its colours are fixed, not the skin's. The shortcut bar's {@link KeyboardBrandButton} draws it through {@link #draw}; this view leads the candidate header the way the macOS candidate window leads its top row, so it neither takes touches nor speaks to accessibility services — the header's own text already names the input method.
 */
public final class KeyboardBrandMark extends View {
    /** 画布：从 x = -8 起的 132×132，与 packages/ui 的 brand-logo 取景相同；外框和折线都是 msime.svg（110×110）放大 1.2 倍后放进这块画布，外框边长 100 → 120。 */
    private static final float LEFT = -8f;
    private static final float SIDE = 132f;
    /** 外框：-2.15686, 5.8335 起边长 120 的正方形，圆角半径 26.4 即边长的 22%（msime.svg 的 rx 22 × 1.2），与 packages/ui 的 msimeFramePath 相同。 */
    private static final float FRAME_LEFT = -2.15686f;
    private static final float FRAME_TOP = 5.8335f;
    private static final float FRAME_RIGHT = 117.843f;
    private static final float FRAME_BOTTOM = 125.833f;
    private static final float FRAME_CORNER = 26.4f;
    /** 固定配色：深色底、绿边（画布单位，边长 120 的 5%，压在外框路径上居中）、白色折线，与 packages/ui/src/assets/msime.svg 相同；折线宽 9.6 即 msime.svg 的 8 × 1.2。 */
    private static final int FIELD = 0xFF252525;
    private static final int EDGE = 0xFFA8DF8E;
    private static final float EDGE_WIDTH = 6f;
    private static final float STROKE_WIDTH = 9.6f;
    /** msime.svg 的折线不写 linejoin，按 SVG 默认是尖角连接、miter limit 4，三个折角（约 35–42°）都画成尖角；Paint 默认 miter 也是 4，这里写明以免被改。 */
    private static final float STROKE_MITER = 4f;

    /** 官方笔画：packages/ui/src/assets/msime.svg 的折线以外框中心为原点放大 1.2 倍放进这块画布，与 packages/ui 的 msimeStrokePath 相同：M76.7775 26.4735 L38.7872 41.0393 L76.7775 51.9637 L38.7872 70.1708 C74.7026 76.2399 108.462 81.2165 36.7231 104.2335。 */
    private static final Path STROKE = new Path();

    static {
        STROKE.moveTo(76.7775f, 26.4735f);
        STROKE.lineTo(38.7872f, 41.0393f);
        STROKE.lineTo(76.7775f, 51.9637f);
        STROKE.lineTo(38.7872f, 70.1708f);
        STROKE.cubicTo(74.7026f, 76.2399f, 108.462f, 81.2165f, 36.7231f, 104.2335f);
    }

    /** The two paints {@link #draw} uses; each drawing view keeps its own pair. */
    static Paint newFillPaint() {
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setStyle(Paint.Style.FILL);
        return paint;
    }

    static Paint newStrokePaint() {
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setStrokeJoin(Paint.Join.MITER);
        paint.setStrokeMiter(STROKE_MITER);
        return paint;
    }

    /** Draws the whole mark - rounded square, edge and stroke - with its 132-unit canvas fitted to a {@code size} square centred on the given point; {@code alpha} fades all three together (the disabled state). */
    static void draw(Canvas canvas, Paint fill, Paint stroke, float centerX, float centerY, float size,
            int alpha) {
        if (size <= 0) return;
        float scale = size / SIDE;
        int saved = canvas.save();
        canvas.translate(centerX - (LEFT + SIDE / 2f) * scale, centerY - SIDE * scale / 2f);
        canvas.scale(scale, scale);
        fill.setColor(FIELD);
        fill.setAlpha(alpha);
        canvas.drawRoundRect(FRAME_LEFT, FRAME_TOP, FRAME_RIGHT, FRAME_BOTTOM, FRAME_CORNER, FRAME_CORNER, fill);
        stroke.setStrokeWidth(EDGE_WIDTH);
        stroke.setColor(EDGE);
        stroke.setAlpha(alpha);
        canvas.drawRoundRect(FRAME_LEFT, FRAME_TOP, FRAME_RIGHT, FRAME_BOTTOM, FRAME_CORNER, FRAME_CORNER, stroke);
        stroke.setStrokeWidth(STROKE_WIDTH);
        stroke.setColor(Color.WHITE);
        stroke.setAlpha(alpha);
        canvas.drawPath(STROKE, stroke);
        canvas.restoreToCount(saved);
    }

    private final Paint fill = newFillPaint();
    private final Paint stroke = newStrokePaint();

    public KeyboardBrandMark(Context context) {
        super(context);
        ViewPolicy.setNonInteractive(this);
        ViewPolicy.hideFromAccessibility(this);
    }

    @Override protected void onDraw(Canvas canvas) {
        int width = DimensionPolicy.contentWidth(this);
        int height = DimensionPolicy.contentHeight(this);
        draw(canvas, fill, stroke, getPaddingLeft() + width / 2f, getPaddingTop() + height / 2f,
            DimensionPolicy.shorterSide(width, height), 255);
    }
}
