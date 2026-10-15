package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ReplacementSpan;

/** 旧系统字体缺少粗等号时，只补候选的绘制，不改变选中的字符或序号。 */
public final class CandidateSymbolLabel {
    private CandidateSymbolLabel() {}

    public static CharSequence apply(CharSequence label, int start, int end) {
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
            }
            offset = next;
        }
        return result == null ? label : result;
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
