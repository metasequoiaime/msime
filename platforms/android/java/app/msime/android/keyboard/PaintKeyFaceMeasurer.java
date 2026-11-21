package app.msime.android;

import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.Typeface;
import java.util.EnumMap;
import java.util.Map;

/**
 * 用画笔量字：主标签用皮肤的字体，提示用系统默认字体（与原来的角标、双拼提示一样）。键组的内容域、绘制前的定位都经它量，所以量的和画的是同一支画笔、同一个字体。
 *
 * <p>只在主线程用；画笔改字号后量完不复原，调用方每次都先设字号。
 */
public final class PaintKeyFaceMeasurer implements KeyFaceMeasurer {
    private final Map<KeyFaceSlot, Paint> paints = new EnumMap<>(KeyFaceSlot.class);
    private final Rect bounds = new Rect();
    private final Typeface mainTypeface;

    public PaintKeyFaceMeasurer(Typeface mainTypeface) {
        this.mainTypeface = mainTypeface;
        for (KeyFaceSlot slot : KeyFaceSlot.values()) {
            Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
            paint.setTypeface(slot == KeyFaceSlot.MAIN ? mainTypeface : Typeface.DEFAULT);
            paint.setTextAlign(Paint.Align.LEFT);
            paints.put(slot, paint);
        }
    }

    public Typeface mainTypeface() { return mainTypeface; }

    /** 画这个位置用的画笔（绘制时设字号、颜色）。 */
    public Paint paint(KeyFaceSlot slot) { return paints.get(slot); }

    @Override public float advance(KeyFaceSlot slot, String text, float sizePx) {
        Paint paint = paints.get(slot);
        paint.setTextSize(sizePx);
        return paint.measureText(text);
    }

    @Override public Ink ink(KeyFaceSlot slot, String text, float sizePx) {
        Paint paint = paints.get(slot);
        paint.setTextSize(sizePx);
        paint.getTextBounds(text, 0, text.length(), bounds);
        return new Ink(bounds.left, bounds.top, bounds.right, bounds.bottom);
    }
}
