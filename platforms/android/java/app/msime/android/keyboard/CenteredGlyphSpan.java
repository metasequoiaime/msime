package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Rect;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ReplacementSpan;

/**
 * 按字形实际墨迹居中画一个标点。
 *
 * <p>全角的「，」「。」在系统字体里只占字身左下角的一小块：键面文字按字身垂直居中时，看到的只是贴着键底的一个小点。这里按 {@link Paint#getTextBounds} 量出墨迹，把墨迹的中心放到键面中线略偏下（与设计稿一致），并按 {@code scale} 放大；颜色取当前画笔，随键盘皮肤变化。
 */
public final class CenteredGlyphSpan extends ReplacementSpan {
    private final float scale;
    private final Rect ink = new Rect();

    public CenteredGlyphSpan(float scale) {
        this.scale = scale;
    }

    /** 整段文字套上本 span；空串原样返回。 */
    public static CharSequence of(String text, float scale) {
        if (text == null || text.isEmpty()) return text == null ? "" : text;
        SpannableString spanned = new SpannableString(text);
        spanned.setSpan(new CenteredGlyphSpan(scale), 0, text.length(), Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return spanned;
    }

    @Override public int getSize(Paint paint, CharSequence text, int start, int end, Paint.FontMetricsInt fm) {
        float size = paint.getTextSize();
        // 行高照原字号算：ReplacementSpan 不回填 fm 时，这一行会被量成零高，字形落在行盒外被裁掉，键面上什么都看不见。
        if (fm != null) {
            Paint.FontMetricsInt metrics = paint.getFontMetricsInt();
            fm.top = metrics.top;
            fm.ascent = metrics.ascent;
            fm.descent = metrics.descent;
            fm.bottom = metrics.bottom;
            fm.leading = metrics.leading;
        }
        paint.setTextSize(size * scale);
        int width = Math.round(paint.measureText(text, start, end));
        paint.setTextSize(size);
        return width;
    }

    @Override public void draw(Canvas canvas, CharSequence text, int start, int end, float x, int top, int y,
                               int bottom, Paint paint) {
        float size = paint.getTextSize();
        paint.setTextSize(size * scale);
        String glyph = text.subSequence(start, end).toString();
        paint.getTextBounds(glyph, 0, glyph.length(), ink);
        float width = paint.measureText(glyph);
        // 墨迹中心对准行盒中线偏下一点：设计稿里逗号、句号落在键面中线稍下方。
        float centerY = (top + bottom) / 2f + (bottom - top) * 0.08f;
        float baseline = centerY - (ink.top + ink.bottom) / 2f;
        float left = x + (width - ink.width()) / 2f - ink.left;
        canvas.drawText(glyph, left, baseline, paint);
        paint.setTextSize(size);
    }
}
