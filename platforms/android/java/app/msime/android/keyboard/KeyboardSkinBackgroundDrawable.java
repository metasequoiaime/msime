package app.msime.android;


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
import java.lang.ref.WeakReference;
import java.util.Arrays;

/** Draws Apple's built-in touch-keyboard backdrop patterns without external assets. */
public final class KeyboardSkinBackgroundDrawable extends Drawable {
    private final Paint background = new Paint();
    private final Paint pattern = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint photoPaint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final Paint shade = new Paint();
    private final RectF photoBounds = new RectF();
    private final Path wavePath = new Path();
    private final KeyboardSkin skin;
    private final float density;
    private final int patternId;
    private final int backgroundStart;
    private final Integer backgroundEnd;
    private final boolean gradientHorizontal;
    private final int patternAlpha;
    private final Bitmap photo;
    private final double photoShade;
    private final double photoPosition;
    private boolean photoBoundsValid;
    private boolean wavePathValid;
    private int alpha = 255;

    /** 这块底图用的那条照片缓存；底图强引用它，缓存本身只弱引用，所以还有底图在画这张照片时它一直留着。 */
    private final DecodedPhoto decodedPhoto;

    /** 一张照片的字节和解码结果。`KeyboardSkin.photo()` 每次返回副本，所以字节要和位图一起由底图留住，才能按内容认出同一张照片。 */
    private record DecodedPhoto(byte[] bytes, Bitmap bitmap) {}

    /** 最近一次解码的照片。键盘底图、各个面板和表情面板用的是同一张照片，解码一次大家共用；只弱引用，换成没有照片的皮肤后随底图一起被回收，不在整个输入法进程里一直占着整张位图。 */
    private static WeakReference<DecodedPhoto> photoCache = new WeakReference<>(null);

    public KeyboardSkinBackgroundDrawable(KeyboardSkin skin, float density) {
        this.skin = skin;
        this.density = density;
        patternId = skin.pattern();
        backgroundStart = Color.parseColor(skin.background());
        backgroundEnd = skin.gradientEnd() == null ? null : Color.parseColor(skin.gradientEnd());
        gradientHorizontal = skin.gradientHorizontal();
        background.setColor(backgroundStart);
        int accent = Color.parseColor(skin.accent());
        int patternColor = ColorPolicy.withAlpha(accent, (float) skin.patternOpacity());
        patternAlpha = Color.alpha(patternColor);
        pattern.setColor(patternColor);
        pattern.setStyle(Paint.Style.FILL);
        decodedPhoto = decodedPhoto(skin.photo());
        photo = decodedPhoto == null ? null : decodedPhoto.bitmap();
        photoShade = skin.photoShade();
        photoPosition = skin.photoPosition();
        shade.setColor(Color.BLACK);
    }

    /**
     * 照片按字节内容缓存解码结果：同一张照片只解码一次，所有底图共用这一个只读的 `Bitmap`。
     *
     * <p>照片最大约半兆，整张解码在主线程上要几十毫秒；以前每个底图各解一次，一次换肤要解十来次。按内容比较而不是按哈希，换了照片一定会重新解码。旧位图不 `recycle()`，还挂着它的底图照常能画，没有底图引用后交给 GC。
     */
    private static synchronized DecodedPhoto decodedPhoto(byte[] bytes) {
        if (bytes == null) return null;
        DecodedPhoto cached = photoCache.get();
        if (cached != null && Arrays.equals(bytes, cached.bytes())) return cached;
        try {
            BitmapFactory.Options bounds = new BitmapFactory.Options();
            bounds.inJustDecodeBounds = true;
            BitmapFactory.decodeByteArray(bytes, 0, bytes.length, bounds);
            int sample = PhotoDecodePolicy.sampleSize(bounds.outWidth, bounds.outHeight);
            if (sample == 0) return null;
            BitmapFactory.Options options = new BitmapFactory.Options();
            options.inSampleSize = sample;
            Bitmap bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.length, options);
            if (bitmap == null || !PhotoDecodePolicy.withinBounds(bitmap.getWidth(), bitmap.getHeight())) {
                if (bitmap != null) bitmap.recycle();
                return null;
            }
            DecodedPhoto decoded = new DecodedPhoto(bytes, bitmap);
            photoCache = new WeakReference<>(decoded);
            return decoded;
        } catch (RuntimeException invalidPhoto) {
            return null;
        }
    }

    /** 这块底图是否正是按 `target` 和 `targetDensity` 画的；`KeyboardSkin` 不可变，同一个对象画出来就完全相同。 */
    public boolean draws(KeyboardSkin target, float targetDensity) {
        return skin == target && density == targetDensity;
    }

    @Override public void draw(Canvas canvas) {
        canvas.drawRect(getBounds(), background);
        if (photo != null && photo.getWidth() > 0 && photo.getHeight() > 0) {
            if (!photoBoundsValid) {
                float width = getBounds().width();
                float height = getBounds().height();
                float scale = BoundsPolicy.atLeast(width / photo.getWidth(), height / photo.getHeight());
                float drawWidth = photo.getWidth() * scale;
                float drawHeight = photo.getHeight() * scale;
                float left = getBounds().left + (width - drawWidth) * (float) photoPosition;
                float top = getBounds().top + (height - drawHeight) * (float) photoPosition;
                photoBounds.set(left, top, left + drawWidth, top + drawHeight);
                photoBoundsValid = true;
            }
            canvas.drawBitmap(photo, null, photoBounds, photoPaint);
            shade.setAlpha((int) Math.round(255 * photoShade * alpha / 255));
            canvas.drawRect(getBounds(), shade);
        }
        if (patternId == 0) return;
        float left = getBounds().left;
        float top = getBounds().top;
        float right = getBounds().right;
        float bottom = getBounds().bottom;
        if (patternId == 1) {
            float diameter = DimensionPolicy.floatPixels(1.5, density);
            for (float y = top + DimensionPolicy.floatPixels(8, density); y < bottom;
                 y += DimensionPolicy.floatPixels(16, density)) {
                for (float x = left + DimensionPolicy.floatPixels(8, density); x < right;
                     x += DimensionPolicy.floatPixels(16, density))
                    canvas.drawOval(x, y, x + diameter, y + diameter, pattern);
            }
            return;
        }
        pattern.setStyle(Paint.Style.STROKE);
        if (patternId == 2) {
            pattern.setStrokeWidth(DimensionPolicy.floatPixels(0.5, density));
            for (float x = left; x < right; x += DimensionPolicy.floatPixels(20, density))
                canvas.drawLine(x, top, x, bottom, pattern);
            for (float y = top; y < bottom; y += DimensionPolicy.floatPixels(20, density))
                canvas.drawLine(left, y, right, y, pattern);
            return;
        }
        pattern.setStrokeWidth(DimensionPolicy.floatPixels(2, density));
        float width = right - left;
        if (!wavePathValid) {
            wavePath.reset();
            for (float offset = top - DimensionPolicy.floatPixels(100, density);
                 offset < bottom + width; offset += DimensionPolicy.floatPixels(24, density)) {
                wavePath.moveTo(left, offset);
                wavePath.cubicTo(left + width * 0.35f, offset - DimensionPolicy.floatPixels(90, density),
                    left + width * 0.65f, offset + DimensionPolicy.floatPixels(20, density), right,
                    offset - DimensionPolicy.floatPixels(70, density));
            }
            wavePathValid = true;
        }
        canvas.drawPath(wavePath, pattern);
    }

    @Override protected void onBoundsChange(Rect bounds) {
        super.onBoundsChange(bounds);
        photoBoundsValid = false;
        wavePathValid = false;
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

    @Override public void setAlpha(int value) {
        alpha = BoundsPolicy.bounded(value, 0, 255);
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
