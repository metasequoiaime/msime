package app.msime.android;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PorterDuff;
import android.graphics.PorterDuffXfermode;
import android.graphics.Rect;
import android.graphics.RectF;
import android.view.MotionEvent;
import android.view.View;

/** 裁剪框与笔刷都以原图像素记录；窗口缩放只改变投影，不改变识别区域。 */
final class OcrPhotoSelectionView extends View {
    enum Mode { FULL, CROP, BRUSH }
    private final Bitmap photo;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final RectF imageBounds = new RectF();
    private final RectF crop;
    private final Path strokes = new Path();
    private Mode mode = Mode.FULL;
    private boolean painted, dragging;
    private int edges;
    private float scale = 1, previousX, previousY;
    private final float brushWidth;

    OcrPhotoSelectionView(Context context, Bitmap image) {
        super(context);
        photo = image;
        crop = new RectF(0, 0, image.getWidth(), image.getHeight());
        brushWidth = Math.max(24, Math.min(image.getWidth(), image.getHeight()) / 14f);
        setLayerType(View.LAYER_TYPE_SOFTWARE, null);
        setContentDescription("识别照片：默认全图，选择裁剪或涂抹调整范围");
    }

    void mode(Mode requested) {
        mode = mode == requested ? Mode.FULL : requested;
        crop.set(0, 0, photo.getWidth(), photo.getHeight());
        strokes.reset();
        painted = false;
        setContentDescription(mode == Mode.CROP ? "拖动裁剪框四边或四角，选择识别区域"
            : mode == Mode.BRUSH ? "涂抹要识别的区域，高亮部分将被识别" : "识别整张照片");
        announceForAccessibility(getContentDescription());
        invalidate();
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        scale = Math.min((float) width / photo.getWidth(), (float) height / photo.getHeight());
        float w = photo.getWidth() * scale, h = photo.getHeight() * scale;
        imageBounds.set((width - w) / 2, (height - h) / 2, (width + w) / 2, (height + h) / 2);
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        canvas.save();
        canvas.translate(imageBounds.left, imageBounds.top);
        canvas.scale(scale, scale);
        paint.setColor(Color.WHITE);
        paint.setStyle(Paint.Style.FILL);
        paint.setXfermode(null);
        canvas.drawBitmap(photo, 0, 0, paint);
        if (mode == Mode.CROP) {
            paint.setColor(0x99000000);
            canvas.drawRect(0, 0, photo.getWidth(), crop.top, paint);
            canvas.drawRect(0, crop.bottom, photo.getWidth(), photo.getHeight(), paint);
            canvas.drawRect(0, crop.top, crop.left, crop.bottom, paint);
            canvas.drawRect(crop.right, crop.top, photo.getWidth(), crop.bottom, paint);
            paint.setColor(Color.WHITE);
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(2 / scale);
            canvas.drawRect(crop, paint);
            paint.setStyle(Paint.Style.FILL);
            for (int x = 0; x < 3; x++) for (int y = 0; y < 3; y++) {
                if (x == 1 && y == 1) continue;
                canvas.drawCircle(crop.left + x * crop.width() / 2,
                    crop.top + y * crop.height() / 2, 7 / scale, paint);
            }
        } else if (mode == Mode.BRUSH) {
            int layer = canvas.saveLayer(0, 0, photo.getWidth(), photo.getHeight(), null);
            paint.setColor(0xAA000000);
            canvas.drawRect(0, 0, photo.getWidth(), photo.getHeight(), paint);
            paint.setXfermode(new PorterDuffXfermode(PorterDuff.Mode.CLEAR));
            brush(paint, brushWidth);
            canvas.drawPath(strokes, paint);
            paint.setXfermode(null);
            paint.setStyle(Paint.Style.FILL);
            canvas.restoreToCount(layer);
        }
        canvas.restore();
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (mode == Mode.FULL || scale <= 0) return false;
        float x = clamp((event.getX() - imageBounds.left) / scale, 0, photo.getWidth());
        float y = clamp((event.getY() - imageBounds.top) / scale, 0, photo.getHeight());
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                if (!imageBounds.contains(event.getX(), event.getY())) return false;
                dragging = true;
                getParent().requestDisallowInterceptTouchEvent(true);
                previousX = x; previousY = y;
                if (mode == Mode.CROP) {
                    float hit = 28 * getResources().getDisplayMetrics().density / scale;
                    edges = (Math.abs(x - crop.left) < hit ? 1 : Math.abs(x - crop.right) < hit ? 2 : 0)
                        | (Math.abs(y - crop.top) < hit ? 4 : Math.abs(y - crop.bottom) < hit ? 8 : 0);
                } else {
                    strokes.moveTo(x, y);
                    strokes.lineTo(x + .01f, y + .01f);
                    painted = true;
                }
            }
            case MotionEvent.ACTION_MOVE -> {
                if (!dragging) return false;
                if (mode == Mode.CROP) {
                    float minimum = Math.max(1, Math.min(photo.getWidth(), photo.getHeight()) / 100f);
                    if ((edges & 1) != 0) crop.left = clamp(x, 0, crop.right - minimum);
                    if ((edges & 2) != 0) crop.right = clamp(x, crop.left + minimum, photo.getWidth());
                    if ((edges & 4) != 0) crop.top = clamp(y, 0, crop.bottom - minimum);
                    if ((edges & 8) != 0) crop.bottom = clamp(y, crop.top + minimum, photo.getHeight());
                    if (edges == 0 && crop.contains(previousX, previousY)) {
                        float dx = clamp(x - previousX, -crop.left, photo.getWidth() - crop.right);
                        float dy = clamp(y - previousY, -crop.top, photo.getHeight() - crop.bottom);
                        crop.offset(dx, dy);
                    }
                } else strokes.lineTo(x, y);
                previousX = x; previousY = y;
            }
            case MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                if (!dragging) return false;
                dragging = false;
                getParent().requestDisallowInterceptTouchEvent(false);
                if (event.getActionMasked() == MotionEvent.ACTION_UP) performClick();
            }
            default -> { return dragging; }
        }
        invalidate();
        return true;
    }

    @Override public boolean performClick() { super.performClick(); return true; }

    Selection selection() {
        if (mode == Mode.BRUSH && !painted) return null;
        Rect region = mode == Mode.CROP ? new Rect((int) Math.floor(crop.left), (int) Math.floor(crop.top),
            (int) Math.ceil(crop.right), (int) Math.ceil(crop.bottom)) : new Rect(0, 0, photo.getWidth(), photo.getHeight());
        return new Selection(photo, region, mode == Mode.BRUSH ? new Path(strokes) : null, brushWidth);
    }

    /** 快照与可继续编辑的视图分离，后台识别不读取正在拖动的框或路径。 */
    static final class Selection {
        private final Bitmap source;
        private final Rect region;
        private final Path path;
        private final float width;
        Selection(Bitmap source, Rect region, Path path, float width) {
            this.source = source; this.region = region; this.path = path; this.width = width;
        }
        Bitmap image() {
            if (path == null) return Bitmap.createBitmap(source, region.left, region.top, region.width(), region.height());
            Bitmap output = Bitmap.createBitmap(source.getWidth(), source.getHeight(), Bitmap.Config.ARGB_8888);
            Canvas canvas = new Canvas(output);
            canvas.drawColor(Color.WHITE);
            Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
            int layer = canvas.saveLayer(0, 0, output.getWidth(), output.getHeight(), null);
            paint.setColor(Color.WHITE);
            brush(paint, width);
            canvas.drawPath(path, paint);
            paint.setStyle(Paint.Style.FILL);
            paint.setXfermode(new PorterDuffXfermode(PorterDuff.Mode.SRC_IN));
            canvas.drawBitmap(source, 0, 0, paint);
            canvas.restoreToCount(layer);
            return output;
        }
    }

    private static void brush(Paint paint, float width) {
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeWidth(width);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setStrokeJoin(Paint.Join.ROUND);
    }
    private static float clamp(float value, float min, float max) { return Math.max(min, Math.min(max, value)); }
}
