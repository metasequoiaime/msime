package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.text.Layout;
import android.text.StaticLayout;
import android.text.TextPaint;
import android.view.MotionEvent;
import android.view.View;

/**
 * 手写笔书写窗口（{@code InputMethodService#getStylusHandwritingWindow}）里唯一的视图：铺满屏幕、背景透明，只画笔迹和模型没就绪时的提示。系统把笔的事件派发到这里，坐标就是屏幕坐标；手指碰不到这个窗口。识别、上屏和会话的结束由键盘服务决定。
 */
public final class StylusInkView extends View {
    public interface Listener {
        void onStrokeBegan();
        void onInkChanged(long revision, java.util.List<java.util.List<HandwritingInk.Point>> strokes);
    }

    private final HandwritingInk ink = new HandwritingInk();
    private final Paint stroke = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint hintBackground = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final TextPaint hintText = new TextPaint(Paint.ANTI_ALIAS_FLAG);
    private final RectF hintRect = new RectF();
    private final Path strokePath = new Path();
    private Listener listener;
    private boolean acceptsInk = true;
    private String hint;
    private StaticLayout hintLayout;
    private float anchorX = -1;
    private float anchorY = -1;

    public StylusInkView(Context context) {
        super(context);
        setContentDescription("手写笔书写区域；停笔后识别并上屏");
        stroke.setStyle(Paint.Style.STROKE);
        stroke.setStrokeCap(Paint.Cap.ROUND);
        stroke.setStrokeJoin(Paint.Join.ROUND);
        stroke.setStrokeWidth(DimensionPolicy.floatPixels(context, 3));
        hintBackground.setColor(0xE6303030);
        hintText.setColor(0xFFFFFFFF);
        hintText.setTextSize(DimensionPolicy.floatPixels(context, 14));
    }

    public void setListener(Listener value) { listener = value; }
    public void setAcceptsInk(boolean value) { acceptsInk = value; }
    public boolean hasInk() { return ink.hasInk(); }
    public long revision() { return ink.revision(); }
    public java.util.List<java.util.List<HandwritingInk.Point>> strokes() { return ink.snapshot(); }

    /** 笔迹颜色（ARGB）与粗细（像素）；粗细不大于 0 时保持原来的粗细。 */
    public void setInk(int color, float widthPixels) {
        stroke.setColor(color);
        if (widthPixels > 0) stroke.setStrokeWidth(widthPixels);
        invalidate();
    }

    /** 在最近一次落笔处上方显示一句提示；null 收起提示。 */
    public void showHint(String text) {
        hint = text == null || text.isEmpty() ? null : text;
        hintLayout = null;
        invalidate();
    }

    public void clear() {
        boolean changed = ink.clear();
        if (hint != null) showHint(null);
        if (changed) changed();
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        super.onSizeChanged(width, height, oldWidth, oldHeight);
        hintLayout = null;
        // 旋转后屏幕坐标全变了，留着的笔迹画在错的地方，识别出来也不是用户写的。
        if (oldWidth > 0 && oldHeight > 0 && (width != oldWidth || height != oldHeight)
                && ink.clear()) changed();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        for (java.util.List<HandwritingInk.Point> points : ink.strokesForDrawing()) {
            if (points.isEmpty()) continue;
            if (points.size() == 1) {
                HandwritingInk.Point point = points.get(0);
                canvas.drawPoint(point.x(), point.y(), stroke);
                continue;
            }
            strokePath.reset();
            strokePath.moveTo(points.get(0).x(), points.get(0).y());
            for (int index = 1; index < points.size(); index++) {
                strokePath.lineTo(points.get(index).x(), points.get(index).y());
            }
            canvas.drawPath(strokePath, stroke);
        }
        if (hint != null) drawHint(canvas);
    }

    private void drawHint(Canvas canvas) {
        float margin = DimensionPolicy.floatPixels(getContext(), 16);
        float padding = DimensionPolicy.floatPixels(getContext(), 12);
        if (hintLayout == null) {
            int available = (int) Math.max(1, getWidth() - 2 * (margin + padding));
            int width = (int) Math.min(Math.ceil(hintText.measureText(hint)), available);
            hintLayout = StaticLayout.Builder.obtain(hint, 0, hint.length(), hintText, Math.max(1, width))
                .setAlignment(Layout.Alignment.ALIGN_NORMAL)
                .build();
        }
        float boxWidth = hintLayout.getWidth() + 2 * padding;
        float boxHeight = hintLayout.getHeight() + 2 * padding;
        float centerX = anchorX >= 0 ? anchorX : getWidth() / 2f;
        float left = BoundsPolicy.bounded(centerX - boxWidth / 2, margin,
            Math.max(margin, getWidth() - margin - boxWidth));
        // 放在落笔点上方，笔尖和手不挡字；上面放不下时放到下方。
        float gap = DimensionPolicy.floatPixels(getContext(), 24);
        float anchor = anchorY >= 0 ? anchorY : getHeight() / 2f;
        float top = anchor - gap - boxHeight;
        if (top < margin) top = Math.min(anchor + gap, Math.max(margin, getHeight() - margin - boxHeight));
        hintRect.set(left, top, left + boxWidth, top + boxHeight);
        float radius = DimensionPolicy.floatPixels(getContext(), 10);
        canvas.drawRoundRect(hintRect, radius, radius, hintBackground);
        canvas.save();
        canvas.translate(left + padding, top + padding);
        hintLayout.draw(canvas);
        canvas.restore();
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (getWidth() <= 0 || getHeight() <= 0) return false;
        if (event.getActionMasked() == MotionEvent.ACTION_DOWN) {
            anchorX = event.getX();
            anchorY = event.getY();
            if (hint != null) invalidate();
        }
        if (!acceptsInk) return true;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                if (!ink.begin(event.getX(), event.getY(), event.getEventTime(),
                        getWidth(), getHeight())) return true;
                if (listener != null) listener.onStrokeBegan();
                invalidate();
                return true;
            }
            case MotionEvent.ACTION_MOVE -> {
                if (!ink.isDrawing()) return true;
                for (int index = 0; index < event.getHistorySize(); index++) {
                    ink.append(event.getHistoricalX(index), event.getHistoricalY(index),
                        event.getHistoricalEventTime(index), getWidth(), getHeight());
                }
                ink.append(event.getX(), event.getY(), event.getEventTime(), getWidth(), getHeight());
                invalidate();
                return true;
            }
            case MotionEvent.ACTION_UP -> {
                if (!ink.isDrawing()) return true;
                ink.append(event.getX(), event.getY(), event.getEventTime(), getWidth(), getHeight());
                ink.finish();
                changed();
                performClick();
                return true;
            }
            case MotionEvent.ACTION_CANCEL -> {
                if (ink.cancel()) changed();
                return true;
            }
            default -> {
                return true;
            }
        }
    }

    @Override public boolean performClick() {
        super.performClick();
        return true;
    }

    private void changed() {
        invalidate();
        if (listener != null) listener.onInkChanged(ink.revision(), ink.snapshot());
    }
}
