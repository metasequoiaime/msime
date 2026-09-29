package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.MotionEvent;
import android.view.View;

/** Touch canvas only; the service injects recognition and candidate presentation. */
public final class HandwritingCanvas extends View {
    public interface Listener {
        void onStrokeBegan();
        void onInkChanged(long revision, java.util.List<java.util.List<HandwritingInk.Point>> strokes);
    }

    private final HandwritingInk ink = new HandwritingInk();
    private final Paint background = new Paint();
    private final Paint guide = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint stroke = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF cardRect = new RectF();
    private Listener listener;
    private boolean acceptsInk = true;

    public HandwritingCanvas(Context context) { this(context, null); }

    public HandwritingCanvas(Context context, AttributeSet attributes) {
        super(context, attributes);
        setContentDescription("手写区域；用手指书写，停笔后选择候选文字");
        stroke.setStyle(Paint.Style.STROKE);
        stroke.setStrokeCap(Paint.Cap.ROUND);
        stroke.setStrokeJoin(Paint.Join.ROUND);
        stroke.setStrokeWidth(3 * getResources().getDisplayMetrics().density);
        guide.setStyle(Paint.Style.STROKE);
        guide.setStrokeWidth(getResources().getDisplayMetrics().density);
        applySkin(KeyboardSkin.system(false));
    }

    public void setListener(Listener value) { listener = value; }
    public void setAcceptsInk(boolean value) { acceptsInk = value; }
    public boolean hasInk() { return ink.hasInk(); }
    public long revision() { return ink.revision(); }
    public java.util.List<java.util.List<HandwritingInk.Point>> strokes() { return ink.snapshot(); }

    /** The card is visual guidance only; ink remains valid across the entire view. */
    public void setCardRect(float left, float top, float right, float bottom) {
        RectF next = new RectF(left, top, right, bottom);
        if (!cardRect.equals(next)) {
            cardRect.set(next);
            invalidate();
        }
    }

    public void applySkin(KeyboardSkin skin) {
        int keyBackground = Color.parseColor(skin.keyBackground());
        int foreground = Color.parseColor(skin.keyForeground());
        int accent = Color.parseColor(skin.accent());
        background.setColor(keyBackground);
        stroke.setColor(foreground);
        guide.setColor(Color.argb(31, Color.red(accent), Color.green(accent), Color.blue(accent)));
        invalidate();
    }

    public void undo() {
        if (ink.undo()) changed();
    }

    public void clear() {
        if (ink.clear()) changed();
    }

    @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
        super.onSizeChanged(width, height, oldWidth, oldHeight);
        if (oldWidth > 0 && oldHeight > 0 && (width != oldWidth || height != oldHeight)
                && ink.clear()) changed();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        RectF card = cardRect.isEmpty()
            ? new RectF(0, 0, getWidth(), getHeight()) : cardRect;
        float radius = 10 * getResources().getDisplayMetrics().density;
        canvas.drawRoundRect(card, radius, radius, background);
        float[] lines = {card.centerX(), card.top, card.centerX(), card.bottom,
            card.left, card.centerY(), card.right, card.centerY()};
        canvas.drawLines(lines, guide);
        for (java.util.List<HandwritingInk.Point> points : ink.snapshot()) {
            if (points.isEmpty()) continue;
            if (points.size() == 1) {
                HandwritingInk.Point point = points.get(0);
                canvas.drawPoint(point.x(), point.y(), stroke);
                continue;
            }
            Path path = new Path();
            path.moveTo(points.get(0).x(), points.get(0).y());
            for (int index = 1; index < points.size(); index++) {
                path.lineTo(points.get(index).x(), points.get(index).y());
            }
            canvas.drawPath(path, stroke);
        }
    }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (!acceptsInk || getWidth() <= 0 || getHeight() <= 0) return false;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                if (!ink.begin(event.getX(), event.getY(), event.getEventTime(),
                        getWidth(), getHeight())) return false;
                getParent().requestDisallowInterceptTouchEvent(true);
                if (listener != null) listener.onStrokeBegan();
                invalidate();
                return true;
            }
            case MotionEvent.ACTION_MOVE -> {
                for (int index = 0; index < event.getHistorySize(); index++) {
                    ink.append(event.getHistoricalX(index), event.getHistoricalY(index),
                        event.getHistoricalEventTime(index), getWidth(), getHeight());
                }
                ink.append(event.getX(), event.getY(), event.getEventTime(), getWidth(), getHeight());
                invalidate();
                return true;
            }
            case MotionEvent.ACTION_UP -> {
                ink.append(event.getX(), event.getY(), event.getEventTime(), getWidth(), getHeight());
                ink.finish();
                getParent().requestDisallowInterceptTouchEvent(false);
                changed();
                performClick();
                return true;
            }
            case MotionEvent.ACTION_CANCEL -> {
                ink.cancel();
                getParent().requestDisallowInterceptTouchEvent(false);
                changed();
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
