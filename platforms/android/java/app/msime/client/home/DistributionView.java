package app.msime.client.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.core.content.ContextCompat;
import app.msime.client.R;
import app.msime.client.TypingStatisticsModel;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * 一组占比：上面一张图，下面每行一个分类。
 *
 * <p>Three shapes, chosen the way the Apple app chooses them (`StatisticsCharts.swift`): a pie for
 * character types, where the question is what share each took; a donut for language modes, whose
 * hole carries the total; and a ranked bar for schemes, because that list runs to fifteen rows and
 * four of them would be slivers in a pie.
 *
 * <p>Drawn rather than charted, for the same reason the trend line is: three small charts are not
 * worth a charting dependency.
 *
 * <p>Styled after the design's Android statistics: the donut is a 168dp ring about a seventh of its diameter thick over a track, with the total in its hole, and each legend row is a 15sp label and share over a 6dp bar in the category's colour, so the bar itself is the key between chart and name. Every category is the accent at its own opacity, as the design draws them (1, .7, .45, .25 down the list, dc.html L1712), rather than a second hue.
 */
public final class DistributionView extends View {
    /** 这一块用哪种图。 */
    public enum Style { PIE, DONUT, RANK }

    private static final int MAX_ROWS = 16;
    /** The design's four opacities, one per row in list order. */
    private static final float[] OPACITY = {1f, .7f, .45f, .25f};
    /** The faintest step; longer lists spread evenly from full accent down to it. */
    private static final float FAINTEST = .25f;

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint ring = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint share = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint track = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint title = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint value = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint centre = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint caption = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint empty = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private final int accent;
    private List<TypingStatisticsModel.Slice> slices = List.of();
    private List<TypingStatisticsModel.Slice> ranked = List.of();
    private Style style = Style.RANK;
    private long total;

    public DistributionView(Context context, AttributeSet attributes) {
        super(context, attributes);
        accent = ContextCompat.getColor(context, R.color.forest);
        track.setColor(ContextCompat.getColor(context, R.color.hairline));
        ring.setStyle(Paint.Style.STROKE);
        ring.setStrokeCap(Paint.Cap.BUTT);
        title.setColor(ContextCompat.getColor(context, R.color.ink));
        title.setTextSize(dp(15f));
        share.setColor(ContextCompat.getColor(context, R.color.ink));
        share.setTextSize(dp(15f));
        share.setFakeBoldText(true);
        value.setColor(ContextCompat.getColor(context, R.color.text_secondary));
        value.setTextSize(dp(12f));
        centre.setColor(ContextCompat.getColor(context, R.color.ink));
        centre.setTextSize(dp(22f));
        centre.setFakeBoldText(true);
        caption.setColor(ContextCompat.getColor(context, R.color.text_secondary));
        caption.setTextSize(dp(11f));
        empty.setColor(ContextCompat.getColor(context, R.color.text_secondary));
        empty.setTextSize(dp(13f));
    }

    /**
     * Show one distribution.
     *
     * <p>The legend keeps the order the categories are declared in, so a category stays the same
     * opacity of the accent whatever it counts this week; only the ranked bars re-order, which is their point. An
     * empty 历史未分类 is the one row dropped -- it is an artefact of older versions, and printing it
     * at zero explains nothing.
     */
    public void setSlices(List<TypingStatisticsModel.Slice> values, Style chart) {
        style = chart == null ? Style.RANK : chart;
        List<TypingStatisticsModel.Slice> kept = new ArrayList<>();
        for (TypingStatisticsModel.Slice slice : values == null
                ? List.<TypingStatisticsModel.Slice>of() : values) {
            if (slice.count() > 0 || !"unknown".equals(slice.id())) kept.add(slice);
        }
        if (kept.size() > MAX_ROWS) kept = kept.subList(0, MAX_ROWS);
        slices = List.copyOf(kept);
        List<TypingStatisticsModel.Slice> order = new ArrayList<>();
        for (TypingStatisticsModel.Slice slice : slices) if (slice.count() > 0) order.add(slice);
        order.sort((left, right) -> Long.compare(right.count(), left.count()));
        ranked = List.copyOf(order);
        total = TypingStatisticsModel.sum(slices);
        describe();
        requestLayout();
        invalidate();
    }

    /**
     * Say the rows out loud.
     *
     * <p>Everything this view shows is drawn, so without this there is nothing here for a screen
     * reader to read -- the chart would be a blank rectangle between two headings.
     */
    private void describe() {
        if (total <= 0) {
            setContentDescription("这一段时间还没有记录");
            return;
        }
        StringBuilder text = new StringBuilder();
        for (TypingStatisticsModel.Slice slice : slices) {
            if (slice.count() <= 0) continue;
            if (text.length() > 0) text.append('，');
            text.append(slice.title()).append(' ').append(slice.count()).append(" 字符");
            text.append(String.format(Locale.ROOT, "，占 %.1f%%", 100.0 * slice.count() / total));
        }
        setContentDescription(text.toString());
    }

    private float dp(float value) { return value * getResources().getDisplayMetrics().density; }

    private float rowHeight() { return dp(46f); }

    private float diameter() { return dp(168f); }

    /** The chart above the legend: a fixed square for the two round ones, a row each for the bars. */
    private float chartHeight() {
        if (total <= 0 && style != Style.RANK) return dp(28f);
        return switch (style) {
            case PIE, DONUT -> diameter();
            case RANK -> ranked.isEmpty() ? dp(28f) : ranked.size() * dp(30f) + dp(20f);
        };
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        int width = resolveSize((int) dp(240f), widthSpec);
        float height = chartHeight() + dp(12f) + Math.max(1, slices.size()) * rowHeight();
        setMeasuredDimension(width, resolveSize((int) height, heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        if (total <= 0 && ranked.isEmpty()) {
            canvas.drawText("这一段时间还没有记录", 0, dp(20f), empty);
            if (slices.isEmpty()) return;
        }
        float top = chartHeight() + dp(12f);
        switch (style) {
            case PIE -> drawSectors(canvas, 0f);
            case DONUT -> drawSectors(canvas, .62f);
            case RANK -> drawRanked(canvas);
        }
        for (int index = 0; index < slices.size(); index++) {
            TypingStatisticsModel.Slice slice = slices.get(index);
            float rowTop = top + index * rowHeight();
            float baseline = rowTop + dp(20f);
            int colour = colour(index);
            canvas.drawText(slice.title(), 0, baseline, title);
            String percent = total <= 0 ? "—"
                : String.format(Locale.ROOT, "%.1f%%", 100.0 * slice.count() / total);
            float percentWidth = share.measureText(percent);
            canvas.drawText(percent, getWidth() - percentWidth, baseline, share);
            String count = String.valueOf(slice.count());
            canvas.drawText(count, getWidth() - percentWidth - dp(10f) - value.measureText(count),
                baseline, value);
            // 名字下面那根条是图和名字之间唯一的连线，所以它必须和扇区同色、同顺序。
            float barTop = rowTop + dp(29f);
            box.set(0, barTop, getWidth(), barTop + dp(6f));
            canvas.drawRoundRect(box, dp(3f), dp(3f), track);
            if (total > 0 && slice.count() > 0) {
                float width = Math.max(dp(6f), getWidth() * slice.count() / (float) total);
                box.set(0, barTop, width, barTop + dp(6f));
                fill.setColor(colour);
                canvas.drawRoundRect(box, dp(3f), dp(3f), fill);
            }
        }
    }

    /** A pie, or a donut when `hole` is more than zero, in declaration order from twelve o'clock. */
    private void drawSectors(Canvas canvas, float hole) {
        if (total <= 0) return;
        float diameter = Math.min(diameter(), getWidth());
        float left = (getWidth() - diameter) / 2f;
        if (hole > 0) {
            drawRing(canvas, left, diameter);
            return;
        }
        box.set(left, 0, left + diameter, diameter);
        float start = -90f;
        for (int index = 0; index < slices.size(); index++) {
            TypingStatisticsModel.Slice slice = slices.get(index);
            if (slice.count() <= 0) continue;
            float sweep = 360f * slice.count() / total;
            fill.setColor(colour(index));
            // 相邻扇区之间留一线：贴在一起时，相邻两档透明度几乎看不出分界。
            float inset = Math.min(1.5f, sweep / 4f);
            canvas.drawArc(box, start + inset, Math.max(0f, sweep - inset * 2f), true, fill);
            start += sweep;
        }
    }

    /** The donut as the design draws it: a stroked ring rather than a pie with a hole punched in the surface colour. */
    private void drawRing(Canvas canvas, float left, float diameter) {
        float thickness = diameter * 5f / 36f;
        ring.setStrokeWidth(thickness);
        box.set(left + thickness / 2f, thickness / 2f, left + diameter - thickness / 2f,
            diameter - thickness / 2f);
        // The design lays the segments over a full track ring, so their opacity reads against the track rather than the card.
        ring.setColor(track.getColor());
        canvas.drawArc(box, 0f, 360f, false, ring);
        float start = -90f;
        for (int index = 0; index < slices.size(); index++) {
            TypingStatisticsModel.Slice slice = slices.get(index);
            if (slice.count() <= 0) continue;
            float sweep = 360f * slice.count() / total;
            ring.setColor(colour(index));
            // 相邻两段之间留一线，理由同扇形。
            float inset = Math.min(1f, sweep / 4f);
            canvas.drawArc(box, start + inset, Math.max(0f, sweep - inset * 2f), false, ring);
            start += sweep;
        }
        // 环心放总数：这一块要回答的是「一共多少、谁占大头」，总数就在图里，不用往上找。
        String amount = String.valueOf(total);
        canvas.drawText(amount, left + diameter / 2f - centre.measureText(amount) / 2f,
            diameter / 2f + dp(2f), centre);
        canvas.drawText("字符", left + diameter / 2f - caption.measureText("字符") / 2f,
            diameter / 2f + dp(20f), caption);
    }

    /** Ranked bars, largest first, each carrying its count at the end. */
    private void drawRanked(Canvas canvas) {
        if (ranked.isEmpty()) return;
        long peak = ranked.get(0).count();
        float rowHeight = dp(30f);
        float radius = dp(5f);
        for (int index = 0; index < ranked.size(); index++) {
            TypingStatisticsModel.Slice slice = ranked.get(index);
            float middle = dp(10f) + index * rowHeight + rowHeight / 2f;
            String count = String.valueOf(slice.count());
            float countWidth = value.measureText(count) + dp(8f);
            float right = Math.max(dp(24f), getWidth() - countWidth);
            box.set(0, middle - dp(9f), right, middle + dp(9f));
            canvas.drawRoundRect(box, radius, radius, track);
            float width = peak <= 0 ? 0 : (right) * slice.count() / (float) peak;
            box.set(0, middle - dp(9f), Math.max(width, dp(6f)), middle + dp(9f));
            fill.setColor(colour(position(slice)));
            canvas.drawRoundRect(box, radius, radius, fill);
            canvas.drawText(slice.title(), dp(8f), middle + dp(4.5f), title);
            canvas.drawText(count, getWidth() - value.measureText(count), middle + dp(4f), value);
        }
    }

    /** Where this slice sits in the declared order, so bar and legend agree on its colour. */
    private int position(TypingStatisticsModel.Slice slice) {
        for (int index = 0; index < slices.size(); index++) {
            if (slices.get(index).id().equals(slice.id())) return index;
        }
        return 0;
    }

    /**
     * The accent at the opacity of the category's place in the declared list: the design's four steps when there are four rows or fewer, otherwise an even spread over the same range, so a sixteen-scheme list still runs from full accent to the faintest step instead of repeating.
     */
    private int colour(int index) {
        int count = slices.size();
        float opacity = count <= OPACITY.length ? OPACITY[Math.min(index, OPACITY.length - 1)]
            : 1f - (1f - FAINTEST) * index / (float) (count - 1);
        return (Math.round(opacity * ((accent >>> 24) & 0xFF)) << 24) | (accent & 0x00FFFFFF);
    }
}
