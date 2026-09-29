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
    private final KeyboardSkin skin;
    private final int fillColor;
    private final boolean action;
    private final float density;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint overlay = new Paint(Paint.ANTI_ALIAS_FLAG);
    private int alpha = 255;

    public KeyboardSkinKeyDrawable(KeyboardSkin skin, int fillColor, boolean action, float density) {
        this.skin = skin;
        this.fillColor = fillColor;
        this.action = action;
        this.density = density;
    }

    private float dp(double value) { return (float) value * density; }

    private Path path(RectF rect) {
        Path path = new Path();
        switch (skin.keyShape()) {
            case "capsule" -> path.addRoundRect(rect, rect.height() / 2, rect.height() / 2,
                Path.Direction.CW);
            case "pebble" -> {
                float x = rect.left, y = rect.top, width = rect.width(), height = rect.height();
                path.moveTo(x + width * .35f, y);
                path.cubicTo(x + width * .83f, y, x + width, y + height * .04f,
                    x + width, y + height * .3f);
                path.cubicTo(x + width, y + height * .85f, x + width * .9f, y + height,
                    x + width * .68f, y + height);
                path.cubicTo(x + width * .18f, y + height, x, y + height * .97f,
                    x, y + height * .7f);
                path.cubicTo(x, y + height * .2f, x + width * .06f, y,
                    x + width * .35f, y);
                path.close();
            }
            case "ticket" -> {
                float notch = Math.min(rect.width(), rect.height()) * .12f;
                path.moveTo(rect.left, rect.top);
                path.lineTo(rect.right, rect.top);
                path.lineTo(rect.right, rect.centerY() - notch);
                path.arcTo(new RectF(rect.right - notch, rect.centerY() - notch,
                    rect.right + notch, rect.centerY() + notch), -90, -180);
                path.lineTo(rect.right, rect.bottom);
                path.lineTo(rect.left, rect.bottom);
                path.lineTo(rect.left, rect.centerY() + notch);
                path.arcTo(new RectF(rect.left - notch, rect.centerY() - notch,
                    rect.left + notch, rect.centerY() + notch), 90, -180);
                path.close();
            }
            default -> {
                float radius = Math.min(dp(skin.cornerRadius()),
                    Math.min(rect.width(), rect.height()) / 2);
                path.addRoundRect(rect, radius, radius, Path.Direction.CW);
            }
        }
        return path;
    }

    @Override public void draw(Canvas canvas) {
        float depth = "raised".equals(skin.keyMaterial()) ? dp(3) : 0;
        RectF face = new RectF(getBounds());
        face.inset(dp(1), dp(1));
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
            overlay.setShader(new LinearGradient(face.centerX(), face.top, face.centerX(), face.bottom,
                new int[] {Color.argb((int) (255 * (glass ? .24 : .13)), 255, 255, 255),
                    Color.TRANSPARENT, Color.argb((int) (255 * (glass ? .03 : .10)), 0, 0, 0)},
                new float[] {0, .48f, 1}, Shader.TileMode.CLAMP));
            overlay.setAlpha(alpha);
            canvas.drawRect(face, overlay);
            if (glass) {
                Path shine = new Path();
                shine.moveTo(face.left, face.top);
                shine.lineTo(face.right, face.top);
                shine.lineTo(face.left, face.top + face.height() * .6f);
                shine.close();
                overlay.setShader(null);
                overlay.setColor(Color.WHITE);
                overlay.setAlpha((int) Math.round(alpha * .09));
                canvas.drawPath(shine, overlay);
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
            paint.setColor(Color.parseColor(skin.borderColor()));
            paint.setAlpha(alpha);
            canvas.drawPath(shape, paint);
        }
    }

    @Override public void setAlpha(int value) {
        alpha = KeyboardGeometry.bounded(value, 0, 255);
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
