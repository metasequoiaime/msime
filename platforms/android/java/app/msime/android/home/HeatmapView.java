package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.R;
import app.msime.android.TypingStatisticsSummary.DayCount;
import java.util.List;

/**
 * 习惯页的近 12 周热力图：12 列（每列一周）× 7 行（每行一天），最早的一天在左上，今天在右下；每格按当天字数分 5 档，填 `?attr/msHeat0`…`msHeat4`，图下右侧是「少 □□□□□ 多」图例。
 *
 * <p>分档按这 84 天里最多的那天算：0 字是第 0 档，其余按占最多那天的比例落到 1–4 档，所以一个打字很少的人也能看出哪几天相对多。
 */
public final class HeatmapView extends View {
    private static final int COLUMNS = 12;
    private static final int ROWS = 7;
    private static final float GAP = 4f;
    private static final float LEGEND_HEIGHT = 26f;
    private static final float LEGEND_CELL = 10f;
    private static final int[] HEAT = {R.attr.msHeat0, R.attr.msHeat1, R.attr.msHeat2, R.attr.msHeat3,
        R.attr.msHeat4};

    private final Paint cell = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private List<DayCount> days = List.of();
    private long peak;

    public HeatmapView(Context context) {
        this(context, null);
    }

    public HeatmapView(Context context, @Nullable AttributeSet attributes) {
        super(context, attributes);
        label.setTextSize(Ui.dp(context, 11));
    }

    /** 换一组 84 天，最早的在前。 */
    public void setDays(List<DayCount> values) {
        days = values == null ? List.of() : List.copyOf(values);
        long highest = 0;
        int active = 0;
        for (DayCount day : days) {
            highest = Math.max(highest, day.count());
            if (day.count() > 0) active++;
        }
        peak = highest;
        setContentDescription("近 12 周输入热力图，" + active + " 天有输入，最多一天 " + highest + " 字");
        requestLayout();
        invalidate();
    }

    /** 这一天落在第几档（0–4）。 */
    static int level(long count, long peak) {
        if (count <= 0 || peak <= 0) return 0;
        double share = (double) count / peak;
        if (share > .75) return 4;
        if (share > .5) return 3;
        if (share > .25) return 2;
        return 1;
    }

    private float cellSize(float width) {
        return (width - Ui.dp(getContext(), GAP) * (COLUMNS - 1)) / COLUMNS;
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        int width = MeasureSpec.getSize(widthSpec);
        float size = cellSize(width);
        float gap = Ui.dp(getContext(), GAP);
        int height = Math.round(size * ROWS + gap * (ROWS - 1) + Ui.dp(getContext(), LEGEND_HEIGHT));
        setMeasuredDimension(width, resolveSize(height, heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        float gap = Ui.dp(context, GAP);
        float size = cellSize(getWidth());
        float radius = Ui.dp(context, 4);
        int total = COLUMNS * ROWS;
        // 不足 84 天时前面补空格，让今天始终落在右下角。
        List<DayCount> shown = days.subList(Math.max(0, days.size() - total), days.size());
        int offset = total - shown.size();
        for (int index = 0; index < total; index++) {
            int column = index / ROWS;
            int row = index % ROWS;
            int source = index - offset;
            long count = source >= 0 ? shown.get(source).count() : 0;
            cell.setColor(Ui.color(context, HEAT[level(count, peak)]));
            float left = column * (size + gap);
            float top = row * (size + gap);
            box.set(left, top, left + size, top + size);
            canvas.drawRoundRect(box, radius, radius, cell);
        }
        // 图例：少 □□□□□ 多，靠右。
        float legend = Ui.dp(context, LEGEND_CELL);
        float legendGap = Ui.dp(context, 3);
        float baseline = ROWS * (size + gap) - gap + Ui.dp(context, LEGEND_HEIGHT) - Ui.dp(context, 7);
        label.setColor(Ui.subText(context));
        float right = getWidth();
        float more = label.measureText("多");
        canvas.drawText("多", right - more, baseline, label);
        float x = right - more - Ui.dp(context, 4) - legend;
        float cellTop = baseline - legend + Ui.dp(context, 1);
        for (int level = HEAT.length - 1; level >= 0; level--) {
            cell.setColor(Ui.color(context, HEAT[level]));
            box.set(x, cellTop, x + legend, cellTop + legend);
            canvas.drawRoundRect(box, Ui.dp(context, 2), Ui.dp(context, 2), cell);
            x -= legend + legendGap;
        }
        float less = label.measureText("少");
        canvas.drawText("少", x + legend + legendGap - Ui.dp(context, 4) - less, baseline, label);
    }
}
