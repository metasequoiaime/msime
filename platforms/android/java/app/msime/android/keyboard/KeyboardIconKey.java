package app.msime.android;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import app.msime.android.KeyboardGeometry;

/**
 * 画描边图标的功能键：⇧（含大写锁定的下划线形）、⌫、↵、123 层的表情键，图标 22 dp，线宽 1.7（viewBox 单位）。
 *
 * <p>节点 text（`⇧`、`⌫`、回车的动作文字）原样保留给无障碍和设备测试，只是不绘制；回车组词时的「确认」、⇧ 在 26 键全拼组字时的「分词」由键面规格设上（{@link #setFace}）。图标颜色就是按钮的文字颜色，由样式通道按皮肤设置（Shift 激活时 accent）。
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
    private String cornerHint = "";

    public KeyboardIconKey(Context context, Kind kind) {
        super(context);
        KeyboardTypographyPolicy.normalizeKeyCap(this);
        this.kind = kind;
        ViewPolicy.setAllCapsFalse(this);
    }

    /** 右上角的提示（回车长按能上屏译文时标「译」，长按换行时标「换行」），由键组按它的右上角位置画（{@link KeyFaceTemplates#RETURN}）；空串或 {@code null} 表示不画。只绘制、不进节点 text。不在键组里的键不画它。 */
    public void setCornerHint(String value) {
        String next = TextPolicy.emptyIfNull(value);
        if (cornerHint.equals(next)) return;
        cornerHint = next;
        if (faceGroup() != null) faceTextChanged(KeyFaceSlot.TOP_RIGHT, next);
    }

    public String cornerHint() { return cornerHint; }

    public Kind kind() { return kind; }

    /** 换图标，例如 Shift 在单次大写与大写锁定之间切换。 */
    public void setKind(Kind value) {
        if (kind == value) return;
        kind = value;
        invalidate();
    }

    /**
     * 按 {@link KeyFaceSpec} 给的键面画（⇧ 的图标或「分词」）：画文字时节点文字也换成这段字，画图标时节点文字（`⇧` 这类读屏和统计用的字）由调用方另设。先换节点文字、再切到画文字，所以切换途中不会把 `⇧` 当成键面的字补进键组。
     */
    public void setFace(KeyFace face) {
        setFace(face, true);
    }

    /** 同上；{@code iconKind} 为假时画图标只切回图标、不换图标种类（回车在剪贴板模式里自己换图标，{@link KeyFaceSpec#ownsIconKind}）。 */
    public void setFace(KeyFace face, boolean iconKind) {
        if (face.text() != null && !face.text().contentEquals(getText())) setText(face.text());
        if (face.icon() != null && iconKind) setKind(face.icon());
        boolean text = face.text() != null;
        if (drawsText != text) {
            drawsText = text;
            if (faceGroup() != null) faceTextChanged(KeyFaceSlot.MAIN, faceText(KeyFaceSlot.MAIN));
            else invalidate();
        }
        setCornerHint(face.corner());
    }

    public boolean drawsText() { return drawsText; }

    /** 这类键画图标（此刻也许画着文字）：键组按它们的键格算图标边长。 */
    @Override public boolean drawsIcons() { return true; }

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
        return drawsText;
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
            return;
        }
        int width = DimensionPolicy.contentWidth(this);
        int height = DimensionPolicy.contentHeight(this);
        float size = BoundsPolicy.atMost(DimensionPolicy.shorterSide(width, height),
            DimensionPolicy.floatPixels(getContext(), ICON_DP));
        drawIcon(canvas, size, width, height, getPaddingLeft(), getPaddingTop());
    }

    /** 在 {@code left}、{@code top} 起的 {@code width}×{@code height} 区域里居中画边长 {@code size} 的图标，停用时变淡。 */
    private void drawIcon(Canvas canvas, float size, int width, int height, int left, int top) {
        if (size <= 0) return;
        int color = getCurrentTextColor();
        if (!isEnabled()) color = ColorPolicy.withAlpha(color, 96f / 255f);
        KeyboardIconPaths.draw(canvas, paint, iconFor(kind), left + (width - size) / 2f, top + (height - size) / 2f,
            size, color);
    }
}
