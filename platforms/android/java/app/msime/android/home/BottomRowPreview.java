package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.view.View;
import app.msime.android.DimensionPolicy;
import app.msime.android.KeyboardActionRow;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.TouchKeyStyle;
import app.msime.android.ViewPolicy;
import java.util.List;

/**
 * 「按键布局」页每个选项下的小图：按 {@link KeyboardActionRow#designEntries} 画出这种样式的字母层底行（不画地球键、不交换中/英与 123），键序和宽度份额与键盘完全相同。字符键用页面底色，功能键用强调浅色，回车用强调色。纯装饰，读屏读的是选项行自己的描述。
 */
final class BottomRowPreview extends View {
    private static final float HEIGHT_DP = 34f;
    private static final float GAP_DP = 4f;
    private static final float RADIUS_DP = 5f;

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private final List<KeyboardActionRow.DesignEntry> entries;
    private final TouchKeyStyle.Row style;

    BottomRowPreview(Context context, int touchLayout, TouchKeyStyle.Row style) {
        super(context);
        this.style = style;
        entries = KeyboardActionRow.designEntries(touchLayout, false, false, false, false, style);
        label.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTypeface(label, Typeface.create(Typeface.DEFAULT, Typeface.BOLD));
        ViewPolicy.setTextSizeSp(label, context, 12);
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO);
    }

    /** 底行一个键在预览里写的字。 */
    static String face(KeyboardActionRow.DesignSlot slot, TouchKeyStyle.Row style) {
        return switch (slot) {
            case SYMBOL_PANEL -> "符";
            case LAYER -> "123";
            case GLOBE -> "🌐";
            case EMOJI -> "😀";
            case COMMA -> "，";
            case SPACE -> style == TouchKeyStyle.Row.LONG_SPACE ? "长空格" : "空格";
            case ZERO -> "0";
            case PERIOD -> "。";
            case LANGUAGE -> "中";
            case REWRITE -> "重写";
            case DELETE -> "⌫";
            case RETURN -> "↵";
        };
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        setMeasuredDimension(MeasureSpec.getSize(widthSpec),
            resolveSize(DimensionPolicy.pixels(getContext(), HEIGHT_DP), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        Context context = getContext();
        float gap = DimensionPolicy.pixels(context, GAP_DP);
        float radius = DimensionPolicy.pixels(context, RADIUS_DP);
        float weights = 0;
        for (KeyboardActionRow.DesignEntry entry : entries) weights += entry.weight();
        // 和键盘的 LinearLayout 一样，按份额铺满整行。
        float unit = (getWidth() - gap * (entries.size() - 1)) / weights;
        float height = getHeight();
        float x = 0;
        Paint.FontMetrics metrics = label.getFontMetrics();
        for (KeyboardActionRow.DesignEntry entry : entries) {
            float width = unit * entry.weight();
            int background;
            int ink;
            switch (entry.slot()) {
                case RETURN -> {
                    background = ThemeColorPolicy.accent(context);
                    ink = ThemeColorPolicy.onAccent(context);
                }
                case SPACE, COMMA, PERIOD, ZERO -> {
                    background = ThemeColorPolicy.page(context);
                    ink = ThemeColorPolicy.text(context);
                }
                default -> {
                    background = ThemeColorPolicy.accentSoft(context);
                    ink = ThemeColorPolicy.text(context);
                }
            }
            fill.setColor(background);
            box.set(x, 0, x + width, height);
            canvas.drawRoundRect(box, radius, radius, fill);
            label.setColor(ink);
            canvas.drawText(face(entry.slot(), style), x + width / 2,
                height / 2 - (metrics.ascent + metrics.descent) / 2, label);
            x += width + gap;
        }
    }
}
