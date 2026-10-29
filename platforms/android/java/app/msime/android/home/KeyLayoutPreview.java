package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.view.View;
import app.msime.android.ColorPolicy;
import app.msime.android.DimensionPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import java.util.List;
import java.util.Set;

/**
 * 「按键布局」页的整块键盘预览：按 {@link KeyLayoutStylesPolicy#keyboard} 逐行画出所选样式下的整副键盘，每行按份额铺满，跨两行的回车画成一整块，被盖住的格和缩进只占宽度。键盘底色、键帽色和回车的强调色取应用当前主题；这个样式与「标准」不同的键（{@link KeyLayoutStylesPolicy#highlights}）外面描一圈强调色。
 *
 * <p>只是示意，不可点，读屏跳过它：胶囊按钮自己有名字和选中状态，说明写在胶囊下面。
 */
final class KeyLayoutPreview extends View {
    private static final float MIN_HEIGHT_DP = 180f;
    private static final float MAX_HEIGHT_DP = 220f;
    /** 高度随宽度等比：360dp 宽的手机上内容区约 328dp，预览约 190dp 高。 */
    private static final float HEIGHT_RATIO = .58f;
    private static final float PADDING_DP = 6f;
    private static final float GAP_DP = 4f;
    private static final float KEY_RADIUS_DP = 5f;
    private static final float RING_DP = 1.5f;
    private static final float MAX_TEXT_SP = 15f;
    private static final float MIN_TEXT_SP = 8f;

    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint ring = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF box = new RectF();
    private List<List<KeyLayoutStylesPolicy.Key>> rows = List.of();
    private Set<String> highlights = Set.of();

    KeyLayoutPreview(Context context) {
        super(context);
        label.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTypeface(label, Typeface.create(Typeface.DEFAULT, Typeface.NORMAL));
        ring.setStyle(Paint.Style.STROKE);
        ring.setStrokeWidth(DimensionPolicy.pixels(context, RING_DP));
        setImportantForAccessibility(IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
    }

    /** 换一副键盘和要高亮的键，立即重画；高度只随宽度变，切样式或开关数字行时页面不跳。 */
    void show(List<List<KeyLayoutStylesPolicy.Key>> keyboard, Set<String> highlighted) {
        rows = keyboard;
        highlights = highlighted;
        invalidate();
    }

    @Override protected void onMeasure(int widthSpec, int heightSpec) {
        Context context = getContext();
        int width = MeasureSpec.getSize(widthSpec);
        float height = Math.max(DimensionPolicy.pixels(context, MIN_HEIGHT_DP),
            Math.min(DimensionPolicy.pixels(context, MAX_HEIGHT_DP), width * HEIGHT_RATIO));
        setMeasuredDimension(width, resolveSize(Math.round(height), heightSpec));
    }

    @Override protected void onDraw(Canvas canvas) {
        if (rows.isEmpty()) return;
        Context context = getContext();
        boolean dark = AppMode.dark(context);
        int board = ThemeColorPolicy.card(context);
        // 浅色主题键帽用最亮的表面色，深色主题从卡片色往正文色提一点，键帽都比键盘底色亮，和真键盘一样。
        int character = dark ? ColorPolicy.blend(board, ThemeColorPolicy.text(context), .16f)
            : ThemeColorPolicy.rowBackground(context);
        int function = ThemeColorPolicy.accentSoft(context);
        int accent = ThemeColorPolicy.accent(context);
        int ink = ThemeColorPolicy.text(context);
        int onAccent = ThemeColorPolicy.onAccent(context);

        float padding = DimensionPolicy.pixels(context, PADDING_DP);
        float gap = DimensionPolicy.pixels(context, GAP_DP);
        float radius = DimensionPolicy.pixels(context, KEY_RADIUS_DP);
        float frameRadius = DimensionPolicy.pixels(context, Ui.GROUP_RADIUS);
        float ringOffset = DimensionPolicy.pixels(context, RING_DP);
        fill.setColor(board);
        box.set(0, 0, getWidth(), getHeight());
        canvas.drawRoundRect(box, frameRadius, frameRadius, fill);

        float innerWidth = getWidth() - 2 * padding;
        float rowHeight = (getHeight() - 2 * padding - gap * (rows.size() - 1)) / rows.size();
        float maxText = Math.min(DimensionPolicy.sp(context, MAX_TEXT_SP), rowHeight * .42f);
        float minText = DimensionPolicy.sp(context, MIN_TEXT_SP);
        ring.setColor(accent);
        for (int r = 0; r < rows.size(); r++) {
            List<KeyLayoutStylesPolicy.Key> row = rows.get(r);
            float weights = 0;
            for (KeyLayoutStylesPolicy.Key key : row) weights += key.weight();
            // 和键盘的 LinearLayout 一样，按份额铺满整行。
            float unit = (innerWidth - gap * (row.size() - 1)) / weights;
            float top = padding + r * (rowHeight + gap);
            float x = padding;
            for (KeyLayoutStylesPolicy.Key key : row) {
                float width = unit * key.weight();
                if (key.rows() > 0) {
                    float height = key.rows() * rowHeight + (key.rows() - 1) * gap;
                    int background = switch (key.tone()) {
                        case RETURN -> accent;
                        case FUNCTION -> function;
                        case CHARACTER -> character;
                    };
                    fill.setColor(background);
                    box.set(x, top, x + width, top + height);
                    canvas.drawRoundRect(box, radius, radius, fill);
                    if (highlights.contains(key.id())) {
                        // 描边画在键外面一圈，回车本身是强调色时也看得出被高亮。
                        box.inset(-ringOffset, -ringOffset);
                        canvas.drawRoundRect(box, radius + ringOffset, radius + ringOffset, ring);
                    }
                    drawFace(canvas, key.face(), x, top, width, height, maxText, minText,
                        key.tone() == KeyLayoutStylesPolicy.Tone.RETURN ? onAccent : ink);
                }
                x += width + gap;
            }
        }
    }

    /** 键面居中，放不下时缩小字号（最小 8sp），「空格」「重输」这类两个字的键面在窄键上也不出界。 */
    private void drawFace(Canvas canvas, String face, float left, float top, float width, float height,
            float maxText, float minText, int color) {
        if (face.isEmpty()) return;
        label.setTextSize(maxText);
        float available = width - DimensionPolicy.floatPixels(getContext(), 4);
        float measured = label.measureText(face);
        if (measured > available && measured > 0) label.setTextSize(Math.max(minText, maxText * available / measured));
        label.setColor(color);
        Paint.FontMetrics metrics = label.getFontMetrics();
        canvas.drawText(face, left + width / 2, top + height / 2 - (metrics.ascent + metrics.descent) / 2, label);
    }
}
