package app.msime.android;

import android.graphics.Paint;
import android.graphics.Typeface;
import android.text.StaticLayout;
import android.text.TextPaint;

/** 用与候选按钮相同的字体测量稳定行高，空闲时也保留相同空间。 */
final class CandidateStripSizing {
    private final MSIMEInputService service;
    private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
    private Typeface measuredTypeface;
    private float measuredSize = -1;
    private int measuredRows = -1;
    private int measuredHeight;

    CandidateStripSizing(MSIMEInputService service) { this.service = service; }

    int heightDp(int glossRows) {
        Typeface typeface = Typeface.create(service.imeStyler.candidateTypeface(), Typeface.BOLD);
        float size = KeyboardTypographyPolicy.keySp(service, service.candidateFontSize);
        CandidateAppearance.Layout layout = service.candidateAppearance.layout();
        int padding = layout == null || !Double.isFinite(layout.itemExtraHeight) ? 0
            : Math.max(0, 2 * (int) Math.ceil(Math.min(4, layout.itemExtraHeight / 2)));
        // 主字体的 FontMetrics 不含汉字回退字体；用与候选标签相同的排版规则测量并缓存。
        float content = contentHeight(glossRows, typeface, size) / DimensionPolicy.density(service);
        int height = CandidateStripHeightPolicy.heightDp(glossRows, service.candidateScalePercent, content, padding);
        int button = Math.min(service.pixels(height), service.pixels(CandidateChevronButton.BUTTON_DP));
        if (service.expandCandidates != null) ViewPolicy.setFixedHeight(service.expandCandidates, button);
        if (service.closePrediction != null) ViewPolicy.setFixedHeight(service.closePrediction, button);
        return height;
    }

    private int contentHeight(int glossRows, Typeface typeface, float size) {
        if (typeface.equals(measuredTypeface) && size == measuredSize && glossRows == measuredRows)
            return measuredHeight;
        textPaint.setTypeface(typeface);
        textPaint.setTextSize(size);
        String sample = "Ag汉あ한";
        StringBuilder annotation = new StringBuilder();
        for (int row = 0; row < glossRows; row++) {
            if (row > 0) annotation.append('\n');
            annotation.append(sample);
        }
        CharSequence label = service.candidateLabel("", sample, annotation.toString(), true);
        measuredHeight = StaticLayout.Builder.obtain(label, 0, label.length(), textPaint, service.pixels(4000))
            .setIncludePad(false).setUseLineSpacingFromFallbacks(true).build().getHeight();
        measuredTypeface = typeface;
        measuredSize = size;
        measuredRows = glossRows;
        return measuredHeight;
    }
}
