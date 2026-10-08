package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.BoundsPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.ListPolicy;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.TypingStatisticsSummary.Share;
import app.msime.android.ViewPolicy;
import java.util.List;

/**
 * 统计页的三种占比图，都按同一组颜色给各段上色：第一段 accent，之后依次是 accent 在卡片色上 52%、28%、15% 的混色（原型的 `aM(n)`）。
 *
 * <ul>
 *   <li>{@link Style#STACK}：输入构成，一根通栏堆叠条，下面两列图例「● 汉字 82%」。</li>
 *   <li>{@link Style#BARS}：选词位置，每行「第 1 个」+ 轨道条 + 百分数。</li>
 *   <li>{@link Style#DONUT}：输入方式，左边环形图、环心写第一段的占比，右边一列图例。</li>
 * </ul>
 */
public final class DistributionView extends View {
    public enum Style { STACK, BARS, DONUT }

    private static final int[] MIX = {100, 52, 28, 15, 8};
    private static final float STACK_HEIGHT = 12f;
    private static final float LEGEND_ROW = 32f;
    private static final float BAR_ROW = 32f;
    private static final float DONUT = 96f;
    private static final float DONUT_STROKE = 14f;

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private final android.graphics.Path clip = new android.graphics.Path();
    private List<Share> shares = List.of();
    private long total;
    private Style style = Style.STACK;

    public DistributionView(Context context) {
        this(context, null);
    }

    public DistributionView(Context context, @Nullable AttributeSet attributes) {
        super(context, attributes);
    }

    /** 换一组占比和画法。 */
    public void setShares(List<Share> values, Style chart) {
        shares = ListPolicy.copyOrEmpty(values);
        total = TypingStatisticsSummary.total(shares);
        style = chart;
        int spokenCapacity = 0;
        for (Share share : shares) {
            spokenCapacity += share.title().length()
                + String.valueOf(TypingStatisticsSummary.share(share.count(), total)).length() + 2;
        }
        StringBuilder spoken = new StringBuilder(spokenCapacity);
        for (Share share : shares) {
            if (spoken.length() > 0) spoken.append('，');
            spoken.append(share.title()).append(' ')
                .append(TypingStatisticsSummary.share(share.count(), total)).append('%');
        }
        setContentDescription(spoken.length() == 0 ? "还没有记录" : spoken);
        requestLayout();
        invalidate();
    }

    /** 第 `index` 段的颜色。 */
    private int colour(int index) {
        Context context = getContext();
        int accent = Ui.accent(context);
        int mix = MIX[BoundsPolicy.atMost(index, MIX.length - 1)];
        return ColorPolicy.blend(Ui.card(context), accent, mix / 100f);
    }

    private int track() {
        boolean dark = Ui.isNight(getContext());
        return dark ? Ui.withAlpha(Color.WHITE, .1f) : Ui.withAlpha(Color.BLACK, .07f);
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        Context context = getContext();
        int rows = BoundsPolicy.bounded(shares.size(), 1, Integer.MAX_VALUE);
        float height = switch (style) {
            case STACK -> STACK_HEIGHT + 12 + LEGEND_ROW * ((rows + 1) / 2);
            case BARS -> BAR_ROW * rows;
            case DONUT -> BoundsPolicy.atLeast(LEGEND_ROW * rows, DONUT);
        };
        setMeasuredDimension(MeasureSpec.getSize(widthSpec),
            resolveSize(Ui.dp(context, height), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        if (shares.isEmpty()) return;
        switch (style) {
            case STACK -> drawStack(canvas);
            case BARS -> drawBars(canvas);
            case DONUT -> drawDonut(canvas);
        }
    }

    private void drawStack(Canvas canvas) {
        Context context = getContext();
        float height = Ui.dp(context, STACK_HEIGHT);
        float gap = Ui.dp(context, 2);
        float width = getWidth();
        fill.setColor(track());
        box.set(0, 0, width, height);
        canvas.drawRoundRect(box, height / 2, height / 2, fill);
        canvas.save();
        clip.reset();
        clip.addRoundRect(box, height / 2, height / 2, android.graphics.Path.Direction.CW);
        canvas.clipPath(clip);
        float x = 0;
        for (int index = 0; index < shares.size(); index++) {
            float part = total <= 0 ? 0 : width * shares.get(index).count() / (float) total;
            fill.setColor(colour(index));
            box.set(x, 0, BoundsPolicy.atLeast(
                x + part - (index < shares.size() - 1 ? gap : 0), x), height);
            canvas.drawRect(box, fill);
            x += part;
        }
        canvas.restore();
        float top = height + Ui.dp(context, 12);
        float column = width / 2;
        for (int index = 0; index < shares.size(); index++) {
            float left = (index % 2) * column;
            float rowTop = top + (index / 2) * Ui.dp(context, LEGEND_ROW);
            legend(canvas, index, left, rowTop, column - Ui.dp(context, index % 2 == 0 ? 16 : 0));
        }
    }

    private void drawBars(Canvas canvas) {
        Context context = getContext();
        float row = Ui.dp(context, BAR_ROW);
        float labelWidth = Ui.dp(context, 66);
        float percentWidth = Ui.dp(context, 52);
        float barHeight = Ui.dp(context, 10);
        float width = getWidth();
        for (int index = 0; index < shares.size(); index++) {
            Share share = shares.get(index);
            float middle = index * row + row / 2;
            styleText(14, Typeface.NORMAL, Ui.text(context));
            canvas.drawText(share.title(), 0, middle + textOffset(), text);
            float left = labelWidth;
            float right = width - percentWidth;
            fill.setColor(track());
            box.set(left, middle - barHeight / 2, right, middle + barHeight / 2);
            canvas.drawRoundRect(box, barHeight / 2, barHeight / 2, fill);
            float part = total <= 0 ? 0 : (right - left) * share.count() / (float) total;
            if (part > 0) {
                fill.setColor(colour(index));
            box.set(left, middle - barHeight / 2,
                left + BoundsPolicy.atLeast(part, barHeight), middle + barHeight / 2);
                canvas.drawRoundRect(box, barHeight / 2, barHeight / 2, fill);
            }
            styleText(14, Typeface.NORMAL, Ui.subText(context));
            String percent = TypingStatisticsSummary.share(share.count(), total) + "%";
            canvas.drawText(percent, width - text.measureText(percent), middle + textOffset(), text);
        }
    }

    private void drawDonut(Canvas canvas) {
        Context context = getContext();
        float size = Ui.dp(context, DONUT);
        float stroke = Ui.dp(context, DONUT_STROKE);
        float top = (getHeight() - size) / 2;
        box.set(stroke / 2, top + stroke / 2, size - stroke / 2, top + size - stroke / 2);
        fill.setStyle(Paint.Style.STROKE);
        fill.setStrokeWidth(stroke);
        fill.setColor(track());
        canvas.drawArc(box, 0, 360, false, fill);
        float start = -90;
        for (int index = 0; index < shares.size(); index++) {
            float sweep = total <= 0 ? 0 : 360f * shares.get(index).count() / total;
            fill.setColor(colour(index));
            canvas.drawArc(box, start, sweep, false, fill);
            start += sweep;
        }
        fill.setStyle(Paint.Style.FILL);
        Share first = shares.get(0);
        styleText(18, Typeface.BOLD, Ui.text(context));
        String percent = TypingStatisticsSummary.share(first.count(), total) + "%";
        canvas.drawText(percent, size / 2 - text.measureText(percent) / 2,
            top + size / 2, text);
        styleText(10, Typeface.NORMAL, Ui.subText(context));
        canvas.drawText(first.title(), size / 2 - text.measureText(first.title()) / 2,
            top + size / 2 + Ui.dp(context, 14), text);
        float left = size + Ui.dp(context, 20);
        float row = Ui.dp(context, LEGEND_ROW);
        float listTop = (getHeight() - row * shares.size()) / 2;
        for (int index = 0; index < shares.size(); index++) {
            legend(canvas, index, left, listTop + index * row, getWidth() - left);
        }
    }

    /** 一行图例：圆点、标题，右对齐的百分数。 */
    private void legend(Canvas canvas, int index, float left, float top, float width) {
        Context context = getContext();
        Share share = shares.get(index);
        float middle = top + Ui.dp(context, LEGEND_ROW) / 2;
        float dot = Ui.dp(context, 4);
        fill.setColor(colour(index));
        canvas.drawCircle(left + dot, middle, dot, fill);
        styleText(14, Typeface.NORMAL, Ui.text(context));
        canvas.drawText(share.title(), left + dot * 2 + Ui.dp(context, 10), middle + textOffset(), text);
        String percent = TypingStatisticsSummary.share(share.count(), total) + "%";
        canvas.drawText(percent, left + width - text.measureText(percent), middle + textOffset(), text);
    }

    private void styleText(int sizeSp, int weight, int colour) {
        ViewPolicy.setTextSizeSp(text, getContext(), sizeSp);
        text.setTypeface(Typeface.create(Typeface.DEFAULT, weight));
        text.setColor(colour);
    }

    private float textOffset() {
        Paint.FontMetrics metrics = text.getFontMetrics();
        return -(metrics.ascent + metrics.descent) / 2;
    }
}
