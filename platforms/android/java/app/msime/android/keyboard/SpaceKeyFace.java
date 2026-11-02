package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;

/**
 * 空格键：键面画 22 dp 的麦克风描边图标加方案短名（13 sp，kbSub），例如「全拼」「双拼 · 小鹤」「五笔 86」，英文模式是「space」。麦克风只在开着「长按空格语音输入」时画，由 {@link ImeBottomRow} 经 {@link #setShowsMic} 设置。
 *
 * <p>数字页（{@link #setNumberPadFace}）的空格只有一个功能键宽，主标改画空格符号 ␣，颜色取本键的文字色（数字页这一格按普通键着色，是皮肤的键前景色，不是 ⌫ 那样的功能键前景色）；麦克风缩到右上角作次要提示，和九键角标数字一样取皮肤的提示色、停用时同样调暗，也只在开着长按空格语音时画（#10059）。只画麦克风会让人以为这是点一下就说话的语音键。
 *
 * <p>节点 text 仍是「空格」（§2.8），只是不绘制；键帽背景照常由样式通道设置。麦克风与短名的颜色由调用方从皮肤取后传入。
 */
public final class SpaceKeyFace extends KeyboardPressButton {
    public static final float MIC_DP = 22f;
    public static final float LABEL_SP = 13f;
    /** 麦克风与短名之间的间距（dp）。 */
    public static final float GAP_DP = 4f;
    /** 数字页主标 ␣ 的边长（dp），与 {@link KeyboardIconKey#ICON_DP} 相同，和右列的 ⌫ 一样大。 */
    public static final float NUMBER_PAD_GLYPH_DP = KeyboardIconKey.ICON_DP;
    /** 数字页右上角麦克风提示的边长（dp）；麦克风在 24 格里约占 14 格高，画出来约 8 dp，与 10 sp 的九键角标数字相当。 */
    public static final float NUMBER_PAD_MIC_DP = 14f;

    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private String schemeLabel = "";
    private int faceColor = 0xFF56685A;
    private int cornerHintColor = 0xFF8A968C;
    private boolean showsMic = true;
    private boolean numberPadFace;
    private String transientLabel = "";

    public SpaceKeyFace(Context context) {
        super(context);
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        setAllCaps(false);
        label.setTextAlign(Paint.Align.LEFT);
    }

    /** 方案短名；空串时只画麦克风。 */
    public void setSchemeLabel(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (schemeLabel.equals(next)) return;
        schemeLabel = next;
        invalidate();
    }

    public String schemeLabel() { return schemeLabel; }

    /** 麦克风与短名的颜色（设计的 kbSub）。 */
    public void setFaceColor(int color) {
        if (faceColor == color) return;
        faceColor = color;
        invalidate();
    }

    /** 数字页右上角麦克风提示的颜色（皮肤的 hint，与九键角标数字同色）。 */
    public void setCornerHintColor(int color) {
        if (cornerHintColor == color) return;
        cornerHintColor = color;
        invalidate();
    }

    /** 手势进行中的临时键面文字（如「移动光标」）；非空时只画这段文字，不画麦克风与方案短名。空串恢复常态。 */
    public void setTransientLabel(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (transientLabel.equals(next)) return;
        transientLabel = next;
        invalidate();
    }

    /** 关掉长按空格语音时不画麦克风，只画短名。 */
    public void setShowsMic(boolean value) {
        if (showsMic == value) return;
        showsMic = value;
        invalidate();
    }

    /**
     * 空格键上的方案短名（设计的「全拼」「双拼 · 小鹤」「五笔 86」），英文模式是「space」。
     *
     * @param scheme 当前方案；null 按全拼
     * @param wubiProfile 五笔的码表版本（{@link KeyboardScheme#WUBI_86} / {@link KeyboardScheme#WUBI_98}）
     * @param english 是否处于英文输入
     */
    public static String schemeLabel(KeyboardScheme scheme, String wubiProfile, boolean english) {
        if (english) return "space";
        if (scheme == null) return "全拼";
        return switch (scheme) {
            case QUANPIN, QUANPIN_FOURTEEN_KEY, QUANPIN_EIGHTEEN_KEY, QUANPIN_NINE_KEY -> "全拼";
            case HANDWRITING -> "手写";
            case XIAOHE -> "双拼 · 小鹤";
            case ZIRANMA -> "双拼 · 自然码";
            case MICROSOFT -> "双拼 · 微软";
            case SHOUDAO -> "双拼 · 首道";
            case NINE_KEY_SHUANGPIN -> "九键双拼";
            case WUBI -> KeyboardScheme.WUBI_98.equals(KeyboardScheme.normalizedWubiProfile(wubiProfile))
                ? "五笔 98" : "五笔 86";
            case JAPANESE, JAPANESE_NINE_KEY -> "日语";
            case KOREAN -> "韩语";
            case CANTONESE -> "粤拼";
            case CANTONESE_SHUANGPIN -> "粤语双拼";
            case ZHUYIN, ZHUYIN_NINE_KEY -> "注音";
            case VIETNAMESE -> "越南语";
            case TIBETAN -> "藏文";
            case STROKE -> "笔画";
        };
    }

    /** 是否画数字页键面：主标 ␣，麦克风只作右上角的次要提示，不画方案短名。离开数字页时由 {@link ImeBottomRow} 设回 false。 */
    public void setNumberPadFace(boolean value) {
        if (numberPadFace == value) return;
        numberPadFace = value;
        invalidate();
    }

    /** 键面内容总宽：麦克风、间距与短名，供居中计算；纯算术，冒烟可测。 */
    public static float contentWidth(float micPx, float gapPx, float labelPx) {
        if (labelPx <= 0) return micPx;
        if (micPx <= 0) return labelPx;
        return micPx + gapPx + labelPx;
    }

    @Override protected void onDraw(Canvas canvas) {
        if (getWidth() <= 0 || getHeight() <= 0) return;
        // 拖动移光标时的「移动光标」照常盖住整个键面，数字页也一样。
        if (numberPadFace && transientLabel.isEmpty()) {
            drawNumberPadFace(canvas);
            return;
        }
        ViewPolicy.setTextSize(label, KeyboardTypographyPolicy.keySp(getContext(), LABEL_SP));
        label.setColor(faceColor);
        boolean overlay = !transientLabel.isEmpty();
        float mic = (showsMic && !overlay)
            ? BoundsPolicy.atMost(DimensionPolicy.floatPixels(getContext(), MIC_DP), getHeight()) : 0f;
        float gap = DimensionPolicy.floatPixels(getContext(), GAP_DP);
        float available = getWidth() - mic - gap - DimensionPolicy.floatPixels(getContext(), 8);
        String text = overlay ? transientLabel : schemeLabel;
        // 分离后的窄空格优先显示方案名，避免截成无法辨别方案的「双拼 ·」。
        int separator = text.lastIndexOf(" · ");
        if (!overlay && separator >= 0 && label.measureText(text) > available)
            text = text.substring(separator + 3);
        while (!text.isEmpty() && label.measureText(text) > available && text.length() > 1) {
            text = text.substring(0, text.length() - 1);
        }
        float textWidth = text.isEmpty() ? 0f : label.measureText(text);
        float total = contentWidth(mic, gap, textWidth);
        float left = (getWidth() - total) / 2f;
        float centerY = getHeight() / 2f;
        if (mic > 0) {
            KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.MIC, left,
                centerY - mic / 2f, mic, faceColor);
            left += mic + gap;
        }
        if (!text.isEmpty()) {
            Paint.FontMetrics metrics = label.getFontMetrics();
            canvas.drawText(text, left, centerY - (metrics.ascent + metrics.descent) / 2f, label);
        }
    }

    /** 数字页键面：居中画 ␣（本键文字色）；开着长按空格语音时在右上角画小麦克风，颜色与透明度同 {@link NineKeyDigitButton} 的角标数字，位置与九键角标数字（{@link KeyHintButton#CORNER_HINT_TOP_DP}、{@link KeyHintButton#CORNER_HINT_RIGHT_DP}）对齐。 */
    private void drawNumberPadFace(Canvas canvas) {
        float glyph = BoundsPolicy.atMost(DimensionPolicy.floatPixels(getContext(), NUMBER_PAD_GLYPH_DP),
            DimensionPolicy.shorterSide(getWidth(), getHeight()));
        int color = getCurrentTextColor();
        if (!isEnabled()) color = ColorPolicy.withAlpha(color, 96f / 255f);
        KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.SPACE_BAR, (getWidth() - glyph) / 2f,
            (getHeight() - glyph) / 2f, glyph, color);
        if (!showsMic) return;
        float mic = DimensionPolicy.floatPixels(getContext(), NUMBER_PAD_MIC_DP);
        KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.MIC,
            getWidth() - DimensionPolicy.floatPixels(getContext(), KeyHintButton.CORNER_HINT_RIGHT_DP) - mic,
            DimensionPolicy.floatPixels(getContext(), KeyHintButton.CORNER_HINT_TOP_DP), mic,
            ColorPolicy.withAlpha(cornerHintColor, ColorPolicy.enabledAlpha(isEnabled(), 255, 96)));
    }
}
