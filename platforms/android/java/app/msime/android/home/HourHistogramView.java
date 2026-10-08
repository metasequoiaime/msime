package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.BoundsPolicy;
import app.msime.android.R;
import app.msime.android.ListPolicy;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.TypingStatisticsSummary.PeakWindow;
import app.msime.android.ViewPolicy;
import java.util.List;

/**
 * 习惯页的 24 小时直方图：近 7 天每小时的字数一根细柱，高峰时段那两小时填 accent，其余填 `?attr/msStatBar`，柱下标 0 时、6、12、18、24。
 */
public final class HourHistogramView extends View {
    private static final float BAR_MAX = 56f;
    private static final float BAR_MIN = 3f;
    private static final float BAR_GAP = 3f;
    private static final float LABEL_GAP = 6f;
    private static final float LABEL_SIZE = 11f;
    private static final int[] TICKS = {0, 6, 12, 18, TypingStatisticsSummary.HOURS_PER_DAY};

    private final Paint bar = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private List<Long> hours = List.of();
    @Nullable private PeakWindow peak;

    public HourHistogramView(Context context) {
        this(context, null);
    }

    public HourHistogramView(Context context, @Nullable AttributeSet attributes) {
        super(context, attributes);
        ViewPolicy.setTextSizeSp(label, context, LABEL_SIZE);
    }

    /**
     * 换一组 24 小时。
     *
     * @param window 高峰时段，可空；为空时没有柱子高亮
     */
    public void setHours(List<Long> values, @Nullable PeakWindow window, @Nullable String spokenPeak) {
        hours = ListPolicy.copyOrEmpty(values);
        peak = window;
        String hoursLabel = TypingStatisticsSummary.HOURS_PER_DAY + " 小时输入分布";
        setContentDescription(spokenPeak == null ? hoursLabel + "，近 7 天还没有记录"
            : hoursLabel + "，最常在" + spokenPeak);
        invalidate();
    }

    private boolean highlighted(int hour) {
        if (peak == null) return false;
        int start = Math.floorMod(peak.start(), TypingStatisticsSummary.HOURS_PER_DAY);
        int end = Math.floorMod(peak.end(), TypingStatisticsSummary.HOURS_PER_DAY);
        if (start == end) return hour == start;
        return start < end ? hour >= start && hour < end : hour >= start || hour < end;
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        int height = Ui.dp(getContext(), BAR_MAX + LABEL_GAP + LABEL_SIZE + 4);
        setMeasuredDimension(MeasureSpec.getSize(widthSpec), resolveSize(height, heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        int count = TypingStatisticsSummary.HOURS_PER_DAY;
        float gap = Ui.dp(context, BAR_GAP);
        float width = (getWidth() - gap * (count - 1)) / count;
        float max = Ui.dp(context, BAR_MAX);
        float min = Ui.dp(context, BAR_MIN);
        float radius = Ui.dp(context, 2);
        long highest = 1;
        for (long value : hours) highest = BoundsPolicy.atLeast(highest, value);
        int accent = Ui.accent(context);
        int rest = Ui.color(context, R.attr.msStatBar);
        for (int hour = 0; hour < count; hour++) {
            long value = hour < hours.size() ? hours.get(hour) : 0;
            float height = BoundsPolicy.atLeast(min, max * value / (float) highest);
            float left = hour * (width + gap);
            box.set(left, max - height, left + width, max);
            bar.setColor(highlighted(hour) ? accent : rest);
            canvas.drawRoundRect(box, radius, radius, bar);
        }
        label.setColor(Ui.subText(context));
        float baseline = max + Ui.dp(context, LABEL_GAP) + label.getTextSize();
        for (int tick : TICKS) {
            String text = tick == 0 ? "0 时" : String.valueOf(tick);
            float x = tick * (width + gap);
            float measured = label.measureText(text);
            float left = tick == 0 ? 0 : tick == TypingStatisticsSummary.HOURS_PER_DAY
                ? getWidth() - measured : x - measured / 2;
            canvas.drawText(text, left, baseline, label);
        }
    }
}
