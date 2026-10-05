package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.util.TypedValue;

/**
 * 空格键：键面画 22 dp 的麦克风描边图标加方案短名（13 sp，kbSub），例如「全拼」「双拼 · 小鹤」「五笔 86」，英文模式是「space」。
 *
 * <p>节点 text 仍是「空格」（§2.8），只是不绘制；键帽背景照常由样式通道设置。麦克风与短名的颜色由调用方从皮肤取后传入。
 */
public final class SpaceKeyFace extends KeyboardPressButton {
    public static final float MIC_DP = 22f;
    public static final float LABEL_SP = 13f;
    /** 麦克风与短名之间的间距（dp）。 */
    public static final float GAP_DP = 4f;

    private final Paint icon = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint label = new Paint(Paint.ANTI_ALIAS_FLAG);
    private String schemeLabel = "";
    private int faceColor = 0xFF56685A;
    private boolean showsMic = true;

    public SpaceKeyFace(Context context) {
        super(context);
        setAllCaps(false);
        label.setTextAlign(Paint.Align.LEFT);
    }

    /** 方案短名；空串时只画麦克风。 */
    public void setSchemeLabel(String value) {
        String next = value == null ? "" : value;
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

    /** 语音不可用时可以只画短名。 */
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
            case QUANPIN, QUANPIN_NINE_KEY -> "全拼";
            case HANDWRITING -> "手写";
            case XIAOHE -> "双拼 · 小鹤";
            case ZIRANMA -> "双拼 · 自然码";
            case MICROSOFT -> "双拼 · 微软";
            case SHOUDAO -> "双拼 · 首道";
            case WUBI -> KeyboardScheme.WUBI_98.equals(KeyboardScheme.normalizedWubiProfile(wubiProfile))
                ? "五笔 98" : "五笔 86";
            case JAPANESE, JAPANESE_NINE_KEY -> "日语";
            case KOREAN -> "韩语";
            case CANTONESE -> "粤拼";
            case ZHUYIN -> "注音";
            case VIETNAMESE -> "越南语";
            case TIBETAN -> "藏文";
            case STROKE -> "笔画";
        };
    }

    /** 键面内容总宽：麦克风、间距与短名，供居中计算；纯算术，冒烟可测。 */
    public static float contentWidth(float micPx, float gapPx, float labelPx) {
        if (labelPx <= 0) return micPx;
        if (micPx <= 0) return labelPx;
        return micPx + gapPx + labelPx;
    }

    @Override protected void onDraw(Canvas canvas) {
        if (getWidth() <= 0 || getHeight() <= 0) return;
        float density = getResources().getDisplayMetrics().density;
        float mic = showsMic ? Math.min(MIC_DP * density, getHeight()) : 0f;
        label.setTextSize(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, LABEL_SP,
            getResources().getDisplayMetrics()));
        label.setColor(faceColor);
        float available = getWidth() - mic - GAP_DP * density - 8 * density;
        String text = schemeLabel;
        while (!text.isEmpty() && label.measureText(text) > available && text.length() > 1) {
            text = text.substring(0, text.length() - 1);
        }
        float textWidth = text.isEmpty() ? 0f : label.measureText(text);
        float total = contentWidth(mic, GAP_DP * density, textWidth);
        float left = (getWidth() - total) / 2f;
        float centerY = getHeight() / 2f;
        if (mic > 0) {
            KeyboardIconPaths.draw(canvas, icon, KeyboardIconPaths.Icon.MIC, left,
                centerY - mic / 2f, mic, faceColor);
            left += mic + GAP_DP * density;
        }
        if (!text.isEmpty()) {
            Paint.FontMetrics metrics = label.getFontMetrics();
            canvas.drawText(text, left, centerY - (metrics.ascent + metrics.descent) / 2f, label);
        }
    }
}
