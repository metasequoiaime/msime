package app.msime.android;

import android.content.Context;
import android.content.res.Resources;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ReplacementSpan;
import android.util.SparseArray;

/** 旧系统字体缺少粗等号、手势或新下标时，只补标签绘制，不改变选中的字符或序号。 */
public final class CandidateSymbolLabel {
    private CandidateSymbolLabel() {}

    public static CharSequence apply(Context context, CharSequence label, int start, int end) {
        SpannableString result = null;
        for (int offset = start; offset < end;) {
            int codePoint = Character.codePointAt(label, offset);
            int next = offset + Character.charCount(codePoint);
            if (codePoint == 0x1f7f0) {
                if (next < end && (label.charAt(next) == '\ufe0e' || label.charAt(next) == '\ufe0f'))
                    next++;
                if (result == null) result = new SpannableString(label);
                result.setSpan(new HeavyEqualsSpan(label.subSequence(offset, next).toString()),
                    offset, next, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            } else if (codePoint >= 0x209d && codePoint <= 0x209f) {
                if (result == null) result = new SpannableString(label);
                result.setSpan(new SubscriptSpan(label.subSequence(offset, next).toString(),
                        codePoint == 0x209d ? "w" : codePoint == 0x209e ? "y" : "z"),
                    offset, next, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            } else if (codePoint == 0x1faf1 || codePoint == 0x1faf2
                    || codePoint == 0x1faf7 || codePoint == 0x1faf8) {
                boolean textVariant = false;
                if (next < end && (label.charAt(next) == '\ufe0e' || label.charAt(next) == '\ufe0f')) {
                    textVariant = label.charAt(next) == '\ufe0e';
                    next++;
                }
                int tone = next < end ? Character.codePointAt(label, next) : 0;
                if (tone >= 0x1f3fb && tone <= 0x1f3ff) next += Character.charCount(tone);
                else tone = 0;
                if (next < end && (label.charAt(next) == '\ufe0e' || label.charAt(next) == '\ufe0f')) {
                    textVariant |= label.charAt(next) == '\ufe0e';
                    next++;
                }
                // 完整 ZWJ 序列交给系统，不把握手等组合拆成两个独立手势。
                boolean joined = (offset > start && label.charAt(offset - 1) == '\u200d')
                    || (next < end && label.charAt(next) == '\u200d');
                if (!textVariant && !joined) {
                    if (result == null) result = new SpannableString(label);
                    result.setSpan(new HandEmojiSpan(context, label.subSequence(offset, next).toString(),
                            codePoint, tone), offset, next, Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
                }
            }
            offset = next;
        }
        return result == null ? label : result;
    }

    private static final class SubscriptSpan extends ReplacementSpan {
        private final String glyph;
        private final String base;

        SubscriptSpan(String glyph, String base) {
            this.glyph = glyph;
            this.base = base;
        }

        private Paint smallPaint(Paint paint) {
            Paint small = new Paint(paint);
            small.setTextSize(paint.getTextSize() * 0.65f);
            return small;
        }

        @Override public int getSize(Paint paint, CharSequence text, int start, int end,
                Paint.FontMetricsInt metrics) {
            if (metrics != null) paint.getFontMetricsInt(metrics);
            return (int) Math.ceil(paint.hasGlyph(glyph)
                ? paint.measureText(glyph) : smallPaint(paint).measureText(base));
        }

        @Override public void draw(Canvas canvas, CharSequence text, int start, int end,
                float x, int top, int y, int bottom, Paint paint) {
            if (paint.hasGlyph(glyph)) {
                canvas.drawText(glyph, x, y, paint);
                return;
            }
            Paint small = smallPaint(paint);
            // 对齐原行下降部，让下标低于基线，同时保留 y 的下降笔画。
            canvas.drawText(base, x, y + paint.descent() - small.descent(), small);
        }
    }

    private static final class HandEmojiSpan extends ReplacementSpan {
        // 固定四个手势及各五种肤色，缓存至多 24 张 128×128 位图。
        private static final SparseArray<Bitmap> BITMAPS = new SparseArray<>();
        private final String glyph;
        private final Resources resources;
        private final int resourceId;

        HandEmojiSpan(Context context, String glyph, int codePoint, int tone) {
            this.glyph = glyph;
            resources = context.getResources();
            String name = "noto_emoji_u" + Integer.toHexString(codePoint)
                + (tone == 0 ? "" : "_" + Integer.toHexString(tone));
            resourceId = resources.getIdentifier(name, "drawable", context.getPackageName());
            if (resourceId == 0) throw new IllegalStateException("Missing candidate emoji resource: " + name);
        }

        @Override public int getSize(Paint paint, CharSequence text, int start, int end,
                Paint.FontMetricsInt metrics) {
            if (metrics != null) paint.getFontMetricsInt(metrics);
            return (int) Math.ceil(paint.hasGlyph(glyph) ? paint.measureText(glyph) : paint.getTextSize());
        }

        @Override public void draw(Canvas canvas, CharSequence text, int start, int end,
                float x, int top, int y, int bottom, Paint paint) {
            if (paint.hasGlyph(glyph)) {
                canvas.drawText(glyph, x, y, paint);
                return;
            }
            Bitmap bitmap;
            synchronized (BITMAPS) {
                bitmap = BITMAPS.get(resourceId);
                if (bitmap == null) {
                    BitmapFactory.Options options = new BitmapFactory.Options();
                    options.inScaled = false;
                    bitmap = BitmapFactory.decodeResource(resources, resourceId, options);
                    BITMAPS.put(resourceId, bitmap);
                }
            }
            float size = paint.getTextSize();
            float imageTop = y + (paint.ascent() + paint.descent() - size) * 0.5f;
            Paint imagePaint = new Paint(Paint.ANTI_ALIAS_FLAG | Paint.FILTER_BITMAP_FLAG);
            imagePaint.setAlpha(paint.getAlpha());
            canvas.drawBitmap(bitmap, null, new RectF(x, imageTop, x + size, imageTop + size), imagePaint);
        }
    }

    private static final class HeavyEqualsSpan extends ReplacementSpan {
        private final String glyph;

        HeavyEqualsSpan(String glyph) { this.glyph = glyph; }

        @Override public int getSize(Paint paint, CharSequence text, int start, int end,
                Paint.FontMetricsInt metrics) {
            if (metrics != null) paint.getFontMetricsInt(metrics);
            return (int) Math.ceil(paint.hasGlyph(glyph)
                ? paint.measureText(glyph) : paint.getTextSize());
        }

        @Override public void draw(Canvas canvas, CharSequence text, int start, int end,
                float x, int top, int y, int bottom, Paint paint) {
            // 字体可能随皮肤和候选设置变化，按实际绘制时的字体决定是否需要兜底。
            if (paint.hasGlyph(glyph)) {
                canvas.drawText(glyph, x, y, paint);
                return;
            }
            Paint bars = new Paint(paint);
            bars.setStyle(Paint.Style.FILL);
            float size = paint.getTextSize();
            float left = x + size * 0.1f;
            float right = x + size * 0.9f;
            float first = y + paint.ascent() * 0.72f;
            float thickness = size * 0.12f;
            canvas.drawRect(left, first, right, first + thickness, bars);
            float second = first + size * 0.3f;
            canvas.drawRect(left, second, right, second + thickness, bars);
        }
    }
}
