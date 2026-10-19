package app.msime.android;

import android.animation.ValueAnimator;
import android.database.ContentObserver;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;
import android.os.SystemClock;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.view.View;

/** 画布分成文字前后的两次绘制，不增加可命中的 View。 */
final class CandidateCanvasRenderer {
    private final View host;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
    private final RectF destination = new RectF();
    private final RectF card = new RectF();
    private CandidateCanvasAssets assets;
    private boolean enabled;
    private long anchor;
    private long elapsed;
    private boolean motion;
    private boolean observing;
    private final ContentObserver animationPolicy = new ContentObserver(new Handler(Looper.getMainLooper())) {
        @Override public void onChange(boolean selfChange) {
            anchor = SystemClock.uptimeMillis();
            visible(true);
            host.invalidate();
        }
    };

    CandidateCanvasRenderer(View host) { this.host = host; }

    void set(CandidateCanvasAssets next, boolean enabled) {
        if (this.enabled == enabled && assets != null && next != null && assets.key.equals(next.key)) return;
        if (assets != next || this.enabled != enabled) {
            visible(false);
            if (assets == null || next == null || !assets.key.equals(next.key) || this.enabled != enabled)
                anchor = SystemClock.uptimeMillis();
            assets = next;
            this.enabled = enabled;
            visible(true);
            host.invalidate();
        }
    }

    void attached() {
        if (!observing) {
            host.getContext().getContentResolver().registerContentObserver(
                Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, animationPolicy);
            observing = true;
        }
    }

    void detached() {
        visible(false);
        if (observing) {
            host.getContext().getContentResolver().unregisterContentObserver(animationPolicy);
            observing = false;
        }
    }

    void visible(boolean visible) {
        if (assets == null) return;
        visible &= enabled && host.isAttachedToWindow() && host.isShown()
            && host.getWindowVisibility() == View.VISIBLE;
        if (assets.panel != null) assets.panel.visible(host, visible);
        for (CandidateCanvasAssets.Widget widget : assets.widgets) widget.image().visible(host, visible);
    }

    private boolean ready() { return enabled && assets != null; }

    void before(Canvas canvas, int surface) {
        if (!ready()) return;
        visible(true);
        motion = ValueAnimator.areAnimatorsEnabled();
        elapsed = Math.max(0, SystemClock.uptimeMillis() - anchor);
        float density = host.getResources().getDisplayMetrics().density;
        card.set((float) assets.outer[3] * density, (float) assets.outer[0] * density,
            host.getWidth() - (float) assets.outer[1] * density,
            host.getHeight() - (float) assets.outer[2] * density);
        if (card.isEmpty()) return;
        paint.setAlpha(255);
        for (CandidateCanvasAssets.Widget widget : assets.widgets) if (widget.z() < 1) widget(canvas, widget, density);
        if (assets.panel != null) panel(canvas, density);
        else { paint.setColor(surface); canvas.drawRect(card, paint); }
    }

    void after(Canvas canvas) {
        if (!ready() || card.isEmpty()) return;
        float density = host.getResources().getDisplayMetrics().density;
        for (CandidateCanvasAssets.Widget widget : assets.widgets) if (widget.z() >= 1) widget(canvas, widget, density);
    }

    private void panel(Canvas canvas, float density) {
        Bitmap image = frame(assets.panel);
        Bitmap[] tiles = assets.panelTiles.at(image);
        float[] dx = axis(card.width(), (float) assets.widths[3] * density, (float) assets.widths[1] * density);
        float[] dy = axis(card.height(), (float) assets.widths[0] * density, (float) assets.widths[2] * density);
        for (int y = 0; y < 3; y++) for (int x = 0; x < 3; x++) {
            if (x == 1 && y == 1 && !assets.fill) continue;
            destination.set(card.left + dx[x], card.top + dy[y], card.left + dx[x + 1], card.top + dy[y + 1]);
            Bitmap tile = tiles[y * 3 + x];
            if (tile != null && !destination.isEmpty()) canvas.drawBitmap(tile, null, destination, paint);
        }
    }

    private void widget(Canvas canvas, CandidateCanvasAssets.Widget widget, float density) {
        paint.setAlpha(255);
        float width = (float) widget.width() * density, height = (float) widget.height() * density;
        String anchor = widget.anchor();
        float x = card.centerX() - width / 2, y = card.centerY() - height / 2;
        if (anchor.contains("left")) x = card.left;
        if (anchor.contains("right")) x = card.right - width;
        if (anchor.contains("top")) y = card.top;
        if (anchor.contains("bottom")) y = card.bottom - height;
        x += (float) widget.x() * density; y += (float) widget.y() * density;
        Bitmap image = frame(widget.image());
        float scale = Math.min(width / image.getWidth(), height / image.getHeight());
        float paintedWidth = image.getWidth() * scale, paintedHeight = image.getHeight() * scale;
        x += (width - paintedWidth) / 2; y += (height - paintedHeight) / 2;
        destination.set(x, y, x + paintedWidth, y + paintedHeight);
        canvas.drawBitmap(image, null, destination, paint);
    }

    private Bitmap frame(CandidateCanvasAssets.Image image) {
        CandidateApngFrames animation = image.apng;
        if (motion && animation != null && animation.size() > 1
                && (animation.loops() == 0 || elapsed / animation.durationMillis() < animation.loops()))
            host.postInvalidateOnAnimation();
        return image.bitmapAt(elapsed, motion);
    }

    private static float[] axis(float length, float start, float end) {
        float scale = start + end > length ? length / (start + end) : 1;
        return new float[] {0, start * scale, length - end * scale, length};
    }
}
