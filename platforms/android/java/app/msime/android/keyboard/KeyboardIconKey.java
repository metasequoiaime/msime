package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import app.msime.android.KeyboardGeometry;

/**
 * 画描边图标的功能键：⇧（含大写锁定的下划线形）、⌫、↵、123 层的表情键，图标 22 dp，线宽 1.7（viewBox 单位）。
 *
 * <p>节点 text（`⇧`、`⌫`、回车的动作文字）原样保留给无障碍和设备测试，只是不绘制；回车在组词时要显示「确认」，调用方用 {@link #setDrawsText} 切回文字。图标颜色就是按钮的文字颜色，由样式通道按皮肤设置（Shift 激活时 accent）。
 *
 * <p>放进键组（{@link #setFaceGroup}）时，图标边长取键组算的 {@link KeyFaceParams#iconPx}，改画文字时文字和右上角提示都按键组的参数画；画图标时主标签位置是空的，`⇧`、`⌫` 这样的节点文字不进键组的内容域。
 */
public final class KeyboardIconKey extends KeyboardPressButton {
    /** 键上画的图标。 */
    public enum Kind { SHIFT, CAPS_LOCK, BACKSPACE, RETURN, EMOJI, CURSOR_LEFT, TOGGLE_NEXT, TRASH, INFO }

    public static final float ICON_DP = 22f;

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private Kind kind;
    private boolean drawsText;
    private java.util.Set<String> textFaces = java.util.Set.of();
    private final Paint cornerPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private String cornerHint = "";
    /** 键帽轮廓：不在键组里的键（回车）自己画角标时按它挪进键帽（{@link KeyOutline}）。 */
    private KeyOutline outline = KeyOutline.NONE;
    private final android.graphics.Rect cornerBounds = new android.graphics.Rect();

    public KeyboardIconKey(Context context, Kind kind) {
        super(context);
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        this.kind = kind;
        ViewPolicy.setAllCapsFalse(this);
        cornerPaint.setTextAlign(Paint.Align.RIGHT);
    }

    /** 右上角的提示（组字中的「确认」长按能上屏译文时标「译」），字号和位置与字母键的角标相同（{@link KeyHintButton}），颜色取键面文字色；空串或 {@code null} 表示不画。只绘制、不进节点 text。 */
    public void setCornerHint(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (cornerHint.equals(next)) return;
        cornerHint = next;
        if (faceGroup() != null) faceTextChanged(KeyFaceSlot.TOP_RIGHT, next);
        else invalidate();
    }

    public String cornerHint() { return cornerHint; }

    public Kind kind() { return kind; }

    /** 换图标，例如 Shift 在单次大写与大写锁定之间切换。 */
    public void setKind(Kind value) {
        if (kind == value) return;
        kind = value;
        invalidate();
    }

    /** 为真时按普通按钮画文字（回车组词时的「确认」），为假时画图标。 */
    public void setDrawsText(boolean value) {
        if (drawsText == value) return;
        drawsText = value;
        mainFaceChanged();
    }

    public boolean drawsText() { return drawsText; }

    /** 节点 text 落在这组文字里时按文字画（回车组词时的「确认」「確定」），其余时候画图标；调用方只管 setText，不必再切 {@link #setDrawsText}。 */
    public void setTextFaces(java.util.Set<String> faces) {
        textFaces = SetPolicy.copyOrEmpty(faces);
        mainFaceChanged();
    }

    /** 在图标和文字之间换了：只重新定位、重画。文字由随后的 setText 补进键组（⇧ 先切到文字、再换成「分词」，这时补进去的会是 `⇧`）。 */
    private void mainFaceChanged() {
        faceRedraw();
    }

    /** 画图标时主标签位置是空的；右上角是 {@link #setCornerHint} 的提示。 */
    @Override public String faceText(KeyFaceSlot slot) {
        return switch (slot) {
            case MAIN -> showsText() ? String.valueOf(getText()) : "";
            case TOP_RIGHT -> cornerHint;
            default -> "";
        };
    }

    /** 现在是否按文字画。 */
    public boolean showsText() {
        CharSequence text = getText();
        return drawsText || (text != null && textFaces.contains(text.toString()));
    }

    /** {@link Kind} 对应的生成图标。 */
    public static KeyboardIconPaths.Icon iconFor(Kind kind) {
        return switch (kind) {
            case SHIFT -> KeyboardIconPaths.Icon.SHIFT;
            case CAPS_LOCK -> KeyboardIconPaths.Icon.CAPS_LOCK;
            case BACKSPACE -> KeyboardIconPaths.Icon.BACKSPACE;
            case RETURN -> KeyboardIconPaths.Icon.RETURN;
            case EMOJI -> KeyboardIconPaths.Icon.KEY_EMOJI;
            case CURSOR_LEFT -> KeyboardIconPaths.Icon.CURSOR_LEFT;
            case TOGGLE_NEXT -> KeyboardIconPaths.Icon.TOGGLE_NEXT;
            case TRASH -> KeyboardIconPaths.Icon.TRASH;
            case INFO -> KeyboardIconPaths.Icon.INFO;
        };
    }

    @Override protected void onDraw(Canvas canvas) {
        KeyFaceParams params = faceParams();
        if (faceGroup() != null) {
            // 键组画文字和提示；这里只补上图标。
            if (!showsText() && params != null) {
                int saved = beginFaceDraw(canvas);
                drawIcon(canvas, params.iconPx(), getWidth(), getHeight(), 0, 0);
                canvas.restoreToCount(saved);
            }
            super.onDraw(canvas);
            return;
        }
        if (showsText()) {
            super.onDraw(canvas);
            drawCornerHint(canvas);
            return;
        }
        int width = DimensionPolicy.contentWidth(this);
        int height = DimensionPolicy.contentHeight(this);
        float size = BoundsPolicy.atMost(DimensionPolicy.shorterSide(width, height),
            DimensionPolicy.floatPixels(getContext(), ICON_DP));
        drawIcon(canvas, size, width, height, getPaddingLeft(), getPaddingTop());
        drawCornerHint(canvas);
    }

    /** 在 {@code left}、{@code top} 起的 {@code width}×{@code height} 区域里居中画边长 {@code size} 的图标，停用时变淡。 */
    private void drawIcon(Canvas canvas, float size, int width, int height, int left, int top) {
        if (size <= 0) return;
        int color = getCurrentTextColor();
        if (!isEnabled()) color = ColorPolicy.withAlpha(color, 96f / 255f);
        KeyboardIconPaths.draw(canvas, paint, iconFor(kind), left + (width - size) / 2f, top + (height - size) / 2f,
            size, color);
    }

    /** 皮肤的键帽轮廓（{@code ImeStyler} 配键帽时设）；变了才重画。 */
    public void setKeyOutline(KeyOutline value) {
        KeyOutline next = value == null ? KeyOutline.NONE : value;
        if (next.equals(outline)) return;
        outline = next;
        invalidate();
    }

    /** 角标按原来的文字框位置画（右端离右边 5 dp、上行线离上边 3 dp）；键帽轮廓在那里收进去时沿对角线挪进来，挪不进来就不画。 */
    private void drawCornerHint(Canvas canvas) {
        if (cornerHint.isEmpty() || getWidth() <= 0 || getHeight() <= 0) return;
        float scale = CompactKeyTypography.scale(getContext(), getHeight());
        ViewPolicy.setTextSize(cornerPaint, KeyboardTypographyPolicy.keySp(getContext(), KeyHintButton.CORNER_HINT_SP) * scale);
        cornerPaint.setColor(getCurrentTextColor());
        cornerPaint.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 204, 96));
        Paint.FontMetrics metrics = cornerPaint.getFontMetrics();
        float baseline = DimensionPolicy.floatPixels(getContext(), KeyHintButton.CORNER_HINT_TOP_DP) * scale - metrics.ascent;
        float right = getWidth() - DimensionPolicy.floatPixels(getContext(), KeyHintButton.CORNER_HINT_RIGHT_DP) * scale;
        if (!outline.none()) {
            cornerPaint.getTextBounds(cornerHint, 0, cornerHint.length(), cornerBounds);
            float origin = right - cornerPaint.measureText(cornerHint);
            float offset = outline.spans(getWidth(), getHeight()).cornerOffset(origin + cornerBounds.right,
                baseline + cornerBounds.top, cornerBounds.width(), cornerBounds.height());
            if (Float.isNaN(offset)) return;
            right -= offset;
            baseline += offset;
        }
        canvas.drawText(cornerHint, right, baseline, cornerPaint);
    }
}
