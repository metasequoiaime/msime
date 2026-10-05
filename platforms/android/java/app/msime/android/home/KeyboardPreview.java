package app.msime.android.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.KeyboardSkin;

/**
 * A still picture of the keyboard the user actually has.
 *
 * It is a picture and not the input view: the real one belongs to the InputMethodService, bound to an
 * editor and owning an Engine session, and a second instance of it here would be a second owner. The
 * rows are the faces the host actually ships, so the two do not drift apart silently -- the labels
 * come from arrays that mirror `KeyboardLayout`'s two layers.
 *
 * The skin and the grid come from the shared preferences the card above already reads. Drawing a
 * fixed nine-key grid in fixed colours under a caption that named the user's own 26-key layout and
 * their own skin was a card that contradicted itself.
 */
public final class KeyboardPreview extends View {
    /** 九键：左列符号，右列退格 / 重输 / 0 / 回车；底行与设计一致。 */
    private static final String[][] NINE_KEY_ROWS = {
        {"，", "@#", "ABC", "DEF", "⌫"},
        {"。", "GHI", "JKL", "MNO", "重输"},
        {"？", "PQRS", "TUV", "WXYZ", "0"},
        {"123", "中", "，", "空格", "。", "↵"},
    };
    /** 设计的全键盘：中文模式小写字母，底行 `123 | 中 | ， | 空格 | 。 | ↵`。 */
    private static final String[][] FULL_ROWS = {
        {"q", "w", "e", "r", "t", "y", "u", "i", "o", "p"},
        {"a", "s", "d", "f", "g", "h", "j", "k", "l"},
        {"⇧", "z", "x", "c", "v", "b", "n", "m", "⌫"},
        {"123", "中", "，", "空格", "。", "↵"},
    };
    /** 画成功能键底色的键面。 */
    private static final java.util.Set<String> FUNCTION_KEYS = java.util.Set.of(
        "⇧", "⌫", "123", "中", "重输", "@#", "，", "。", "？", "0");

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF key = new RectF();
    private String[][] rows = FULL_ROWS;
    private String caption = "";
    @Nullable private KeyboardSkin skin;
    private float cornerRadiusDp = 16f;

    /** Inflated from a layout, so it takes the two-argument constructor. */
    public KeyboardPreview(Context context, AttributeSet attributes) {
        super(context, attributes);
        applyBackground();
    }

    /** Built in code, for the community detail sheet, which has no layout of its own. */
    public KeyboardPreview(Context context) {
        super(context);
        applyBackground();
    }

    /**
     * Draw the user's own keyboard.
     *
     * @param skin the resolved skin, or null to keep the page's own palette
     * @param nineKey whether the grid is three columns rather than 26 keys
     * @param caption the short label on the space bar (the scheme), may be empty
     */
    public void setKeyboard(@Nullable KeyboardSkin skin, boolean nineKey, String caption) {
        this.skin = skin;
        this.rows = nineKey ? NINE_KEY_ROWS : FULL_ROWS;
        this.caption = caption == null ? "" : caption;
        applyBackground();
        invalidate();
    }

    /** {@link #setKeyboard(KeyboardSkin, boolean, String)} without a space-bar label. */
    public void setKeyboard(@Nullable KeyboardSkin skin, boolean nineKey) {
        setKeyboard(skin, nineKey, "");
    }

    /** 圆角半径（dp）；默认 16，社区皮肤卡上的小预览用 6，与外面那圈描边对齐。 */
    public void setCornerRadiusDp(float radius) {
        cornerRadiusDp = Math.max(0f, radius);
        applyBackground();
        invalidate();
    }

    private void applyBackground() {
        int radius = Ui.dp(getContext(), cornerRadiusDp);
        setBackground(Ui.rounded(skin == null ? Ui.card(getContext()) : parse(skin.background(), Ui.card(getContext())), radius));
        setClipToOutline(true);
    }

    private static int parse(@Nullable String colour, int fallback) {
        if (colour == null || colour.isEmpty()) return fallback;
        try {
            return Color.parseColor(colour);
        } catch (IllegalArgumentException error) {
            return fallback;
        }
    }

    private float dp(float value) {
        return value * getResources().getDisplayMetrics().density;
    }

    private int ink() {
        return skin == null ? Ui.text(getContext()) : parse(skin.keyForeground(), Ui.text(getContext()));
    }

    private int letterCap() {
        return skin == null ? Ui.page(getContext()) : parse(skin.keyBackground(), Color.WHITE);
    }

    private int functionCap() {
        return skin == null ? Ui.accentSoft(getContext())
            : parse(skin.functionBackground(), letterCap());
    }

    private int returnCap() {
        return skin == null ? Ui.accent(getContext()) : parse(skin.returnBackground(), Ui.accent(getContext()));
    }

    private int returnLabel() {
        return skin == null ? Ui.onAccent(getContext()) : parse(skin.returnForeground(), Color.WHITE);
    }

    private int secondary() {
        return skin == null ? Ui.subText(getContext()) : parse(skin.secondary(), Ui.subText(getContext()));
    }

    /** 参考键盘的宽度：6 dp 边距 ×2、10 列 32 dp 键宽、9 个 5 dp 键距。 */
    private static final float REFERENCE_WIDTH_DP = 6f * 2 + 32f * 10 + 5f * 9;
    /** 参考键盘的高度：6 dp 边距 ×2、24 dp 候选条、4 行 40 dp、3 个 5 dp 行距。 */
    private static final float REFERENCE_HEIGHT_DP = 6f * 2 + 24f + 40f * 4 + 5f * 3;

    /**
     * 按视图实际尺寸等比缩放的系数，上限 1。
     *
     * 边距、键距、候选条和字号是按设置首页那张整宽预览定的；社区皮肤卡上的缩略图只有约 150×75 dp，原样套用这些固定值时每行只剩几 dp 高，键面缩成细条而 13 dp 的字溢出到键外。以参考键盘为基准整体缩小；整宽预览不小于参考尺寸，系数为 1，外观不变。
     */
    private float scale() {
        return Math.min(1f, Math.min(getWidth() / dp(REFERENCE_WIDTH_DP), getHeight() / dp(REFERENCE_HEIGHT_DP)));
    }

    /** 字号取设计值与键面能容下的较小者，保证标签不出键。 */
    private void fitText(String label, float designSize, RectF bounds) {
        float size = Math.min(designSize, bounds.height() * 0.62f);
        paint.setTextSize(size);
        float limit = bounds.width() * 0.86f;
        float measured = paint.measureText(label);
        if (measured > limit && measured > 0f) paint.setTextSize(size * limit / measured);
    }

    @Override protected void onDraw(Canvas canvas) {
        if (getWidth() <= 0 || getHeight() <= 0) return;
        float s = scale();
        float pad = dp(6) * s;
        float gap = dp(5) * s;
        float stripHeight = dp(24) * s;
        float radius = (skin == null ? dp(6) : Math.min(dp((float) skin.cornerRadius()), dp(12))) * s;

        // 候选条：一个拼音和两枚候选，首选用强调色。
        float baseline = pad + stripHeight * 0.68f;
        paint.setTextAlign(Paint.Align.LEFT);
        paint.setTextSize(dp(12) * s);
        paint.setColor(secondary());
        canvas.drawText("ni hao", pad + dp(6) * s, baseline, paint);
        paint.setTextSize(dp(13) * s);
        paint.setColor(returnCap());
        canvas.drawText("你好", pad + dp(52) * s, baseline, paint);
        paint.setColor(ink());
        canvas.drawText("拟好", pad + dp(90) * s, baseline, paint);

        float top = pad + stripHeight;
        float available = getHeight() - top - pad;
        float rowHeight = (available - gap * (rows.length - 1)) / rows.length;
        float unit = (getWidth() - pad * 2 - gap * 9) / 10f;
        paint.setTextAlign(Paint.Align.CENTER);
        for (int r = 0; r < rows.length; r++) {
            String[] row = rows[r];
            float y = top + r * (rowHeight + gap);
            boolean bottom = r == rows.length - 1;
            float[] widths = new float[row.length];
            float total = 0;
            for (int c = 0; c < row.length; c++) {
                float weight = bottom ? ("空格".equals(row[c]) ? 4f : "↵".equals(row[c]) || "123".equals(row[c]) ? 1.5f : 1f)
                    : rows == FULL_ROWS && r == 2 && ("⇧".equals(row[c]) || "⌫".equals(row[c])) ? 1.5f : 1f;
                widths[c] = weight;
                total += weight;
            }
            float rowWidth = getWidth() - pad * 2 - gap * (row.length - 1);
            // 全键盘第二行比第一行少一键，按设计居中缩进半个键位。
            float x = pad;
            if (rows == FULL_ROWS && r == 1) x = (getWidth() - (unit * 9 + gap * 8)) / 2f;
            for (int c = 0; c < row.length; c++) {
                float width = rows == FULL_ROWS && r == 1 ? unit : rowWidth * widths[c] / total;
                key.set(x, y, x + width, y + rowHeight);
                String face = row[c];
                boolean action = "↵".equals(face);
                boolean function = !action && FUNCTION_KEYS.contains(face);
                paint.setColor(action ? returnCap() : function ? functionCap() : letterCap());
                canvas.drawRoundRect(key, radius, radius, paint);
                String label = "空格".equals(face) ? caption : face;
                if (!label.isEmpty()) {
                    paint.setColor(action ? returnLabel() : "空格".equals(face) ? secondary() : ink());
                    fitText(label, dp(label.length() > 2 ? 10 : 13) * s, key);
                    Paint.FontMetrics metrics = paint.getFontMetrics();
                    canvas.drawText(label, key.centerX(), key.centerY() - (metrics.ascent + metrics.descent) / 2f, paint);
                }
                x += width + gap;
            }
        }
    }
}
