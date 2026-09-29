package app.msime.client.home;

import android.annotation.SuppressLint;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.MotionEvent;
import android.view.View;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import app.msime.client.R;
import java.time.LocalDate;
import java.time.format.DateTimeParseException;
import java.util.List;
import java.util.function.Consumer;

/**
 * One cell per day, one column per week, matching the Apple app's calendar.
 *
 * A cell can be tapped to scope the page to that one day. That is the only way to read a single
 * day's mix of character kinds and schemes: the distributions below otherwise cover the whole
 * record, where one day's shape is lost in the total.
 *
 * <p>Drawn the way the design has it: month labels over the weeks, r3 cells in five steps (an empty day in the hairline tone, then the accent at 30, 50, 75 and 100 percent), and a 少…多 legend under the grid. Steps rather than a continuous ramp because a reader can tell four greens apart and cannot tell forty.
 */
public final class HeatmapView extends View {
    private static final int ROWS = 7;
    /** Accent opacity of each step above empty. */
    private static final float[] LEVELS = {.3f, .5f, .75f, 1f};

    private final Paint cell = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint outline = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private int[] daily = new int[0];
    private List<String> days = List.of();
    @Nullable private Consumer<String> onDayPicked;
    @Nullable private String selected;
    /**
     * Where the finger went down, so {@link #performClick} knows which cell was meant.
     *
     * <p>The tap is not resolved at touch time. Consuming the gesture would take it away from the
     * scrolling page this calendar sits in, and a drag that started on the calendar would fail to
     * scroll; leaving it to the clickable View's own handling keeps that distinction where the
     * framework already makes it correctly.
     */
    private float downX;
    private float downY;

    public HeatmapView(Context context, AttributeSet attributes) {
        super(context, attributes);
        outline.setStyle(Paint.Style.STROKE);
        outline.setStrokeWidth(dp(1.5f));
        outline.setColor(ContextCompat.getColor(context, R.color.ink));
        label.setColor(ContextCompat.getColor(context, R.color.text_secondary));
        label.setTextSize(dp(10f));
    }

    public void setDaily(int[] daily) {
        this.daily = daily == null ? new int[0] : daily;
        invalidate();
    }

    /** The day key behind each cell, in the same order as {@link #setDaily}. */
    public void setDays(List<String> days) {
        this.days = days == null ? List.of() : days;
        invalidate();
    }

    /** Called with the tapped day, or with null when the tapped cell holds no record. */
    public void setOnDayPicked(@Nullable Consumer<String> listener) {
        onDayPicked = listener;
        setClickable(listener != null);
    }

    public void setSelected(@Nullable String day) {
        selected = day;
        invalidate();
    }

    private float dp(float value) { return value * getResources().getDisplayMetrics().density; }

    /** The strip above the grid that carries the month labels. */
    private float header() { return dp(16f); }

    /** The strip under the grid that carries the legend. */
    private float footer() { return dp(24f); }

    private float cellSize() {
        return (getHeight() - header() - footer() - dp(3f) * (ROWS - 1)) / ROWS;
    }

    private int columns() {
        float gap = dp(3f);
        return Math.max(1, (int) ((getWidth() + gap) / (cellSize() + gap)));
    }

    /** The index into {@link #daily} drawn at one cell, or -1 when that cell is before the record. */
    private int offsetAt(int column, int row) {
        int index = column * ROWS + row;
        int offset = daily.length - columns() * ROWS + index;
        return offset >= 0 && offset < daily.length ? offset : -1;
    }

    // Lint wants this override to call performClick itself. It is super.onTouchEvent that decides a
    // tap happened and calls it -- doing it here as well would fire the selection on a drag that
    // was only passing through on its way to scrolling the page.
    @SuppressLint("ClickableViewAccessibility")
    @Override public boolean onTouchEvent(MotionEvent event) {
        if (event.getActionMasked() == MotionEvent.ACTION_DOWN) {
            downX = event.getX();
            downY = event.getY();
        }
        return super.onTouchEvent(event);
    }

    @Override public boolean performClick() {
        boolean handled = super.performClick();
        if (onDayPicked == null) return handled;
        float gap = dp(3f);
        float size = cellSize();
        if (size <= 0) return handled;
        int column = (int) (downX / (size + gap));
        float gridY = downY - header();
        if (gridY < 0) return handled;
        int row = (int) (gridY / (size + gap));
        if (column < 0 || column >= columns() || row < 0 || row >= ROWS) return handled;
        int offset = offsetAt(column, row);
        onDayPicked.accept(offset >= 0 && offset < days.size() ? days.get(offset) : null);
        return true;
    }

    @Override protected void onDraw(Canvas canvas) {
        float gap = dp(3f);
        float size = cellSize();
        int columns = columns();
        int peak = 0;
        for (int value : daily) peak = Math.max(peak, value);
        if (size <= 0) return;
        float radius = dp(3f);
        float top = header();
        int forest = ContextCompat.getColor(getContext(), R.color.forest);
        int empty = ContextCompat.getColor(getContext(), R.color.hairline);

        for (int index = 0; index < columns * ROWS; index++) {
            int column = index / ROWS;
            int row = index % ROWS;
            float x = column * (size + gap);
            float y = top + row * (size + gap);
            int offset = offsetAt(column, row);
            int value = offset >= 0 ? daily[offset] : 0;
            cell.setColor(colour(level(value, peak), forest, empty));
            box.set(x, y, x + size, y + size);
            canvas.drawRoundRect(box, radius, radius, cell);
            if (selected != null && offset >= 0 && offset < days.size()
                    && selected.equals(days.get(offset))) {
                // Outlined rather than recoloured: the fill is the day's own count, and replacing it would hide the one number the selection is there to read.
                box.inset(dp(0.75f), dp(0.75f));
                canvas.drawRoundRect(box, radius, radius, outline);
            }
        }
        drawMonths(canvas, columns, size, gap);
        drawLegend(canvas, size, gap, forest, empty);
    }

    /** 0 for no record, otherwise 1 to 4 by the day's share of the busiest day; a quiet day still gets the first step so it never reads as no day at all. */
    private static int level(int value, int peak) {
        if (peak <= 0 || value <= 0) return 0;
        return Math.max(1, Math.min(LEVELS.length, (int) Math.ceil(LEVELS.length * value / (double) peak)));
    }

    private static int colour(int level, int accent, int empty) {
        if (level <= 0) return empty;
        return Color.argb(Math.round(255 * LEVELS[level - 1]), Color.red(accent), Color.green(accent),
            Color.blue(accent));
    }

    /** A month's label over the first week it starts in, skipped when the previous label would run into it. */
    private void drawMonths(Canvas canvas, int columns, float size, float gap) {
        int previous = -1;
        float free = 0f;
        float baseline = header() - dp(5f);
        for (int column = 0; column < columns; column++) {
            int offset = offsetAt(column, 0);
            if (offset < 0 || offset >= days.size()) continue;
            int month;
            try {
                month = LocalDate.parse(days.get(offset)).getMonthValue();
            } catch (DateTimeParseException error) {
                continue;
            }
            if (month == previous) continue;
            previous = month;
            float x = column * (size + gap);
            if (x < free) continue;
            String text = month + " 月";
            canvas.drawText(text, x, baseline, label);
            free = x + label.measureText(text) + dp(6f);
        }
    }

    /** 少, the five steps, 多; right-aligned under the grid. */
    private void drawLegend(Canvas canvas, float size, float gap, int accent, int empty) {
        float swatch = Math.min(size, dp(11f));
        float bottom = getHeight() - dp(4f);
        float right = getWidth();
        float more = label.measureText("多");
        canvas.drawText("多", right - more, bottom - dp(1f), label);
        float x = right - more - dp(6f) - swatch;
        for (int level = LEVELS.length; level >= 0; level--) {
            cell.setColor(colour(level, accent, empty));
            box.set(x, bottom - swatch, x + swatch, bottom);
            canvas.drawRoundRect(box, dp(2.5f), dp(2.5f), cell);
            x -= swatch + gap;
        }
        float less = label.measureText("少");
        canvas.drawText("少", x + swatch + gap - dp(6f) - less, bottom - dp(1f), label);
    }
}
