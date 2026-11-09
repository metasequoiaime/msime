package app.msime.android;

import android.content.Context;
import android.graphics.Paint;
import android.graphics.Rect;

/**
 * 文本编辑面板带上滑角标的侧键（全选右上角的「选行」，#11111）。角标的字号、颜色和位置同字母键（{@link KeyHintButton}），只画不进节点文字；每次排版后按 {@link TextEditCornerHintPolicy} 判断放不放得下，放不下就不画。
 */
public final class TextEditHintButton extends KeyHintButton {
    private final String hint;
    private final Paint measure = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Rect ink = new Rect();

    public TextEditHintButton(Context context, String hint) {
        super(context);
        this.hint = TextPolicy.emptyIfNull(hint);
    }

    @Override protected void onLayout(boolean changed, int left, int top, int right, int bottom) {
        super.onLayout(changed, left, top, right, bottom);
        setCornerHint(fits() ? hint : "");
    }

    private boolean fits() {
        if (hint.isEmpty() || getWidth() <= 0 || getHeight() <= 0) return false;
        Context context = getContext();
        String label = getText().toString();
        Paint labelPaint = getPaint();
        labelPaint.getTextBounds(label, 0, label.length(), ink);
        float labelRight = (getWidth() + labelPaint.measureText(label)) / 2f;
        float labelTop = getBaseline() + ink.top;

        ViewPolicy.setTextSize(measure, KeyboardTypographyPolicy.keySp(context, CORNER_HINT_SP));
        measure.getTextBounds(hint, 0, hint.length(), ink);
        // 和 KeyHintButton 画角标的位置一致：基线在上边距加字体上升之下，右边贴右边距。
        float baseline = DimensionPolicy.floatPixels(context, CORNER_HINT_TOP_DP) - measure.getFontMetrics().ascent;
        float hintLeft = getWidth() - DimensionPolicy.floatPixels(context, CORNER_HINT_RIGHT_DP) - measure.measureText(hint);
        float hintBottom = baseline + ink.bottom;
        return TextEditCornerHintPolicy.fits(getWidth(), labelRight, labelTop, hintLeft, hintBottom,
            DimensionPolicy.floatPixels(context, 2));
    }
}
