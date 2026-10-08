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
import app.msime.android.TypingStatisticsSummary.DayCount;
import app.msime.android.ViewPolicy;
import java.time.LocalDate;
import java.time.format.DateTimeParseException;
import java.util.List;

/**
 * 概览英雄卡里的近 7 天柱状图：每天一根圆角柱，今天那根填 accent，其余填 `?attr/msStatBar`，柱下是星期。
 *
 * <p>柱高按 7 天里最多的那天归一，最高 118dp，最矮 4dp，这样没有输入的一天仍看得见是一根空柱，而不是缺了一天。
 */
public final class TrendChart extends View {
    private static final float BAR_MAX = 118f;
    private static final float BAR_MIN = 4f;
    private static final float BAR_GAP = 10f;
    private static final float LABEL_GAP = 8f;
    private static final float LABEL_SIZE = 12f;
    private static final String[] WEEKDAYS = {"一", "二", "三", "四", "五", "六", "日"};

    private final Paint bar = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private List<DayCount> days = List.of();

    public TrendChart(Context context) {
        this(context, null);
    }

    public TrendChart(Context context, @Nullable AttributeSet attributes) {
        super(context, attributes);
        label.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTextSizeSp(label, context, LABEL_SIZE);
    }

    /** 换一组 7 天；最后一项是今天。 */
    public void setDays(List<DayCount> values) {
        days = ListPolicy.copyOrEmpty(values);
        StringBuilder spoken = new StringBuilder("近 7 天每日字数");
        for (DayCount day : days) {
            spoken.append("，星期").append(weekday(day.day())).append(' ').append(day.count());
        }
        setContentDescription(spoken);
        invalidate();
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        Context context = getContext();
        // 星期标签按 sp 画，高度也按它实际的字号和下沉量算；按 dp 估算时系统字体一调大，标签下半截就落到控件外被裁掉。
        int height = Math.round(Ui.dp(context, BAR_MAX + LABEL_GAP + 2) + label.getTextSize()
            + label.getFontMetrics().descent);
        setMeasuredDimension(MeasureSpec.getSize(widthSpec), resolveSize(height, heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        if (days.isEmpty()) return;
        Context context = getContext();
        float gap = Ui.dp(context, BAR_GAP);
        float width = (getWidth() - gap * (days.size() - 1)) / days.size();
        float max = Ui.dp(context, BAR_MAX);
        float min = Ui.dp(context, BAR_MIN);
        float radius = Ui.dp(context, 6);
        long peak = 1;
        for (DayCount day : days) peak = BoundsPolicy.atLeast(peak, day.count());
        int accent = Ui.accent(context);
        int rest = Ui.color(context, R.attr.msStatBar);
        int text = Ui.text(context);
        int sub = Ui.subText(context);
        float labelBaseline = max + Ui.dp(context, LABEL_GAP) + label.getTextSize();
        for (int index = 0; index < days.size(); index++) {
            boolean today = index == days.size() - 1;
            float height = BoundsPolicy.atLeast(min, max * days.get(index).count() / (float) peak);
            float left = index * (width + gap);
            box.set(left, max - height, left + width, max);
            bar.setColor(today ? accent : rest);
            canvas.drawRoundRect(box, radius, radius, bar);
            label.setColor(today ? text : sub);
            canvas.drawText(weekday(days.get(index).day()), left + width / 2, labelBaseline, label);
        }
    }

    private static String weekday(String day) {
        try {
            return WEEKDAYS[LocalDate.parse(day).getDayOfWeek().getValue() - 1];
        } catch (DateTimeParseException error) {
            return "";
        }
    }
}
