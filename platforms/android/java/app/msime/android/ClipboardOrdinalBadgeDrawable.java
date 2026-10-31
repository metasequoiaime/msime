package app.msime.android;

import android.graphics.Canvas;
import android.graphics.ColorFilter;
import android.graphics.Paint;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.drawable.Drawable;

/**
 * 多选剪贴板里选中项右上角的序号角标（①②③④，#10411）。
 *
 * <p>画在卡片自己的浮层（`View.getOverlay()`）里，不做卡片旁边的兄弟 `View`：带阴影的皮肤会经 `ImeStyler.applyShadow` 给卡片抬高度，`FrameLayout` 按高度从低到高画子视图，高度为 0 的兄弟角标会被不透明的强调色卡片整个盖住。画在卡片浮层里，角标和卡片同一高度、总在卡片内容上面，左滑和按压缩放时也跟着卡片走。也不用 `TextView`：换肤遍历会把面板里所有 `TextView` 改成按键前景色，而角标压在选中卡片的强调色底上，要用强调色上的文字色才看得清。读屏不读它，序号由卡片的读屏说明「已选择第 N 项；」给出。
 */
public final class ClipboardOrdinalBadgeDrawable extends Drawable {
    private final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final String label;

    /** 序号文字由 {@link ClipboardSelectionPolicy#ordinalLabel} 给出，颜色由当前键盘皮肤传入。 */
    public ClipboardOrdinalBadgeDrawable(String label, int color) {
        this.label = label;
        text.setColor(color);
        text.setTextAlign(Paint.Align.CENTER);
    }

    @Override public void draw(Canvas canvas) {
        Rect bounds = getBounds();
        if (label.isEmpty() || bounds.isEmpty()) return;
        text.setTextSize(Math.min(bounds.width(), bounds.height()) * .85f);
        Paint.FontMetrics metrics = text.getFontMetrics();
        float baseline = bounds.exactCenterY() - (metrics.ascent + metrics.descent) / 2f;
        canvas.drawText(label, bounds.exactCenterX(), baseline, text);
    }

    @Override public void setAlpha(int alpha) {
        text.setAlpha(alpha);
        invalidateSelf();
    }

    @Override public void setColorFilter(ColorFilter filter) {
        text.setColorFilter(filter);
        invalidateSelf();
    }

    // Drawable 在最低支持版本上仍要求实现这个抽象方法，现代绘制管线不再查询它。
    @Deprecated
    @Override public int getOpacity() {
        return PixelFormat.TRANSLUCENT;
    }
}
