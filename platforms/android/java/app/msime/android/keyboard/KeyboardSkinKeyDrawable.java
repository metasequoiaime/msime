package app.msime.android;


import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.ColorFilter;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.graphics.RectF;
import android.graphics.Shader;
import android.graphics.drawable.Drawable;

/** Draws Apple's custom key shapes and flat, raised, glass or paper materials. */
public final class KeyboardSkinKeyDrawable extends Drawable {
    /** 键帽面离键格四边的内缩（dp）。键面排版按同一个数算轮廓（{@link KeyOutline#forSkin}）。 */
    static final float FACE_INSET_DP = 1;
    /** 凸起材质在键帽面下方占的厚度（dp）。 */
    static final float RAISED_DEPTH_DP = 3;

    private final KeyboardSkin skin;
    private final int fillColor;
    private final boolean action;
    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint overlay = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path shapePath = new Path();
    private final RectF shapeBounds = new RectF();
    private final RectF arcBounds = new RectF();
    private final RectF roundBounds = new RectF();
    private final RectF faceBounds = new RectF();
    private final RectF gradientBounds = new RectF();
    private final Path shinePath = new Path();
    private final int borderColor;
    private Shader overlayGradient;
    private boolean shapeValid;
    private boolean gradientValid;
    private int alpha = 255;

    public KeyboardSkinKeyDrawable(KeyboardSkin skin, int fillColor, boolean action, float density) {
        this.skin = skin;
        this.fillColor = fillColor;
        this.action = action;
        this.density = density;
        borderColor = Color.parseColor(skin.borderColor());
        outline = KeyOutline.forSkin(skin, density);
    }

    /** 键形由 {@link KeyOutline} 描出，键面排版用同一份描述判断墨迹是否还在键帽里。 */
    private final KeyOutline outline;
    private final KeyOutline.PathSink pathSink = new KeyOutline.PathSink() {
        @Override public void moveTo(float x, float y) { shapePath.moveTo(x, y); }

        @Override public void lineTo(float x, float y) { shapePath.lineTo(x, y); }

        @Override public void cubicTo(float x1, float y1, float x2, float y2, float x3, float y3) {
            shapePath.cubicTo(x1, y1, x2, y2, x3, y3);
        }

        @Override public void arcTo(float left, float top, float right, float bottom, float start, float sweep) {
            arcBounds.set(left, top, right, bottom);
            shapePath.arcTo(arcBounds, start, sweep);
        }

        @Override public void roundRect(float left, float top, float right, float bottom, float radiusX,
                float radiusY) {
            roundBounds.set(left, top, right, bottom);
            shapePath.addRoundRect(roundBounds, radiusX, radiusY, Path.Direction.CW);
        }

        @Override public void close() { shapePath.close(); }
    };

    private float dp(double value) { return DimensionPolicy.floatPixels(value, density); }

    private Path path(RectF rect) {
        if (shapeValid && shapeBounds.equals(rect)) return shapePath;
        shapePath.reset();
        outline.trace(rect.left, rect.top, rect.right, rect.bottom, pathSink);
        shapeBounds.set(rect);
        shapeValid = true;
        return shapePath;
    }

    @Override public void draw(Canvas canvas) {
        float depth = "raised".equals(skin.keyMaterial()) ? dp(RAISED_DEPTH_DP) : 0;
        faceBounds.set(getBounds());
        RectF face = faceBounds;
        face.inset(dp(FACE_INSET_DP), dp(FACE_INSET_DP));
        face.bottom -= depth;
        Path shape = path(face);
        int opacity = action ? alpha : (int) Math.round(alpha * skin.keyOpacity());
        paint.setStyle(Paint.Style.FILL);
        paint.setShader(null);
        paint.setColor(fillColor);
        paint.setAlpha(opacity);
        if (depth > 0) {
            canvas.save();
            canvas.translate(0, depth);
            canvas.drawPath(shape, paint);
            paint.setColor(Color.BLACK);
            paint.setAlpha((int) Math.round(opacity * .28));
            canvas.drawPath(shape, paint);
            canvas.restore();
            paint.setColor(fillColor);
            paint.setAlpha(opacity);
        }
        canvas.drawPath(shape, paint);
        canvas.save();
        canvas.clipPath(shape);
        if ("glass".equals(skin.keyMaterial()) || "raised".equals(skin.keyMaterial())) {
            boolean glass = "glass".equals(skin.keyMaterial());
            if (!gradientValid || !gradientBounds.equals(face)) {
                overlayGradient = new LinearGradient(face.centerX(), face.top, face.centerX(), face.bottom,
                    new int[] {ColorPolicy.withAlpha(Color.WHITE, (int) (255 * (glass ? .24 : .13))),
                        Color.TRANSPARENT,
                        ColorPolicy.withAlpha(Color.BLACK, (int) (255 * (glass ? .03 : .10)))},
                    new float[] {0, .48f, 1}, Shader.TileMode.CLAMP);
                gradientBounds.set(face);
                gradientValid = true;
            }
            overlay.setShader(overlayGradient);
            overlay.setAlpha(alpha);
            canvas.drawRect(face, overlay);
            if (glass) {
                shinePath.reset();
                shinePath.moveTo(face.left, face.top);
                shinePath.lineTo(face.right, face.top);
                shinePath.lineTo(face.left, face.top + face.height() * .6f);
                shinePath.close();
                overlay.setShader(null);
                overlay.setColor(Color.WHITE);
                overlay.setAlpha((int) Math.round(alpha * .09));
                canvas.drawPath(shinePath, overlay);
            }
        } else if ("paper".equals(skin.keyMaterial())) {
            overlay.setShader(null);
            overlay.setStyle(Paint.Style.STROKE);
            overlay.setStrokeWidth(dp(.5));
            overlay.setColor(Color.BLACK);
            overlay.setAlpha((int) Math.round(alpha * .08));
            for (float y = face.top + dp(3); y < face.bottom; y += dp(4))
                canvas.drawLine(face.left, y, face.right, y - dp(1), overlay);
            overlay.setStyle(Paint.Style.FILL);
        }
        canvas.restore();
        if (skin.borderWidth() > 0) {
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(dp(skin.borderWidth()));
            paint.setColor(borderColor);
            paint.setAlpha(alpha);
            canvas.drawPath(shape, paint);
        }
    }

    @Override protected void onBoundsChange(android.graphics.Rect bounds) {
        super.onBoundsChange(bounds);
        shapeValid = false;
        gradientValid = false;
    }

    @Override public void setAlpha(int value) {
        alpha = BoundsPolicy.bounded(value, 0, 255);
        invalidateSelf();
    }

    @Override public int getAlpha() { return alpha; }

    @Override public void setColorFilter(ColorFilter filter) {
        paint.setColorFilter(filter);
        overlay.setColorFilter(filter);
        invalidateSelf();
    }

    @Deprecated
    @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }
}
