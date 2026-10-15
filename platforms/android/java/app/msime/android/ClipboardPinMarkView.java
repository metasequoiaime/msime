package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.view.View;

/** 固定剪贴板记录的角标：不占文字行宽，长文本也始终可见。 */
public final class ClipboardPinMarkView extends View {
    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint slash = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path corner = new Path();

    public ClipboardPinMarkView(Context context) {
        super(context);
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
        setContentDescription(null);
        fill.setStyle(Paint.Style.FILL);
        slash.setStyle(Paint.Style.STROKE);
        slash.setStrokeCap(Paint.Cap.ROUND);
        slash.setStrokeJoin(Paint.Join.ROUND);
        setPinned(false, 0);
    }

    /** 颜色由当前键盘皮肤传入；非固定时隐藏整个角标。 */
    public void setPinned(boolean pinned, int accent) {
        setVisibility(pinned ? VISIBLE : GONE);
        fill.setColor(accent);
        slash.setColor(0xFFFFFFFF);
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        if (getVisibility() != VISIBLE) return;
        float w = getWidth();
        float h = getHeight();
        corner.reset();
        corner.moveTo(w, 0);
        corner.lineTo(w, h);
        corner.lineTo(0, 0);
        corner.close();
        canvas.drawPath(corner, fill);
        float inset = Math.min(w, h) * .22f;
        slash.setStrokeWidth(Math.max(1f, Math.min(w, h) * .1f));
        canvas.drawLine(inset, inset, w - inset, h - inset, slash);
    }
}
