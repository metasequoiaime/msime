package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.view.View;
import app.msime.android.DimensionPolicy;
import app.msime.android.KeyHeatmapLayout;
import app.msime.android.KeyboardActionRow;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TouchKeyStyle;
import app.msime.android.ViewPolicy;
import java.util.ArrayList;
import java.util.List;

/**
 * 「按键布局」页每个选项下的小图：按 {@link KeyboardActionRow#designEntries} 画出这种样式的字母层底行（不画地球键、不交换中/英与 123），键序和宽度份额与键盘完全相同。字符键用页面底色，功能键用强调浅色，回车用强调色。纯装饰，读屏读的是选项行自己的描述。
 *
 * <p>右侧大回车改的不只是底行，而是整个九键外框，只画底行看不出回车跨两行、符 在符号栏下方。这一项画外框的最后两行（{@link #tallReturnRows}）：九宫格第三行连同左栏和右列，加上底行，回车从上一行跨到底行。
 */
final class BottomRowPreview extends View {
    private static final float HEIGHT_DP = 34f;
    private static final float GAP_DP = 4f;
    private static final float RADIUS_DP = 5f;

    /** 小图里一个键的底色：字符键、功能键或回车。 */
    enum Tone { CHARACTER, FUNCTION, RETURN }

    /** 小图里的一个键：键面、宽度份额、底色，以及占几行（2 表示跨到下一行，0 表示被上一行跨下来的键盖住、只占宽度）。 */
    record Cell(String face, float weight, Tone tone, int rows) {}

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private final List<List<Cell>> rows;

    BottomRowPreview(Context context, int touchLayout, TouchKeyStyle.Row style) {
        this(context, touchLayout, style, false);
    }

    /** `symbolFirst`：「「符」与「123」互换」打开时 26 键的小图也把 符 画在 123 左边，与键盘相同。 */
    BottomRowPreview(Context context, int touchLayout, TouchKeyStyle.Row style, boolean symbolFirst) {
        super(context);
        rows = rows(touchLayout, style, symbolFirst);
        label.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTypeface(label, Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        ViewPolicy.setTextSizeSp(label, context, 12);
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
    }

    /** 小图画的行：右侧大回车是外框最后两行，其他样式是一行底行。 */
    static List<List<Cell>> rows(int touchLayout, TouchKeyStyle.Row style) {
        return rows(touchLayout, style, false);
    }

    static List<List<Cell>> rows(int touchLayout, TouchKeyStyle.Row style, boolean symbolFirst) {
        if (KeyboardActionRow.drawsTallReturn(touchLayout, style)) return tallReturnRows();
        List<Cell> row = new ArrayList<>();
        for (KeyboardActionRow.DesignEntry entry : KeyboardActionRow.designEntries(touchLayout, false, false, false, false,
                style, symbolFirst, TouchKeyStyle.NumberPadSpace.PLATFORM_DEFAULT)) {
            Tone tone = switch (entry.slot()) {
                case RETURN -> Tone.RETURN;
                case SPACE, COMMA, PERIOD, ZERO -> Tone.CHARACTER;
                default -> Tone.FUNCTION;
            };
            row.add(new Cell(face(entry.slot()), entry.weight(), tone, 1));
        }
        return List.of(List.copyOf(row));
    }

    /** 右侧大回车外框的最后两行，取自统计页热力图的同一套外框（{@link KeyHeatmapLayout#nine}），键序、宽度和跨行都与键盘一致；回车用强调色，记字符的键（左栏符号、九宫格、空格）用页面底色，其余是功能键。 */
    private static List<List<Cell>> tallReturnRows() {
        List<List<KeyHeatmapLayout.Key>> frame = KeyHeatmapLayout.nine(TouchKeyStyle.Row.TALL_RETURN);
        List<List<Cell>> rows = new ArrayList<>(2);
        for (List<KeyHeatmapLayout.Key> keys : frame.subList(frame.size() - 2, frame.size())) {
            List<Cell> row = new ArrayList<>(keys.size());
            for (KeyHeatmapLayout.Key key : keys) {
                String id = key.ids().isEmpty() ? "" : key.ids().get(0);
                Tone tone = "Enter".equals(id) ? Tone.RETURN
                    : id.startsWith("Nine") || "SoftPunctuation".equals(id) || "Space".equals(id) ? Tone.CHARACTER
                    : Tone.FUNCTION;
                row.add(new Cell(key.face(), key.weight(), tone, key.rows()));
            }
            rows.add(List.copyOf(row));
        }
        return List.copyOf(rows);
    }

    /** 底行一个键在预览里写的字。 */
    static String face(KeyboardActionRow.DesignSlot slot) {
        return switch (slot) {
            case SYMBOL_PANEL -> "符";
            case LAYER -> "123";
            case GLOBE -> "🌐";
            case EMOJI -> "😀";
            case COMMA -> "，";
            // 长空格样式的空格同样写「空格」，宽度已经说明它长，与 iOS、鸿蒙的预览一致。
            case SPACE -> "空格";
            case ZERO -> "0";
            case PERIOD -> "。";
            case LANGUAGE -> "中";
            case REWRITE -> "重写";
            case DELETE -> "⌫";
            case RETURN -> "↵";
        };
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        Context context = getContext();
        float height = rows.size() * HEIGHT_DP + (rows.size() - 1) * GAP_DP;
        setMeasuredDimension(MeasureSpec.getSize(widthSpec),
            resolveSize(DimensionPolicy.pixels(context, height), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        float gap = DimensionPolicy.pixels(context, GAP_DP);
        float radius = DimensionPolicy.pixels(context, RADIUS_DP);
        float rowHeight = (getHeight() - gap * (rows.size() - 1)) / rows.size();
        Paint.FontMetrics metrics = label.getFontMetrics();
        for (int r = 0; r < rows.size(); r++) {
            List<Cell> row = rows.get(r);
            float weights = 0;
            for (Cell cell : row) weights += cell.weight();
            // 和键盘的 LinearLayout 一样，按份额铺满整行。
            float unit = (getWidth() - gap * (row.size() - 1)) / weights;
            float top = r * (rowHeight + gap);
            float x = 0;
            for (Cell cell : row) {
                float width = unit * cell.weight();
                if (cell.rows() > 0) {
                    float height = cell.rows() * rowHeight + (cell.rows() - 1) * gap;
                    int background;
                    int ink;
                    switch (cell.tone()) {
                        case RETURN -> {
                            background = ThemeColorPolicy.accent(context);
                            ink = ThemeColorPolicy.onAccent(context);
                        }
                        case CHARACTER -> {
                            background = ThemeColorPolicy.page(context);
                            ink = ThemeColorPolicy.text(context);
                        }
                        default -> {
                            background = ThemeColorPolicy.accentSoft(context);
                            ink = ThemeColorPolicy.text(context);
                        }
                    }
                    fill.setColor(background);
                    box.set(x, top, x + width, top + height);
                    canvas.drawRoundRect(box, radius, radius, fill);
                    label.setColor(ink);
                    canvas.drawText(cell.face(), x + width / 2,
                        top + height / 2 - (metrics.ascent + metrics.descent) / 2, label);
                }
                x += width + gap;
            }
        }
    }
}
