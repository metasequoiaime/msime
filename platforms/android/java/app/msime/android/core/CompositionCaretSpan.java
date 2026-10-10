package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.text.SpannableString;
import android.text.Spanned;
import android.text.style.ReplacementSpan;
import android.widget.TextView;

/**
 * 读音行里的组字光标（#6110）：把读音里的 {@link CompositionCaretPolicy#CARET_MARK} 画成一条皮肤强调色的粗竖条，高度从读音字体的 ascent 到 descent。原来光标只是一个和拼音同色同字号的 `|`，夹在字母中间看不出来。
 *
 * <p>只换画法不换文字：`|` 仍在读音文本里占一个字符，`TextView.getOffsetForPosition` 给出的下标和原来一样，{@link CompositionCaretPolicy#tapTarget} 的换算不用改。颜色由 {@code ImeStyler.applySkin} 随皮肤刷新（{@link #recolor}）。
 */
final class CompositionCaretSpan extends ReplacementSpan {
    private final int barWidthPx;
    private final int sidePx;
    private int color;

    private CompositionCaretSpan(int barWidthPx, int sidePx, int color) {
        this.barWidthPx = barWidthPx;
        this.sidePx = sidePx;
        this.color = color;
    }

    /**
     * 读音行要显示的文字：{@code index} 处是光标符时把它换成竖条，否则原样返回。
     *
     * @param index 光标符在 {@code text} 里的下标（{@link CompositionCaretPolicy#markInTitle}），没有光标时为 -1
     * @param barWidthPx 竖条宽度
     * @param sidePx 竖条左右各留的空白，免得贴住两边的字母
     */
    static CharSequence mark(String text, int index, int barWidthPx, int sidePx, int color) {
        if (text == null) return "";
        if (index < 0 || index >= text.length() || text.charAt(index) != CompositionCaretPolicy.CARET_MARK) return text;
        SpannableString spanned = new SpannableString(text);
        spanned.setSpan(new CompositionCaretSpan(barWidthPx, sidePx, color), index, index + 1,
            Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return spanned;
    }

    /** 把 {@code view} 文字里的光标竖条改成 {@code color}；换了皮肤时由 applySkin 调用。 */
    static void recolor(TextView view, int color) {
        if (view == null || !(view.getText() instanceof Spanned text)) return;
        CompositionCaretSpan[] spans = text.getSpans(0, text.length(), CompositionCaretSpan.class);
        boolean changed = false;
        for (CompositionCaretSpan span : spans) {
            if (span.color == color) continue;
            span.color = color;
            changed = true;
        }
        if (changed) view.invalidate();
    }

    @Override public int getSize(Paint paint, CharSequence text, int start, int end, Paint.FontMetricsInt fm) {
        // 行高照读音字体算：ReplacementSpan 不回填 fm 时，只剩光标的那一段会被量成零高。
        if (fm != null) {
            Paint.FontMetricsInt metrics = paint.getFontMetricsInt();
            fm.top = metrics.top;
            fm.ascent = metrics.ascent;
            fm.descent = metrics.descent;
            fm.bottom = metrics.bottom;
            fm.leading = metrics.leading;
        }
        return barWidthPx + 2 * sidePx;
    }

    @Override public void draw(Canvas canvas, CharSequence text, int start, int end, float x, int top, int y,
                               int bottom, Paint paint) {
        Paint.FontMetricsInt metrics = paint.getFontMetricsInt();
        int previousColor = paint.getColor();
        Paint.Style previousStyle = paint.getStyle();
        paint.setColor(color);
        paint.setStyle(Paint.Style.FILL);
        float left = x + sidePx;
        canvas.drawRect(left, y + metrics.ascent, left + barWidthPx, y + metrics.descent, paint);
        paint.setColor(previousColor);
        paint.setStyle(previousStyle);
    }
}
