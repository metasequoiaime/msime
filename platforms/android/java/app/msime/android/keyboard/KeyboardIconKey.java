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
 */
public final class KeyboardIconKey extends KeyboardPressButton {
    /** 键上画的图标。 */
    public enum Kind { SHIFT, CAPS_LOCK, BACKSPACE, RETURN, EMOJI, CURSOR_LEFT, TOGGLE_NEXT }

    public static final float ICON_DP = 22f;

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private Kind kind;
    private boolean drawsText;
    private java.util.Set<String> textFaces = java.util.Set.of();

    public KeyboardIconKey(Context context, Kind kind) {
        super(context);
        KeyboardGeometry.normalizeKeyCap(this);
        this.kind = kind;
        setAllCaps(false);
    }

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
        invalidate();
    }

    public boolean drawsText() { return drawsText; }

    /** 节点 text 落在这组文字里时按文字画（回车组词时的「确认」「確定」），其余时候画图标；调用方只管 setText，不必再切 {@link #setDrawsText}。 */
    public void setTextFaces(java.util.Set<String> faces) {
        textFaces = faces == null ? java.util.Set.of() : java.util.Set.copyOf(faces);
        invalidate();
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
        };
    }

    @Override protected void onDraw(Canvas canvas) {
        if (showsText()) {
            super.onDraw(canvas);
            return;
        }
        int width = Math.max(0, getWidth() - getPaddingLeft() - getPaddingRight());
        int height = Math.max(0, getHeight() - getPaddingTop() - getPaddingBottom());
        float size = Math.min(Math.min(width, height),
            KeyboardGeometry.floatPixels(getContext(), ICON_DP));
        if (size <= 0) return;
        int color = getCurrentTextColor();
        if (!isEnabled()) color = ColorPolicy.withAlpha(color, 96f / 255f);
        KeyboardIconPaths.draw(canvas, paint, iconFor(kind),
            getPaddingLeft() + (width - size) / 2f, getPaddingTop() + (height - size) / 2f,
            size, color);
    }
}
