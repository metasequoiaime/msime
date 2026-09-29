package app.msime.client;

import android.graphics.Canvas;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.graphics.ColorFilter;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.RectF;
import android.graphics.Shader;
import android.graphics.drawable.Drawable;

/** Draws Apple's built-in touch-keyboard backdrop patterns without external assets. */
public final class KeyboardSkinBackgroundDrawable extends Drawable {
    private final Paint background = new Paint();
    private final Paint pattern = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint photoPaint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final Paint shade = new Paint();
    private final float density;
    private final int patternId;
    private final int backgroundStart;
    private final Integer backgroundEnd;
    private final boolean gradientHorizontal;
    private final int patternAlpha;
    private final Bitmap photo;
    private final double photoShade;
    private final double photoPosition;
    private int alpha = 255;

    public KeyboardSkinBackgroundDrawable(KeyboardSkin skin, float density) {
        this.density = density;
        patternId = skin.pattern();
        backgroundStart = Color.parseColor(skin.background());
        backgroundEnd = skin.gradientEnd() == null ? null : Color.parseColor(skin.gradientEnd());
        gradientHorizontal = skin.gradientHorizontal();
        background.setColor(backgroundStart);
        int accent = Color.parseColor(skin.accent());
        patternAlpha = (int) Math.round(255 * skin.patternOpacity());
        pattern.setColor(Color.argb(patternAlpha, Color.red(accent), Color.green(accent), Color.blue(accent)));
        pattern.setStyle(Paint.Style.FILL);
        byte[] photoBytes = skin.photo();
        photo = photoBytes == null ? null
            : BitmapFactory.decodeByteArray(photoBytes, 0, photoBytes.length);
        photoShade = skin.photoShade();
        photoPosition = skin.photoPosition();
        shade.setColor(Color.BLACK);
    }

    private float dp(double value) { return (float) value * density; }

    @Override protected void onBoundsChange(Rect bounds) {
        super.onBoundsChange(bounds);
        if (backgroundEnd == null) {
            background.setShader(null);
            background.setColor(backgroundStart);
            return;
        }
        float endX = gradientHorizontal ? bounds.right : bounds.left;
        float endY = gradientHorizontal ? bounds.top : bounds.bottom;
        background.setShader(new LinearGradient(bounds.left, bounds.top, endX, endY,
            backgroundStart, backgroundEnd, Shader.TileMode.CLAMP));
    }

    @Override public void draw(Canvas canvas) {
        canvas.drawRect(getBounds(), background);
        if (photo != null && photo.getWidth() > 0 && photo.getHeight() > 0) {
            float width = getBounds().width();
            float height = getBounds().height();
            float scale = Math.max(width / photo.getWidth(), height / photo.getHeight());
            float drawWidth = photo.getWidth() * scale;
            float drawHeight = photo.getHeight() * scale;
            float left = getBounds().left + (width - drawWidth) * (float) photoPosition;
            float top = getBounds().top + (height - drawHeight) * (float) photoPosition;
            canvas.drawBitmap(photo, null,
                new RectF(left, top, left + drawWidth, top + drawHeight), photoPaint);
            shade.setAlpha((int) Math.round(255 * photoShade * alpha / 255));
            canvas.drawRect(getBounds(), shade);
        }
        if (patternId == 0) return;
        float left = getBounds().left;
        float top = getBounds().top;
        float right = getBounds().right;
        float bottom = getBounds().bottom;
        if (patternId == 1) {
            float diameter = dp(1.5);
            for (float y = top + dp(8); y < bottom; y += dp(16)) {
                for (float x = left + dp(8); x < right; x += dp(16))
                    canvas.drawOval(x, y, x + diameter, y + diameter, pattern);
            }
            return;
        }
        pattern.setStyle(Paint.Style.STROKE);
        if (patternId == 2) {
            pattern.setStrokeWidth(dp(0.5));
            for (float x = left; x < right; x += dp(20))
                canvas.drawLine(x, top, x, bottom, pattern);
            for (float y = top; y < bottom; y += dp(20))
                canvas.drawLine(left, y, right, y, pattern);
            return;
        }
        pattern.setStrokeWidth(dp(2));
        float width = right - left;
        Path wave = new Path();
        for (float offset = top - dp(100); offset < bottom + width; offset += dp(24)) {
            wave.moveTo(left, offset);
            wave.cubicTo(left + width * 0.35f, offset - dp(90),
                left + width * 0.65f, offset + dp(20), right, offset - dp(70));
        }
        canvas.drawPath(wave, pattern);
    }

    @Override public void setAlpha(int value) {
        alpha = KeyboardGeometry.bounded(value, 0, 255);
        background.setAlpha(alpha);
        pattern.setAlpha(Math.round(patternAlpha * alpha / 255f));
        photoPaint.setAlpha(alpha);
        invalidateSelf();
    }

    @Override public int getAlpha() { return alpha; }

    @Override public void setColorFilter(ColorFilter filter) {
        background.setColorFilter(filter);
        pattern.setColorFilter(filter);
        invalidateSelf();
    }

    @Deprecated
    @Override public int getOpacity() { return alpha == 255 ? PixelFormat.OPAQUE : PixelFormat.TRANSLUCENT; }
}
