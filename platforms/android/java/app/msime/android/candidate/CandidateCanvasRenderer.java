package app.msime.android;

import android.animation.ValueAnimator;
import android.database.ContentObserver;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import java.util.List;
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
    private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
    private List<CandidateCanvasAssets.Word> words = List.of();
    private Typeface typeface = Typeface.DEFAULT;
    private boolean enabled;
    private long anchor;
    private long elapsed;
    private boolean motion;
    private boolean observing;
    // 这一遍画到的 APNG 里最早一次换帧还差多少毫秒；没有要动的图时为 Long.MAX_VALUE。
    private long nextFrame = Long.MAX_VALUE;
    // APNG 按自己的帧时长换帧，只在换帧那一刻重画一次；不能每次绘制都约下一个 vsync，那样候选在屏上时整列按屏幕刷新率空转重画。
    private final Runnable tick;
    private final ContentObserver animationPolicy = new ContentObserver(new Handler(Looper.getMainLooper())) {
        @Override public void onChange(boolean selfChange) {
            anchor = SystemClock.uptimeMillis();
            visible(true);
            host.invalidate();
        }
    };

    CandidateCanvasRenderer(View host) {
        this.host = host;
        tick = host::invalidate;
    }

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

    void words(List<CandidateCanvasAssets.Word> next, Typeface font) {
        if (words.equals(next) && typeface.equals(font)) return;
        words = List.copyOf(next);
        typeface = font;
        host.invalidate();
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
        visible &= enabled && host.isAttachedToWindow() && host.isShown()
            && host.getWindowVisibility() == View.VISIBLE;
        // 看不见时撤掉约好的换帧重画，重新显示后的那次绘制会再约。
        if (!visible) host.removeCallbacks(tick);
        if (assets == null) return;
        if (assets.panel != null) assets.panel.visible(host, visible);
        if (assets.banner != null) assets.banner.image().visible(host, visible);
        for (CandidateCanvasAssets.Widget widget : assets.widgets) widget.image().visible(host, visible);
    }

    private boolean ready() { return enabled && assets != null; }

    void before(Canvas canvas, int surface) {
        host.removeCallbacks(tick);
        nextFrame = Long.MAX_VALUE;
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
        if (assets.banner != null) banner(canvas, assets.banner, density);
        // 约在 before() 量过 elapsed 之后，到点时最早那张图一定已换到下一帧。
        if (nextFrame != Long.MAX_VALUE) host.postDelayed(tick, nextFrame);
    }

    private void banner(Canvas canvas, CandidateCanvasAssets.Banner banner, float density) {
        Bitmap image = frame(banner.image());
        float width = (float) banner.width() * density, height = (float) banner.height() * density;
        float scale = Math.min(Math.min(width / image.getWidth(), height / image.getHeight()), card.width() / image.getWidth());
        if (!(scale > 0) || !Float.isFinite(scale)) return;
        float align = "left".equals(banner.align()) ? 0 : "center".equals(banner.align()) ? .5f : 1;
        float w = image.getWidth() * scale, h = image.getHeight() * scale;
        float left = card.left + (card.width() - w) * align, bottom = card.top;
        destination.set(left, bottom - h, left + w, bottom);
        paint.setAlpha(255);
        canvas.drawBitmap(image, null, destination, paint);
        CandidateCanvasAssets.CandidateOverlay style = banner.candidate();
        if (style == null) return;
        String word = style.word(words);
        if (word.isEmpty()) return;
        textPaint.setTypeface(typeface);
        textPaint.setColor(style.color());
        textPaint.setTextSize((float) style.fontSize() * density);
        float natural = Layout.getDesiredWidth(word, textPaint);
        float limit = style.maxWidth() > 0 ? (float) style.maxWidth() * density : width;
        if (natural > limit) textPaint.setTextSize(Math.max((float) Math.min(style.fontSize(), style.minimum()) * density,
            textPaint.getTextSize() * limit / natural));
        // 单行布局保留完整原文，最小字号后允许超出声明框，不向图片裁切。
        int measured = Math.max(1, (int) Math.ceil(Layout.getDesiredWidth(word, textPaint)));
        StaticLayout layout = StaticLayout.Builder.obtain(word, 0, word.length(), textPaint, measured)
            .setIncludePad(false).setMaxLines(1).build();
        float ax = "left".equals(style.horizontal()) ? 0 : "center".equals(style.horizontal()) ? .5f : 1;
        float ay = "top".equals(style.vertical()) ? 0 : "center".equals(style.vertical()) ? .5f : 1;
        int save = canvas.save();
        canvas.translate(left + (w - width) * align + (width - measured) * ax + (float) style.x() * density,
            bottom - height + (height - layout.getHeight()) * ay + (float) style.y() * density);
        layout.draw(canvas);
        canvas.restoreToCount(save);
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
        long wait = animation == null ? -1 : animation.millisUntilNextFrame(elapsed, motion);
        if (wait > 0) nextFrame = Math.min(nextFrame, wait);
        return image.bitmapAt(elapsed, motion);
    }

    private static float[] axis(float length, float start, float end) {
        float scale = start + end > length ? length / (start + end) : 1;
        return new float[] {0, start * scale, length - end * scale, length};
    }
}
