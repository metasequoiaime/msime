package app.msime.android.home;

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
import app.msime.android.KeyPressIds;
import app.msime.android.MapPolicy;
import app.msime.android.R;
import app.msime.android.TypingStatisticsSummary;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 按键页的迷你键盘热力图：26 键或 9 键布局，每个键按它占全部按键的比例分 5 档填 `?attr/msHeat0`…`msHeat4`，键面写键名和百分数；图下左边是「字母键最常按 N · 9.6%」，右边是「少 □□□□□ 多」图例。
 *
 * <p>分档按画出来的键里最多的那个算（同 {@link HeatmapView#level}），百分数按这段时间的全部按键算，所以实体键盘的 Tab、F1 这类没画出来的键也算在分母里，键面上的数字和存储里的口径一致。
 */
public final class KeyHeatmapView extends View {
    /** 一个画出来的键：键 id、宽度权重和键面。 */
    private record Key(String id, float weight, String face) {}

    private static final List<List<Key>> SOFT = List.of(
        row("KeyQ", "KeyW", "KeyE", "KeyR", "KeyT", "KeyY", "KeyU", "KeyI", "KeyO", "KeyP"),
        List.of(new Key(null, .5f, ""), k("KeyA"), k("KeyS"), k("KeyD"), k("KeyF"), k("KeyG"),
            k("KeyH"), k("KeyJ"), k("KeyK"), k("KeyL"), new Key(null, .5f, "")),
        List.of(new Key("ShiftLeft", 1.5f, "⇧"), k("KeyZ"), k("KeyX"), k("KeyC"), k("KeyV"),
            k("KeyB"), k("KeyN"), k("KeyM"), new Key("Backspace", 1.5f, "⌫")),
        List.of(new Key("SoftLayer", 1.25f, "123"), new Key("Comma", 1f, "，"),
            new Key("Space", 4.25f, "空格"), new Key("Period", 1f, "。"), new Key("SoftLanguage", 1f, "中"),
            new Key("Enter", 1.5f, "↵")));

    private static final List<List<Key>> NINE = List.of(
        List.of(new Key("Comma", 1f, "，"), new Key("Nine1", 1.4f, "@#"), new Key("Nine2", 1.4f, "ABC"),
            new Key("Nine3", 1.4f, "DEF"), new Key("Backspace", 1f, "⌫")),
        List.of(new Key("Period", 1f, "。"), new Key("Nine4", 1.4f, "GHI"), new Key("Nine5", 1.4f, "JKL"),
            new Key("Nine6", 1.4f, "MNO"), new Key("SoftSymbol", 1f, "符")),
        List.of(new Key("SoftPunctuation", 1f, "标点"), new Key("Nine7", 1.4f, "PQRS"),
            new Key("Nine8", 1.4f, "TUV"), new Key("Nine9", 1.4f, "WXYZ"), new Key("Nine0", 1f, "0")),
        List.of(new Key("SoftLayer", 1f, "123"), new Key("Space", 2.8f, "空格"),
            new Key("SoftLanguage", 1f, "中"), new Key("Enter", 1.4f, "↵")));

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

    private static Key k(String id) {
        return new Key(id, 1f, KeyPressIds.label(id));
    }

    private static List<Key> row(String... ids) {
        List<Key> keys = new ArrayList<>(ids.length);
        for (String id : ids) keys.add(k(id));
        return List.copyOf(keys);
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

    /** 图下左边那句：26 键说字母键里最常按的，9 键说带字母的格子里最常按的。 */
    private String headline() {
        Key best = null;
        long most = 0;
        for (List<Key> row : layout()) {
            for (Key key : row) {
                if (key.id() == null) continue;
                boolean candidate = nine ? key.id().startsWith("Nine") && !"Nine0".equals(key.id())
                    && !"Nine1".equals(key.id()) : key.id().startsWith("Key");
                long value = counts.getOrDefault(key.id(), 0L);
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
            resolveSize(Ui.dp(context, height), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        float gap = Ui.dp(context, GAP);
        float height = Ui.dp(context, KEY_HEIGHT);
        float radius = Ui.dp(context, 6);
        long peak = 0;
        for (List<Key> row : layout()) {
            for (Key key : row) {
                if (key.id() != null) peak = BoundsPolicy.atLeast(peak, counts.getOrDefault(key.id(), 0L));
            }
        }
        int text = ThemeColorPolicy.text(context);
        int sub = ThemeColorPolicy.subText(context);
        int onAccent = ThemeColorPolicy.onAccent(context);
        float width = getWidth();
        List<List<Key>> rows = layout();
        for (int r = 0; r < rows.size(); r++) {
            List<Key> row = rows.get(r);
            float weights = 0;
            int keys = 0;
            for (Key key : row) {
                weights += key.weight();
                if (key.id() != null) keys++;
            }
            float unit = (width - gap * (keys - 1)) / weights;
            float x = 0;
            float top = r * (height + gap);
            boolean drawn = false;
            for (Key key : row) {
                float keyWidth = unit * key.weight();
                if (key.id() == null) {
                    x += keyWidth;
                    continue;
                }
                if (drawn) x += gap;
                drawn = true;
                long value = counts.getOrDefault(key.id(), 0L);
                int level = HeatmapView.level(value, peak);
                fill.setColor(ThemeColorPolicy.color(context, HEAT[level]));
                box.set(x, top, x + keyWidth, top + height);
                canvas.drawRoundRect(box, radius, radius, fill);
                int ink = level >= 3 ? onAccent : text;
                face.setColor(ink);
                share.setColor(level >= 3 ? ColorPolicy.withAlpha(onAccent, .85f) : sub);
                float centre = x + keyWidth / 2;
                canvas.drawText(key.face(), centre, top + height / 2 + Ui.dp(context, 1), face);
                if (total > 0) {
                    String percent = TypingStatisticsSummary.percentTenths(value / (double) total) + "%";
                    canvas.drawText(percent, centre, top + height - Ui.dp(context, 6), share);
                }
                x += keyWidth;
            }
        }
        // 图下：左边最常按，右边图例。
        float baseline = rows.size() * (height + gap) - gap + Ui.dp(context, FOOTER) - Ui.dp(context, 9);
        caption.setColor(text);
        canvas.drawText(headline(), 0, baseline, caption);
        caption.setColor(sub);
        float more = caption.measureText("多");
        canvas.drawText("多", width - more, baseline, caption);
        float cell = Ui.dp(context, 10);
        float cellGap = Ui.dp(context, 3);
        float cellTop = baseline - cell + Ui.dp(context, 1);
        float cx = width - more - Ui.dp(context, 4) - cell;
        for (int level = HEAT.length - 1; level >= 0; level--) {
            fill.setColor(ThemeColorPolicy.color(context, HEAT[level]));
            box.set(cx, cellTop, cx + cell, cellTop + cell);
            canvas.drawRoundRect(box, Ui.dp(context, 2), Ui.dp(context, 2), fill);
            cx -= cell + cellGap;
        }
        float less = caption.measureText("少");
        canvas.drawText("少", cx + cell + cellGap - Ui.dp(context, 4) - less, baseline, caption);
    }
}
