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
 * 手写笔书写窗口（{@code InputMethodService#getStylusHandwritingWindow}）里唯一的视图：铺满屏幕、背景透明，只画笔迹和模型没就绪时的提示。手指碰不到这个窗口。识别、上屏和会话的结束由键盘服务决定。
 *
 * <p>笔的事件由服务的 {@code onStylusHandwritingMotionEvent} 直接交给 {@link #handle}，不走系统默认的派发：默认实现先把事件缓存起来，等视图「对用户可见」后再补发，同一个窗口第二次书写时缓存的事件不再补发（API 35 模拟器上整笔丢失），第一次补发也会夹着成串的 DOWN、CANCEL。所以笔迹按屏幕坐标记在 {@link #setArea} 给的屏幕大小里，与视图此刻有没有布局好无关，画的时候再按视图在屏幕上的位置平移。
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
    private int areaWidth;
    private int areaHeight;
    private final int[] location = new int[2];

    public StylusInkView(Context context) {
        super(context);
        setContentDescription("手写笔书写区域；停笔后识别并上屏");
        HandwritingStrokeRenderer.configure(stroke, DimensionPolicy.floatPixels(context, 3));
        hintBackground.setColor(0xE6303030);
        hintText.setColor(0xFFFFFFFF);
        hintText.setTextSize(DimensionPolicy.floatPixels(context, 14));
    }

    public void setListener(Listener value) { listener = value; }
    public void setAcceptsInk(boolean value) { acceptsInk = value; }
    public boolean hasInk() { return ink.hasInk(); }
    public long revision() { return ink.revision(); }
    public java.util.List<java.util.List<HandwritingInk.Point>> strokes() { return ink.snapshot(); }

    /** 书写区域，即屏幕大小（像素）。变了（旋转）时丢掉留着的笔迹：屏幕坐标全变了，识别出来也不是用户写的。 */
    public void setArea(int width, int height) {
        if (width == areaWidth && height == areaHeight) return;
        boolean changed = areaWidth > 0 && areaHeight > 0 && ink.clear();
        areaWidth = width;
        areaHeight = height;
        hintLayout = null;
        if (changed) changed();
    }

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

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        getLocationOnScreen(location);
        canvas.save();
        canvas.translate(-location[0], -location[1]);
        HandwritingStrokeRenderer.draw(canvas, stroke, strokePath, ink.strokesForDrawing());
        if (hint != null) drawHint(canvas);
        canvas.restore();
    }

    private void drawHint(Canvas canvas) {
        float margin = DimensionPolicy.floatPixels(getContext(), 16);
        float padding = DimensionPolicy.floatPixels(getContext(), 12);
        int width = areaWidth > 0 ? areaWidth : getWidth();
        int height = areaHeight > 0 ? areaHeight : getHeight();
        if (hintLayout == null) {
            int available = (int) Math.max(1, width - 2 * (margin + padding));
            int textWidth = (int) Math.min(Math.ceil(hintText.measureText(hint)), available);
            hintLayout = StaticLayout.Builder.obtain(hint, 0, hint.length(), hintText, Math.max(1, textWidth))
                .setAlignment(Layout.Alignment.ALIGN_NORMAL)
                .build();
        }
        float boxWidth = hintLayout.getWidth() + 2 * padding;
        float boxHeight = hintLayout.getHeight() + 2 * padding;
        float centerX = anchorX >= 0 ? anchorX : width / 2f;
        float left = BoundsPolicy.bounded(centerX - boxWidth / 2, margin,
            Math.max(margin, width - margin - boxWidth));
        // 放在落笔点上方，笔尖和手不挡字；上面放不下时放到下方。
        float gap = DimensionPolicy.floatPixels(getContext(), 24);
        float anchor = anchorY >= 0 ? anchorY : height / 2f;
        float top = anchor - gap - boxHeight;
        if (top < margin) top = Math.min(anchor + gap, Math.max(margin, height - margin - boxHeight));
        hintRect.set(left, top, left + boxWidth, top + boxHeight);
        float radius = DimensionPolicy.floatPixels(getContext(), 10);
        canvas.drawRoundRect(hintRect, radius, radius, hintBackground);
        canvas.save();
        canvas.translate(left + padding, top + padding);
        hintLayout.draw(canvas);
        canvas.restore();
    }

    /** 系统派发到视图的事件不处理，笔迹只从 {@link #handle} 来，免得同一笔记两次。 */
    @Override public boolean onTouchEvent(MotionEvent event) {
        return false;
    }

    /** 一个手写笔事件（服务的 {@code onStylusHandwritingMotionEvent}）。坐标换成屏幕坐标：事件自己的坐标系与屏幕之差取 {@code getRawX() - getX()}，历史采样点同样平移。 */
    public void handle(MotionEvent event) {
        if (areaWidth <= 0 || areaHeight <= 0) return;
        float dx = event.getRawX() - event.getX();
        float dy = event.getRawY() - event.getY();
        float x = event.getRawX();
        float y = event.getRawY();
        if (event.getActionMasked() == MotionEvent.ACTION_DOWN) {
            anchorX = x;
            anchorY = y;
            if (hint != null) invalidate();
        }
        if (!acceptsInk) return;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                // 上一笔没收到抬起（事件丢了）时先作废它，不让后面的每一笔都因「还在写」被拒。
                if (ink.isDrawing()) ink.cancel();
                if (!ink.begin(x, y, event.getEventTime(), areaWidth, areaHeight)) return;
                if (listener != null) listener.onStrokeBegan();
                invalidate();
            }
            case MotionEvent.ACTION_MOVE -> {
                if (!ink.isDrawing()) return;
                for (int index = 0; index < event.getHistorySize(); index++) {
                    ink.append(event.getHistoricalX(index) + dx, event.getHistoricalY(index) + dy,
                        event.getHistoricalEventTime(index), areaWidth, areaHeight);
                }
                ink.append(x, y, event.getEventTime(), areaWidth, areaHeight);
                invalidate();
            }
            case MotionEvent.ACTION_UP -> {
                if (!ink.isDrawing()) return;
                ink.append(x, y, event.getEventTime(), areaWidth, areaHeight);
                ink.finish();
                changed();
            }
            case MotionEvent.ACTION_CANCEL -> {
                if (ink.cancel()) changed();
            }
            default -> { }
        }
    }

    private void changed() {
        invalidate();
        if (listener != null) listener.onInkChanged(ink.revision(), ink.snapshot());
    }
}
