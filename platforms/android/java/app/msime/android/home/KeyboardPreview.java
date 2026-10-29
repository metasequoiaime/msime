package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.DimensionPolicy;

import app.msime.android.KeyboardGeometry;
import app.msime.android.DrawablePolicy;
import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import app.msime.android.BoundsPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.KeyboardSkin;
import app.msime.android.ViewPolicy;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppEdition;
import app.msime.android.JsonPolicy;
import app.msime.android.NineKeySidebarPolicy;
import app.msime.android.SchemePreferences;
import app.msime.android.TouchKeyStyle;
import app.msime.android.core.InputViewValuePolicy;
import java.util.List;
import org.json.JSONObject;

/**
 * A still picture of the keyboard the user actually has.
 *
 * It is a picture and not the input view: the real one belongs to the InputMethodService, bound to an
 * editor and owning an Engine session, and a second instance of it here would be a second owner.
 *
 * <p>行取自 {@link KeyLayoutStylesPolicy.Preview}：「按键布局」页和键盘本身用的同一张表，按用户自己的共享偏好画所选样式的底行、顶部数字行、「符」与「123」互换和右侧大回车，再按本机的「交换语言键与数字键」对调 123 与中/英。以前这里写死了 `123 | ， | 空格 | 。 | 中 | ↵` 一种底行，用户换了样式后皮肤页画的和自己的键盘对不上。偏好由调用方在工作线程上读好（{@link #layout}）再传进来；拿不到偏好时画 {@link KeyLayoutStylesPolicy.Preview#STANDARD}。
 *
 * The skin comes from the shared preferences the card above already reads. Drawing fixed colours
 * under a caption that named the user's own layout and their own skin was a card that contradicted
 * itself.
 */
public final class KeyboardPreview extends View {
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF key = new RectF();
    private List<List<KeyLayoutStylesPolicy.Key>> rows = KeyLayoutStylesPolicy.Preview.STANDARD.rows();
    private String caption = "";
    @Nullable private KeyboardSkin skin;
    private float cornerRadiusDp = 16f;
    private boolean colorsValid;
    private int inkColor;
    private int letterCapColor;
    private int functionCapColor;
    private int returnCapColor;
    private int returnLabelColor;
    private int secondaryColor;

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
     * @param layout which keyboard and which rows to draw, read from the user's preferences ({@link #layout})
     * @param caption the short label on the space bar, may be empty
     */
    public void setKeyboard(@Nullable KeyboardSkin skin, KeyLayoutStylesPolicy.Preview layout, String caption) {
        this.skin = skin;
        this.rows = layout.rows();
        this.caption = caption == null ? "" : caption;
        colorsValid = false;
        applyBackground();
        invalidate();
    }

    /** {@link #setKeyboard(KeyboardSkin, KeyLayoutStylesPolicy.Preview, String)} without a space-bar label. */
    public void setKeyboard(@Nullable KeyboardSkin skin, KeyLayoutStylesPolicy.Preview layout) {
        setKeyboard(skin, layout, "");
    }

    /**
     * 从共享偏好读出预览要画的那一副（{@link KeyLayoutStylesPolicy#preview}）：画哪一类键盘与「按键布局」页默认停的那一类同一条规则，各类所选样式、顶部数字行、「符」与「123」互换、九键左侧符号和「中/英键放在空格左边」读的是键盘读的同几项，另加本机的「交换语言键与数字键」。会读本机设置文件，要在工作线程上调用。
     *
     * @param preferences 共享偏好（{@code KeyboardSheets.preferences}）；null 时按标准样式画，只保留本机的互换设置
     */
    static KeyLayoutStylesPolicy.Preview layout(Context context, @Nullable JSONObject preferences) {
        boolean swap = AndroidLocalSettings.load(context).bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS);
        if (preferences == null) {
            KeyLayoutStylesPolicy.Preview standard = KeyLayoutStylesPolicy.Preview.STANDARD;
            return new KeyLayoutStylesPolicy.Preview(standard.kind(), standard.styles(), false, false, false, swap);
        }
        AppEdition edition = AppEdition.current();
        return KeyLayoutStylesPolicy.preview(TouchKeyStyle.Groups.offeredBy(edition),
            SchemePreferences.storedScheme(preferences, edition),
            InputViewValuePolicy.textOr(preferences, "touch_shuangpin_layout", "twenty_six_key"),
            InputViewValuePolicy.textOr(preferences, "touch_wubi_layout", "twenty_six_key"),
            TouchKeyStyle.Styles.parse(
                JsonPolicy.strictString(preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE)),
                JsonPolicy.strictString(preferences.opt(TouchKeyStyle.FOURTEEN_KEY_PREFERENCE)),
                JsonPolicy.strictString(preferences.opt(TouchKeyStyle.NINE_KEY_PREFERENCE))),
            preferences.optBoolean(TouchKeyStyle.NUMBER_ROW_PREFERENCE, false),
            preferences.optBoolean(TouchKeyStyle.SYMBOL_FIRST_PREFERENCE, false),
            swap,
            NineKeySidebarPolicy.fromPreference(preferences.opt(NineKeySidebarPolicy.LETTER_PREFERENCE)),
            TouchKeyStyle.languageLeft(preferences.opt(TouchKeyStyle.LANGUAGE_KEY_SIDE_PREFERENCE)));
    }

    /** 圆角半径（dp）；默认 16，社区皮肤卡上的小预览用 6，与外面那圈描边对齐。 */
    public void setCornerRadiusDp(float radius) {
        cornerRadiusDp = BoundsPolicy.nonNegative(radius);
        applyBackground();
        invalidate();
    }

    private void applyBackground() {
        int radius = DimensionPolicy.pixels(getContext(), cornerRadiusDp);
        int base = skin == null ? ThemeColorPolicy.card(getContext()) : ColorPolicy.parse(skin.background(), ThemeColorPolicy.card(getContext()));
        android.graphics.drawable.GradientDrawable surface = DrawablePolicy.rounded(base, radius);
        // 设计皮肤的底是一道渐变，和键盘本身一样画出来；只画纯色时，深色设计上的功能键和回车显得格外跳。
        String end = skin != null && skin.designed() ? skin.gradientEnd() : null;
        if (end != null && !end.isEmpty()) {
            surface.setOrientation(skin.gradientHorizontal()
                ? android.graphics.drawable.GradientDrawable.Orientation.LEFT_RIGHT
                : android.graphics.drawable.GradientDrawable.Orientation.TOP_BOTTOM);
            surface.setColors(new int[] {base, ColorPolicy.parse(end, base)});
        }
        setBackground(surface);
        setClipToOutline(true);
    }

    /** 设计皮肤的字母键与功能键按其键帽不透明度叠在背景上，与键盘的 KeyboardSkinKeyDrawable 一致；回车不透明。 */
    private int withKeyOpacity(int colour) {
        if (skin == null || !skin.designed()) return colour;
        return ColorPolicy.withAlpha(colour, skin.keyOpacity());
    }

    private int ink() {
        resolveColors();
        return inkColor;
    }

    private int letterCap() {
        resolveColors();
        return letterCapColor;
    }

    private int functionCap() {
        resolveColors();
        return functionCapColor;
    }

    private int returnCap() {
        resolveColors();
        return returnCapColor;
    }

    private int returnLabel() {
        resolveColors();
        return returnLabelColor;
    }

    private int secondary() {
        resolveColors();
        return secondaryColor;
    }

    private void resolveColors() {
        if (colorsValid) return;
        Context context = getContext();
        if (skin == null) {
            inkColor = ThemeColorPolicy.text(context);
            letterCapColor = ThemeColorPolicy.page(context);
            functionCapColor = ThemeColorPolicy.accentSoft(context);
            returnCapColor = ThemeColorPolicy.accent(context);
            returnLabelColor = ThemeColorPolicy.onAccent(context);
            secondaryColor = ThemeColorPolicy.subText(context);
        } else {
            letterCapColor = ColorPolicy.parse(skin.keyBackground(), Color.WHITE);
            inkColor = ColorPolicy.parse(skin.keyForeground(), ThemeColorPolicy.text(context));
            functionCapColor = ColorPolicy.parse(skin.functionBackground(), letterCapColor);
            returnCapColor = ColorPolicy.parse(skin.returnBackground(), ThemeColorPolicy.accent(context));
            returnLabelColor = ColorPolicy.parse(skin.returnForeground(), Color.WHITE);
            secondaryColor = ColorPolicy.parse(skin.secondary(), ThemeColorPolicy.subText(context));
        }
        colorsValid = true;
    }

    /** 参考键盘的宽度：6 dp 边距 ×2、10 列 32 dp 键宽、9 个 5 dp 键距。 */
    private static final float REFERENCE_WIDTH_DP = 6f * 2 + 32f * 10 + 5f * 9;
    /** 参考键盘的高度：6 dp 边距 ×2、24 dp 候选条、4 行 46 dp（设计的键高）、3 个 5 dp 行距。 */
    private static final float REFERENCE_HEIGHT_DP = 6f * 2 + 24f + 46f * 4 + 5f * 3;

    /**
     * 布局没有给定高度时（wrap_content），按参考键盘的宽高比由宽度推出高度。
     *
     * 社区皮肤卡原先把预览框写死为 80 dp 高，手机上约 170 dp 宽，比键盘本身扁得多，键面被横向拉宽压扁。由宽度推高度后，缩略图与真实键盘同一比例。
     */
    @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
        if (MeasureSpec.getMode(heightMeasureSpec) == MeasureSpec.EXACTLY) {
            super.onMeasure(widthMeasureSpec, heightMeasureSpec);
            return;
        }
        int width = MeasureSpec.getSize(widthMeasureSpec);
        int height = Math.round(width * REFERENCE_HEIGHT_DP / REFERENCE_WIDTH_DP);
        if (MeasureSpec.getMode(heightMeasureSpec) == MeasureSpec.AT_MOST) {
            height = BoundsPolicy.atMost(height, MeasureSpec.getSize(heightMeasureSpec));
        }
        setMeasuredDimension(width, height);
    }

    /**
     * 按视图实际尺寸等比缩放的系数，上限 1。
     *
     * 边距、键距、候选条和字号是按设置首页那张整宽预览定的；社区皮肤卡上的缩略图只有约 170×106 dp，原样套用这些固定值时每行只剩几 dp 高，键面缩成细条而 13 dp 的字溢出到键外。以参考键盘为基准整体缩小；整宽预览不小于参考尺寸，系数为 1，外观不变。
     */
    private float scale() {
        return BoundsPolicy.atMost(DimensionPolicy.shorterSide(
            getWidth() / DimensionPolicy.floatPixels(getContext(), REFERENCE_WIDTH_DP),
            getHeight() / DimensionPolicy.floatPixels(getContext(), REFERENCE_HEIGHT_DP)), 1f);
    }

    /** 字号取设计值与键面能容下的较小者，保证标签不出键。 */
    private void fitText(String label, float designSize, RectF bounds) {
        float size = BoundsPolicy.atMost(designSize, bounds.height() * 0.62f);
        ViewPolicy.setTextSize(paint, size);
        float limit = bounds.width() * 0.86f;
        float measured = paint.measureText(label);
        if (measured > limit && measured > 0f) {
            ViewPolicy.setTextSize(paint, size * limit / measured);
        }
    }

    @Override protected void onDraw(Canvas canvas) {
        if (getWidth() <= 0 || getHeight() <= 0) return;
        float s = scale();
        // 缩小时按参考键盘的宽高比等比画，居中放进视图，空出的边露出皮肤底色；只缩不拉，键面不会被卡片的扁长比例压扁。整宽预览（s 为 1）仍铺满视图。
        float w = s < 1f ? DimensionPolicy.floatPixels(getContext(), REFERENCE_WIDTH_DP) * s : getWidth();
        float h = s < 1f ? DimensionPolicy.floatPixels(getContext(), REFERENCE_HEIGHT_DP) * s : getHeight();
        int saved = canvas.save();
        canvas.translate((getWidth() - w) / 2f, (getHeight() - h) / 2f);
        float pad = DimensionPolicy.floatPixels(getContext(), 6) * s;
        float gap = DimensionPolicy.floatPixels(getContext(), 5) * s;
        float stripHeight = DimensionPolicy.floatPixels(getContext(), 24) * s;
        float radius = (skin == null ? DimensionPolicy.floatPixels(getContext(), 6)
            : BoundsPolicy.atMost(DimensionPolicy.floatPixels(getContext(), (float) skin.cornerRadius()),
                DimensionPolicy.floatPixels(getContext(), 12))) * s;

        // 候选条：一个拼音和两枚候选，首选用强调色。
        float baseline = pad + stripHeight * 0.68f;
        paint.setTextAlign(Paint.Align.LEFT);
        ViewPolicy.setTextSizeSp(paint, getContext(), 12 * s);
        paint.setColor(secondary());
        canvas.drawText("ni hao", pad + DimensionPolicy.floatPixels(getContext(), 6) * s, baseline, paint);
        ViewPolicy.setTextSizeSp(paint, getContext(), 13 * s);
        paint.setColor(returnCap());
        canvas.drawText("你好", pad + DimensionPolicy.floatPixels(getContext(), 52) * s, baseline, paint);
        paint.setColor(ink());
        canvas.drawText("拟好", pad + DimensionPolicy.floatPixels(getContext(), 90) * s, baseline, paint);

        float top = pad + stripHeight;
        float available = h - top - pad;
        float rowHeight = (available - gap * (rows.size() - 1)) / rows.size();
        paint.setTextAlign(Paint.Align.CENTER);
        for (int r = 0; r < rows.size(); r++) {
            List<KeyLayoutStylesPolicy.Key> row = rows.get(r);
            float y = top + r * (rowHeight + gap);
            float total = 0;
            for (KeyLayoutStylesPolicy.Key cell : row) total += cell.weight();
            // 和键盘的 LinearLayout 一样按份额铺满整行；跨两行的回车画成一整块，被它盖住的格和 18 键的缩进只占宽度。
            float unit = (w - pad * 2 - gap * (row.size() - 1)) / total;
            float x = pad;
            for (KeyLayoutStylesPolicy.Key cell : row) {
                float width = unit * cell.weight();
                if (cell.rows() > 0) {
                    float height = cell.rows() * rowHeight + (cell.rows() - 1) * gap;
                    boolean space = KeyLayoutStylesPolicy.SPACE.equals(cell.id());
                    List<String> stack = cell.stack();
                    if (stack.isEmpty()) {
                        drawKey(canvas, cell, space ? caption : cell.face(), x, y, width, height, radius, s);
                    } else {
                        // 9 键的左侧符号栏：整条栏的高度等分给栏里的每一项，与「按键布局」页的 KeyLayoutPreview 画法相同。
                        float item = (height - gap * (stack.size() - 1)) / stack.size();
                        for (int index = 0; index < stack.size(); index++)
                            drawKey(canvas, cell, stack.get(index), x, y + index * (item + gap), width, item, radius, s);
                    }
                }
                x += width + gap;
            }
        }
        canvas.restoreToCount(saved);
    }

    /** 一个键帽：按底色画圆角块，再把键面居中画上（空格键面是皮肤说明，用次要色）。 */
    private void drawKey(Canvas canvas, KeyLayoutStylesPolicy.Key cell, String label, float left, float top, float width,
            float height, float radius, float s) {
        key.set(left, top, left + width, top + height);
        boolean action = cell.tone() == KeyLayoutStylesPolicy.Tone.RETURN;
        boolean function = cell.tone() == KeyLayoutStylesPolicy.Tone.FUNCTION;
        paint.setColor(action ? returnCap() : withKeyOpacity(function ? functionCap() : letterCap()));
        canvas.drawRoundRect(key, radius, radius, paint);
        if (label.isEmpty()) return;
        boolean space = KeyLayoutStylesPolicy.SPACE.equals(cell.id());
        paint.setColor(action ? returnLabel() : space ? secondary() : ink());
        fitText(label, DimensionPolicy.floatPixels(getContext(), label.length() > 2 ? 10 : 13) * s, key);
        Paint.FontMetrics metrics = paint.getFontMetrics();
        canvas.drawText(label, key.centerX(), key.centerY() - (metrics.ascent + metrics.descent) / 2f, paint);
    }
}
