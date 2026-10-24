package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.view.accessibility.AccessibilityNodeInfo;

/** 文本编辑的换行键：上半区提示长按符号，分隔线下方保留换行主标签。 */
public final class TextEditReturnButton extends KeyboardPressButton {
    private final Paint labelPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint.FontMetrics metrics = new Paint.FontMetrics();

    public TextEditReturnButton(Context context) {
        super(context);
        labelPaint.setTextAlign(Paint.Align.CENTER);
    }

    @Override protected void onDraw(Canvas canvas) {
        // 背景和按压反馈由 View 绘制；这里只替代 Button 的居中文字，节点文字仍是“换行”。
        float width = getWidth(), height = getHeight();
        if (width <= 0 || height <= 0) return;
        float inset = Math.min(width / 8f, DimensionPolicy.floatPixels(getContext(), 8));
        float split = height * 0.45f;
        labelPaint.setColor(getCurrentTextColor());
        labelPaint.setTypeface(getTypeface());
        labelPaint.setStrokeWidth(DimensionPolicy.floatPixels(getContext(), 1));
        labelPaint.setAlpha(110);
        canvas.drawLine(inset, split, width - inset, split, labelPaint);
        labelPaint.setAlpha(200);
        drawLabel(canvas, "符号", 0, split, width - 2 * inset, getTextSize() * 0.75f);
        labelPaint.setAlpha(255);
        drawLabel(canvas, "换行", split, height, width - 2 * inset, getTextSize());
    }

    private void drawLabel(Canvas canvas, String label, float top, float bottom, float width, float size) {
        labelPaint.setTextSize(size);
        labelPaint.getFontMetrics(metrics);
        float scale = Math.min(1f, Math.min(width / labelPaint.measureText(label),
            (bottom - top) * 0.8f / (metrics.descent - metrics.ascent)));
        labelPaint.setTextSize(Math.max(1f, size * scale));
        labelPaint.getFontMetrics(metrics);
        canvas.drawText(label, getWidth() / 2f, (top + bottom - metrics.ascent - metrics.descent) / 2f, labelPaint);
    }

    @Override public void onInitializeAccessibilityNodeInfo(AccessibilityNodeInfo info) {
        super.onInitializeAccessibilityNodeInfo(info);
        info.addAction(new AccessibilityNodeInfo.AccessibilityAction(
            AccessibilityNodeInfo.ACTION_LONG_CLICK, "打开符号"));
    }
}
