package app.msime.android.home;

import app.msime.android.DimensionPolicy;

import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.BoundsPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.KeyHeatmapLayout;
import app.msime.android.KeyHeatmapLayout.Key;
import app.msime.android.MapPolicy;
import app.msime.android.R;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.ViewPolicy;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * 按键页的迷你键盘热力图：竖屏默认设置下的中文全拼 26 键或拼音九键，键序、宽度、键面和每个键记的 id 都来自 {@link KeyHeatmapLayout}（它直接读触屏键盘的布局常量），每个键按它占全部按键的比例分 5 档填 `?attr/msHeat0`…`msHeat4`，键面写键名和百分数；图下左边是「字母键最常按 N · 9.6%」，右边是「少 □□□□□ 多」图例。
 *
 * <p>分档按画出来的键里最多的那个算（同 {@link HeatmapView#level}），百分数按这段时间的全部按键算，所以实体键盘的 Tab、F1 这类没画出来的键也算在分母里，键面上的数字和存储里的口径一致。
 *
 * <p>九键左栏的「，。？」和右列的「！」都记 `SoftPunctuation`，存储分不出按的是哪一个：这几个键涂同一档颜色，哪个键上都不印百分数（{@link KeyHeatmapLayout#sharedIds}），只印在一个键上会被读成那个键自己的占比。「重输」按了不计数，画成 0 次那一档、不印百分数。
 */
public final class KeyHeatmapView extends View {
    private static final List<List<Key>> SOFT = KeyHeatmapLayout.SOFT;
    private static final List<List<Key>> NINE = KeyHeatmapLayout.NINE;
    private static final Set<String> SOFT_SHARED = KeyHeatmapLayout.sharedIds(SOFT);
    private static final Set<String> NINE_SHARED = KeyHeatmapLayout.sharedIds(NINE);

    private static final int[] HEAT = {R.attr.msHeat0, R.attr.msHeat1, R.attr.msHeat2, R.attr.msHeat3,
        R.attr.msHeat4};
    private static final float KEY_HEIGHT = 44f;
    private static final float GAP = 5f;
    private static final float FOOTER = 30f;

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint face = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint share = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint caption = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private Map<String, Long> counts = Map.of();
    private long total;
    private boolean nine;

    public KeyHeatmapView(Context context) {
        this(context, null);
    }

    public KeyHeatmapView(Context context, @Nullable AttributeSet attributes) {
        super(context, attributes);
        face.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTypeface(face, Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        ViewPolicy.setTextSizeSp(face, context, 14);
        share.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTextSizeSp(share, context, 9);
        ViewPolicy.setTextSizeSp(caption, context, 12);
    }

    /** 这段时间每个键的按键次数。 */
    public void setKeys(Map<String, Long> values) {
        counts = MapPolicy.copyOrEmpty(values);
        long sum = 0;
        for (long value : counts.values()) sum += BoundsPolicy.nonNegative(value);
        total = sum;
        describe();
        invalidate();
    }

    /** 画 9 键还是 26 键。 */
    public void setNineKey(boolean value) {
        nine = value;
        describe();
        requestLayout();
        invalidate();
    }

    public boolean nineKey() { return nine; }

    /** 9 键格子的总按键数是否多过字母键，用来决定第一次打开时画哪套布局。 */
    public static boolean prefersNine(Map<String, Long> values) {
        long letters = 0;
        long cells = 0;
        for (Map.Entry<String, Long> entry : values.entrySet()) {
            if (entry.getKey().startsWith("Key")) letters += entry.getValue();
            if (entry.getKey().startsWith("Nine")) cells += entry.getValue();
        }
        return cells > letters;
    }

    private List<List<Key>> layout() { return nine ? NINE : SOFT; }

    /** 一个键的按键次数：它记过的每个 id 相加；不计数的键是 0。 */
    private long value(Key key) {
        long sum = 0;
        for (String id : key.ids()) sum += BoundsPolicy.nonNegative(counts.getOrDefault(id, 0L));
        return sum;
    }

    private static boolean containsAny(Set<String> set, List<String> ids) {
        for (String id : ids) if (set.contains(id)) return true;
        return false;
    }

    /** 图下左边那句：26 键说字母键里最常按的，9 键说带字母的格子里最常按的。 */
    private String headline() {
        Key best = null;
        long most = 0;
        for (List<Key> row : layout()) {
            for (Key key : row) {
                if (key.ids().isEmpty()) continue;
                String id = key.ids().get(0);
                boolean candidate = nine ? id.startsWith("Nine") && !"Nine0".equals(id)
                    && !"Nine1".equals(id) : id.startsWith("Key");
                long value = value(key);
                if (candidate && value > most) {
                    best = key;
                    most = value;
                }
            }
        }
        if (best == null) return "这段时间还没有按键记录";
        String percent = TypingStatisticsSummary.percentTenths(most / (double) total) + "%";
        return (nine ? "最常按 " : "字母键最常按 ") + best.face() + " · " + percent;
    }

    private void describe() {
        setContentDescription((nine ? "9 键" : "26 键") + "按键热力图，" + headline());
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        Context context = getContext();
        int rows = layout().size();
        float height = rows * KEY_HEIGHT + (rows - 1) * GAP + FOOTER;
        setMeasuredDimension(MeasureSpec.getSize(widthSpec),
            resolveSize(DimensionPolicy.pixels(context, height), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        float gap = DimensionPolicy.pixels(context, GAP);
        float height = DimensionPolicy.pixels(context, KEY_HEIGHT);
        float radius = DimensionPolicy.pixels(context, 6);
        long peak = 0;
        for (List<Key> row : layout()) {
            for (Key key : row) peak = BoundsPolicy.atLeast(peak, value(key));
        }
        int text = ThemeColorPolicy.text(context);
        int sub = ThemeColorPolicy.subText(context);
        int onAccent = ThemeColorPolicy.onAccent(context);
        float width = getWidth();
        List<List<Key>> rows = layout();
        Set<String> shared = nine ? NINE_SHARED : SOFT_SHARED;
        for (int r = 0; r < rows.size(); r++) {
            List<Key> row = rows.get(r);
            float weights = 0;
            for (Key key : row) weights += key.weight();
            // 和键盘的 LinearLayout 一样，每一行按份额铺满整行。
            float unit = (width - gap * (row.size() - 1)) / weights;
            float x = 0;
            float top = r * (height + gap);
            for (int index = 0; index < row.size(); index++) {
                Key key = row.get(index);
                float keyWidth = unit * key.weight();
                if (index > 0) x += gap;
                long value = value(key);
                int level = HeatmapView.level(value, peak);
                fill.setColor(ThemeColorPolicy.color(context, HEAT[level]));
                box.set(x, top, x + keyWidth, top + height);
                canvas.drawRoundRect(box, radius, radius, fill);
                int ink = level >= 3 ? onAccent : text;
                face.setColor(ink);
                share.setColor(level >= 3 ? ColorPolicy.withAlpha(onAccent, .85f) : sub);
                float centre = x + keyWidth / 2;
                canvas.drawText(key.face(), centre, top + height / 2 + DimensionPolicy.pixels(context, 1), face);
                if (total > 0 && !key.ids().isEmpty() && !containsAny(shared, key.ids())) {
                    String percent = TypingStatisticsSummary.percentTenths(value / (double) total) + "%";
                    canvas.drawText(percent, centre, top + height - DimensionPolicy.pixels(context, 6), share);
                }
                x += keyWidth;
            }
        }
        // 图下：左边最常按，右边图例。
        float baseline = rows.size() * (height + gap) - gap + DimensionPolicy.pixels(context, FOOTER) - DimensionPolicy.pixels(context, 9);
        caption.setColor(text);
        canvas.drawText(headline(), 0, baseline, caption);
        caption.setColor(sub);
        float more = caption.measureText("多");
        canvas.drawText("多", width - more, baseline, caption);
        float cell = DimensionPolicy.pixels(context, 10);
        float cellGap = DimensionPolicy.pixels(context, 3);
        float cellTop = baseline - cell + DimensionPolicy.pixels(context, 1);
        float cx = width - more - DimensionPolicy.pixels(context, 4) - cell;
        for (int level = HEAT.length - 1; level >= 0; level--) {
            fill.setColor(ThemeColorPolicy.color(context, HEAT[level]));
            box.set(cx, cellTop, cx + cell, cellTop + cell);
            canvas.drawRoundRect(box, DimensionPolicy.pixels(context, 2), DimensionPolicy.pixels(context, 2), fill);
            cx -= cell + cellGap;
        }
        float less = caption.measureText("少");
        canvas.drawText("少", cx + cell + cellGap - DimensionPolicy.pixels(context, 4) - less, baseline, caption);
    }
}
