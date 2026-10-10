package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;

/**
 * Draws a shortcut glyph while retaining the button's text for accessibility.
 *
 * <p>表情、常用语、剪贴板、皮肤、输入方式、浮动键盘按设计画 24 dp 的 Material 实心图标，收起画 21 dp 的描边 chevron（{@link KeyboardIconPaths}）；其余几个沿用原来的描边画法。图标颜色就是按钮的文字颜色，由调用方按皮肤设置（平时 kbSub，打开面板时 accent）。
 */
public final class KeyboardShortcutButton extends KeyboardPressButton {
    private final KeyboardShortcutIconPolicy.Icon icon;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path path = new Path();
    private final RectF bounds = new RectF();
    private final Paint fill = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint glyph = new Paint(Paint.ANTI_ALIAS_FLAG);
    private int activeFill = Color.TRANSPARENT;
    private boolean iconColorsSet;
    private int idleIconColor;
    private int activeIconColor;
    private boolean flipped;

    public KeyboardShortcutButton(Context context, KeyboardShortcutIconPolicy.Icon icon) {
        super(context);
        this.icon = icon;
        setKeyboardRole(KeyboardKeyRole.GLYPH);
        ViewPolicy.setCentered(this);
        ViewPolicy.clearPadding(this);
        paint.setStyle(Paint.Style.STROKE);
        paint.setStrokeCap(Paint.Cap.ROUND);
        paint.setStrokeJoin(Paint.Join.ROUND);
    }

    /**
     * The glyph's share of its touch target.
     *
     * <p>These marks used to sit inside a filled box, where filling the button was what made them
     * legible. The toolbar is flat now, so the glyph is the whole control and takes the smaller size
     * the shared design draws it at; the 44dp hit target is unchanged.
     */
    private static final float GLYPH_SCALE = 0.62f;

    /** 选中时垫在图标后面的底块占触控区短边的比例。 */
    private static final float ACTIVE_SCALE = 0.86f;

    /** 设计的工具栏按钮：40 dp 见方、圆角 12 dp 的底块，Material 图标 24 dp，收起 chevron 21 dp。 */
    private static final float ACTIVE_SIDE_DP = 40f;
    private static final float ACTIVE_RADIUS_DP = 12f;
    private static final float MATERIAL_ICON_DP = 24f;
    private static final float DISMISS_ICON_DP = 21f;
    private static final float[] SETTINGS_LINES = {25f, 50f, 75f};

    /** 选中状态的底色：工具栏按钮打开了它的面板时，图标后面垫这块柔和的强调色，而不是把整个按钮铺成实心色块。 */
    public void setActiveFill(int color) {
        if (activeFill == color) return;
        activeFill = color;
        invalidate();
    }

    /** 新设计的图标色：平时 kbSub，选中（面板打开）时 accent。设置后不再跟随文字色，皮肤重新套样式时也不会被改回键面文字色。 */
    public void setIconColors(int idle, int active) {
        if (iconColorsSet && idleIconColor == idle && activeIconColor == active) return;
        iconColorsSet = true;
        idleIconColor = idle;
        activeIconColor = active;
        invalidate();
    }

    /** 图标上下翻转：外接键盘的候选条模式里，收起键的 chevron 朝上，表示展开软键盘。 */
    public void setFlipped(boolean value) {
        if (flipped == value) return;
        flipped = value;
        invalidate();
    }

    private int iconColor() {
        if (!iconColorsSet) return getCurrentTextColor();
        return isSelected() ? activeIconColor : idleIconColor;
    }

    @Override protected void onDraw(Canvas canvas) {
        int width = DimensionPolicy.contentWidth(this);
        int height = DimensionPolicy.contentHeight(this);
        if (KeyboardShortcutIconPolicy.materialGlyph(icon)) {
            drawMaterial(canvas, width, height);
            return;
        }
        float size = DimensionPolicy.shorterSide(width, height) * GLYPH_SCALE;
        if (size <= 0) return;
        if (isSelected() && Color.alpha(activeFill) > 0) {
        float side = DimensionPolicy.shorterSide(width, height) * ACTIVE_SCALE;
            float left = getPaddingLeft() + (width - side) / 2f;
            float top = getPaddingTop() + (height - side) / 2f;
            bounds.set(left, top, left + side, top + side);
            fill.setColor(activeFill);
            canvas.drawRoundRect(bounds, side * 0.25f, side * 0.25f, fill);
        }
        int color = iconColor();
        if (color == Color.TRANSPARENT) color = Color.WHITE;
        paint.setColor(color);
        paint.setAlpha(ColorPolicy.enabledAlpha(isEnabled(), 255, 96));
        paint.setStrokeWidth(7f);
        canvas.save();
        canvas.translate(getPaddingLeft() + (width - size) / 2f,
            getPaddingTop() + (height - size) / 2f);
        canvas.scale(size / 100f, size / 100f);
        switch (icon) {
            case SETTINGS -> drawSettings(canvas);
            case REPLY -> drawReply(canvas);
            case EMOJI -> drawEmoji(canvas);
            case VOICE -> drawVoice(canvas);
            case SKIN -> drawSkin(canvas);
            case DISMISS -> drawDismiss(canvas);
            case GLOBE -> drawGlobe(canvas);
            case BOOKMARK -> drawBookmark(canvas);
            case PHRASE, CLIPBOARD, SCHEME, FLOATING, TEXT_EDIT -> { }
        }
        canvas.restore();
    }

    private void drawMaterial(Canvas canvas, int width, int height) {
        float density = getResources().getDisplayMetrics().density;
        float shorter = DimensionPolicy.shorterSide(width, height);
        if (shorter <= 0) return;
        float centerX = getPaddingLeft() + width / 2f;
        float centerY = getPaddingTop() + height / 2f;
        if (isSelected() && Color.alpha(activeFill) > 0) {
            float side = BoundsPolicy.atMost(shorter,
                DimensionPolicy.floatPixels(getContext(), ACTIVE_SIDE_DP));
            bounds.set(centerX - side / 2f, centerY - side / 2f, centerX + side / 2f,
                centerY + side / 2f);
            fill.setColor(activeFill);
            float radius = BoundsPolicy.atMost(side / 2f,
                DimensionPolicy.floatPixels(getContext(), ACTIVE_RADIUS_DP));
            canvas.drawRoundRect(bounds, radius, radius, fill);
        }
        int color = iconColor();
        if (color == Color.TRANSPARENT) color = Color.WHITE;
        if (!isEnabled()) color = ColorPolicy.withAlpha(color, 96f / 255f);
        KeyboardIconPaths.Icon path = switch (icon) {
            case EMOJI -> KeyboardIconPaths.Icon.TOOLBAR_EMOJI;
            case PHRASE -> KeyboardIconPaths.Icon.TOOLBAR_PHRASE;
            case CLIPBOARD -> KeyboardIconPaths.Icon.TOOLBAR_CLIPBOARD;
            case SKIN -> KeyboardIconPaths.Icon.TOOLBAR_SKIN;
            case SCHEME -> KeyboardIconPaths.Icon.TOOLBAR_SCHEME;
            case FLOATING -> KeyboardIconPaths.Icon.TOOLBAR_FLOATING;
            // 和功能面板「文本编辑」磁贴同一个图标，用户在两处认得出是同一个面板。
            case TEXT_EDIT -> KeyboardIconPaths.Icon.TEXT_EDIT;
            case DISMISS, SETTINGS, REPLY, VOICE, GLOBE, BOOKMARK ->
                KeyboardIconPaths.Icon.TOOLBAR_DISMISS;
        };
        float iconDp = icon == KeyboardShortcutIconPolicy.Icon.DISMISS
            ? DISMISS_ICON_DP : MATERIAL_ICON_DP;
        float size = BoundsPolicy.atMost(shorter,
            DimensionPolicy.floatPixels(getContext(), iconDp));
        int saved = canvas.save();
        if (flipped) canvas.rotate(180f, centerX, centerY);
        KeyboardIconPaths.draw(canvas, glyph, path, centerX - size / 2f, centerY - size / 2f,
            size, color);
        canvas.restoreToCount(saved);
    }

    private void drawSettings(Canvas canvas) {
        for (float y : SETTINGS_LINES) canvas.drawLine(15f, y, 85f, y, paint);
        canvas.drawCircle(38f, 25f, 7f, paint);
        canvas.drawCircle(68f, 50f, 7f, paint);
        canvas.drawCircle(30f, 75f, 7f, paint);
    }

    private void drawReply(Canvas canvas) {
        bounds.set(12f, 19f, 63f, 57f);
        canvas.drawRoundRect(bounds, 10f, 10f, paint);
        path.reset();
        path.moveTo(25f, 57f);
        path.lineTo(20f, 69f);
        path.lineTo(36f, 57f);
        canvas.drawPath(path, paint);
        bounds.set(38f, 39f, 88f, 77f);
        canvas.drawRoundRect(bounds, 10f, 10f, paint);
        path.reset();
        path.moveTo(75f, 77f);
        path.lineTo(80f, 88f);
        path.lineTo(64f, 77f);
        canvas.drawPath(path, paint);
    }

    private void drawEmoji(Canvas canvas) {
        canvas.drawCircle(50f, 50f, 34f, paint);
        paint.setStyle(Paint.Style.FILL);
        canvas.drawCircle(38f, 43f, 4f, paint);
        canvas.drawCircle(62f, 43f, 4f, paint);
        paint.setStyle(Paint.Style.STROKE);
        bounds.set(32f, 37f, 68f, 68f);
        canvas.drawArc(bounds, 25f, 130f, false, paint);
    }

    private void drawVoice(Canvas canvas) {
        bounds.set(35f, 14f, 65f, 62f);
        canvas.drawRoundRect(bounds, 15f, 15f, paint);
        bounds.set(22f, 34f, 78f, 82f);
        canvas.drawArc(bounds, 0f, 180f, false, paint);
        canvas.drawLine(50f, 82f, 50f, 91f, paint);
        canvas.drawLine(36f, 91f, 64f, 91f, paint);
    }

    private void drawSkin(Canvas canvas) {
        path.reset();
        path.moveTo(36f, 18f);
        path.lineTo(19f, 31f);
        path.lineTo(30f, 46f);
        path.lineTo(39f, 39f);
        path.lineTo(39f, 82f);
        path.lineTo(61f, 82f);
        path.lineTo(61f, 39f);
        path.lineTo(70f, 46f);
        path.lineTo(81f, 31f);
        path.lineTo(64f, 18f);
        path.lineTo(57f, 31f);
        path.lineTo(43f, 31f);
        path.close();
        canvas.drawPath(path, paint);
    }

    /** The input-method switch: a meridian and two parallels, as every platform draws it. */
    private void drawGlobe(Canvas canvas) {
        canvas.drawCircle(50f, 50f, 34f, paint);
        bounds.set(28f, 16f, 72f, 84f);
        canvas.drawOval(bounds, paint);
        canvas.drawLine(18f, 36f, 82f, 36f, paint);
        canvas.drawLine(18f, 64f, 82f, 64f, paint);
    }

    /** 回复面板的模板入口：一枚书签，对应 iOS 的 `bookmark` 符号。 */
    private void drawBookmark(Canvas canvas) {
        path.reset();
        path.moveTo(28f, 14f);
        path.lineTo(72f, 14f);
        path.lineTo(72f, 86f);
        path.lineTo(50f, 68f);
        path.lineTo(28f, 86f);
        path.close();
        canvas.drawPath(path, paint);
    }

    private void drawDismiss(Canvas canvas) {
        path.reset();
        path.moveTo(24f, 37f);
        path.lineTo(50f, 64f);
        path.lineTo(76f, 37f);
        canvas.drawPath(path, paint);
    }
}
