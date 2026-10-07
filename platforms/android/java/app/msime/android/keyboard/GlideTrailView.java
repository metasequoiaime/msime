package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.view.View;

/** 滑行输入时画在键盘上的轨迹：一条圆头圆角的折线，抬手就清掉。它只画不收触摸。 */
public final class GlideTrailView extends View {
    private final Path path = new Path();
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private boolean empty = true;

    public GlideTrailView(Context context) {
        super(context);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setStrokeJoin(Paint.Join.ROUND);
        setClickable(false);
        setFocusable(false);
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
    }

    /** 开始一条新轨迹，(x, y) 是这个视图自己的坐标。 */
    public void start(int color, float widthPx, float x, float y) {
        paint.setColor(color);
        paint.setAlpha(200);
        paint.setStrokeWidth(widthPx);
        path.reset();
        path.moveTo(x, y);
        empty = false;
        invalidate();
    }

    public void lineTo(float x, float y) {
        if (empty) return;
        path.lineTo(x, y);
        invalidate();
    }

    public void clear() {
        if (empty) return;
        path.reset();
        empty = true;
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        if (!empty) canvas.drawPath(path, paint);
    }
}
