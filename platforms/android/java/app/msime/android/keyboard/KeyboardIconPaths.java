// 由 platforms/android/scripts/generate_keyboard_icons.py 生成，不要手工编辑；改图标请改脚本里的 ICONS 表再重新运行。
// 来源：设计令牌 §6.2（工具栏 Material Icons）、§6.3（按键描边图标）、§6.4（功能面板描边图标），viewBox 0 0 24 24；设计未给图标的几项是同网格的 Lucide 风格补充。弧线已在脚本里转成三次贝塞尔。
package app.msime.android;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;

/** 键盘里用到的全部矢量图标：IME 进程不能用 `R`，图标以 `Path` 构造代码的形式随代码发布。 */
public final class KeyboardIconPaths {
    /** 图标坐标系的边长（SVG viewBox 0 0 24 24）。 */
    public static final float VIEWBOX = 24f;

    /** 一个图标：实心填充，或按给定线宽描边（线宽以 viewBox 单位计，圆头圆角）。 */
    public enum Icon {
        /** Material Icons sentiment_satisfied (outlined) */
        TOOLBAR_EMOJI(false, 0f),
        /** Material Icons chat (outlined) */
        TOOLBAR_PHRASE(false, 0f),
        /** Material Icons content_paste */
        TOOLBAR_CLIPBOARD(false, 0f),
        /** Material Icons palette (outlined) */
        TOOLBAR_SKIN(false, 0f),
        /** Material Icons keyboard (outlined) */
        TOOLBAR_SCHEME(false, 0f),
        /** Material Icons picture_in_picture_alt */
        TOOLBAR_FLOATING(false, 0f),
        /** design chevron-down (21dp) */
        TOOLBAR_DISMISS(true, 1.8f),
        /** design key shift */
        SHIFT(true, 1.7f),
        /** design key caps lock */
        CAPS_LOCK(true, 1.7f),
        /** design key backspace */
        BACKSPACE(true, 1.7f),
        /** design key return */
        RETURN(true, 1.7f),
        /** design key emoji (123 layer) */
        KEY_EMOJI(true, 1.7f),
        /** design space mic */
        MIC(true, 1.7f),
        /** design candidate expand chevron (20dp) */
        CHEVRON(true, 1.8f),
        /** design panel 手写 */
        HANDWRITING(true, 1.7f),
        /** design panel 词库 */
        LEXICON(true, 1.7f),
        /** design panel 键盘高度 */
        KEYBOARD_HEIGHT(true, 1.7f),
        /** design panel 设置 */
        SETTINGS(true, 1.7f),
        /** design panel 按键音 */
        KEY_SOUND(true, 1.7f),
        /** design panel 振动 */
        VIBRATION(true, 1.7f),
        /** design panel 单手模式 */
        ONE_HAND(true, 1.7f),
        /** design panel 隐私模式 */
        INCOGNITO(true, 1.7f),
        /** design panel 反馈 */
        FEEDBACK(true, 1.7f),
        /** design panel 关于 */
        ABOUT(true, 1.7f),
        /** design on-state check badge */
        CHECK(true, 4f),
        /** Lucide-style sparkles (AI 回复与润色) */
        AI_ASSIST(true, 1.7f),
        /** Lucide-style laptop (本地输入) */
        LOCAL_INPUT(true, 1.7f),
        /** Lucide-style mic (语音结果) */
        VOICE_RESULT(true, 1.7f),
        /** Lucide-style activity (振动强度) */
        VIBRATION_STRENGTH(true, 1.7f),
        /** Lucide-style clipboard-list (剪贴板历史) */
        CLIPBOARD_HISTORY(true, 1.7f),
        /** Lucide-style chevron-left (单手换边) */
        SWAP_SIDE(true, 1.8f),
        /** Lucide-style arrow-left (日语九键 光标左移) */
        CURSOR_LEFT(true, 1.7f),
        /** Lucide-style arrow-right (日语九键 结束连点) */
        TOGGLE_NEXT(true, 1.7f),
        /** Lucide-style maximize-2 (退出单手) */
        EXIT_ONE_HAND(true, 1.7f),
        /** Lucide-style picture-in-picture-2 (浮动键盘) */
        FLOATING(true, 1.7f),
        /** Lucide-style text-cursor-input (文本编辑) */
        TEXT_EDIT(true, 1.7f),
        /** Lucide-style trash-2 (剪贴板左滑删除) */
        TRASH(true, 1.7f);

        private final boolean stroked;
        private final float strokeWidth;

        Icon(boolean stroked, float strokeWidth) {
            this.stroked = stroked;
            this.strokeWidth = strokeWidth;
        }

        /** 为真时按 {@link #strokeWidth()} 描边，否则实心填充。 */
        public boolean stroked() { return stroked; }

        /** 描边线宽，viewBox 单位；实心图标为 0。 */
        public float strokeWidth() { return strokeWidth; }
    }

    private static final Path[] CACHE = new Path[Icon.values().length];

    private KeyboardIconPaths() {}

    /** 图标在 viewBox 坐标里的路径；结果被缓存，调用方不得修改它。 */
    public static Path path(Icon icon) {
        Path cached = CACHE[icon.ordinal()];
        if (cached != null) return cached;
        Path built = new Path();
        append(icon, built);
        CACHE[icon.ordinal()] = built;
        return built;
    }

    /**
     * 在 {@code (left, top)} 起、边长 {@code size} 的方框里用 {@code color} 画出图标。
     *
     * <p>{@code paint} 的样式、线宽、端点与颜色会被改写；调用方为每个视图保留一支专用的 Paint。
     */
    public static void draw(Canvas canvas, Paint paint, Icon icon, float left, float top,
            float size, int color) {
        if (size <= 0) return;
        paint.setColor(color);
        if (icon.stroked()) {
            paint.setStyle(Paint.Style.STROKE);
            paint.setStrokeWidth(icon.strokeWidth());
            paint.setStrokeCap(Paint.Cap.ROUND);
            paint.setStrokeJoin(Paint.Join.ROUND);
        } else {
            paint.setStyle(Paint.Style.FILL);
        }
        int saved = canvas.save();
        canvas.translate(left, top);
        float scale = size / VIEWBOX;
        canvas.scale(scale, scale);
        canvas.drawPath(path(icon), paint);
        canvas.restoreToCount(saved);
    }

    /** 把图标的路径追加到 {@code p}。 */
    public static void append(Icon icon, Path p) {
        switch (icon) {
            case TOOLBAR_EMOJI -> appendToolbarEmoji(p);
            case TOOLBAR_PHRASE -> appendToolbarPhrase(p);
            case TOOLBAR_CLIPBOARD -> appendToolbarClipboard(p);
            case TOOLBAR_SKIN -> appendToolbarSkin(p);
            case TOOLBAR_SCHEME -> appendToolbarScheme(p);
            case TOOLBAR_FLOATING -> appendToolbarFloating(p);
            case TOOLBAR_DISMISS -> appendToolbarDismiss(p);
            case SHIFT -> appendShift(p);
            case CAPS_LOCK -> appendCapsLock(p);
            case BACKSPACE -> appendBackspace(p);
            case RETURN -> appendReturn(p);
            case KEY_EMOJI -> appendKeyEmoji(p);
            case MIC -> appendMic(p);
            case CHEVRON -> appendChevron(p);
            case HANDWRITING -> appendHandwriting(p);
            case LEXICON -> appendLexicon(p);
            case KEYBOARD_HEIGHT -> appendKeyboardHeight(p);
            case SETTINGS -> appendSettings(p);
            case KEY_SOUND -> appendKeySound(p);
            case VIBRATION -> appendVibration(p);
            case ONE_HAND -> appendOneHand(p);
            case INCOGNITO -> appendIncognito(p);
            case FEEDBACK -> appendFeedback(p);
            case ABOUT -> appendAbout(p);
            case CHECK -> appendCheck(p);
            case AI_ASSIST -> appendAiAssist(p);
            case LOCAL_INPUT -> appendLocalInput(p);
            case VOICE_RESULT -> appendVoiceResult(p);
            case VIBRATION_STRENGTH -> appendVibrationStrength(p);
            case CLIPBOARD_HISTORY -> appendClipboardHistory(p);
            case SWAP_SIDE -> appendSwapSide(p);
            case CURSOR_LEFT -> appendCursorLeft(p);
            case TOGGLE_NEXT -> appendToggleNext(p);
            case EXIT_ONE_HAND -> appendExitOneHand(p);
            case FLOATING -> appendFloating(p);
            case TEXT_EDIT -> appendTextEdit(p);
            case TRASH -> appendTrash(p);
        }
    }

    private static void appendToolbarEmoji(Path p) {
        p.moveTo(11.99f, 2f);
        p.cubicTo(6.47f, 2f, 2f, 6.48f, 2f, 12f);
        p.cubicTo(2f, 17.52f, 6.47f, 22f, 11.99f, 22f);
        p.cubicTo(17.52f, 22f, 22f, 17.52f, 22f, 12f);
        p.cubicTo(22f, 6.48f, 17.52f, 2f, 11.99f, 2f);
        p.close();
        p.moveTo(12f, 20f);
        p.cubicTo(7.58f, 20f, 4f, 16.42f, 4f, 12f);
        p.cubicTo(4f, 7.58f, 7.58f, 4f, 12f, 4f);
        p.cubicTo(16.42f, 4f, 20f, 7.58f, 20f, 12f);
        p.cubicTo(20f, 16.42f, 16.42f, 20f, 12f, 20f);
        p.close();
        p.moveTo(15.5f, 11f);
        p.cubicTo(16.33f, 11f, 17f, 10.33f, 17f, 9.5f);
        p.cubicTo(17f, 8.67f, 16.33f, 8f, 15.5f, 8f);
        p.cubicTo(14.67f, 8f, 14f, 8.67f, 14f, 9.5f);
        p.cubicTo(14f, 10.33f, 14.67f, 11f, 15.5f, 11f);
        p.close();
        p.moveTo(8.5f, 11f);
        p.cubicTo(9.33f, 11f, 10f, 10.33f, 10f, 9.5f);
        p.cubicTo(10f, 8.67f, 9.33f, 8f, 8.5f, 8f);
        p.cubicTo(7.67f, 8f, 7f, 8.67f, 7f, 9.5f);
        p.cubicTo(7f, 10.33f, 7.67f, 11f, 8.5f, 11f);
        p.close();
        p.moveTo(12f, 17.5f);
        p.cubicTo(14.33f, 17.5f, 16.31f, 16.04f, 17.11f, 14f);
        p.lineTo(6.89f, 14f);
        p.cubicTo(7.69f, 16.04f, 9.67f, 17.5f, 12f, 17.5f);
        p.close();
    }

    private static void appendToolbarPhrase(Path p) {
        p.moveTo(20f, 2f);
        p.lineTo(4f, 2f);
        p.cubicTo(2.9f, 2f, 2.01f, 2.9f, 2.01f, 4f);
        p.lineTo(2f, 22f);
        p.lineTo(6f, 18f);
        p.lineTo(20f, 18f);
        p.cubicTo(21.1f, 18f, 22f, 17.1f, 22f, 16f);
        p.lineTo(22f, 4f);
        p.cubicTo(22f, 2.9f, 21.1f, 2f, 20f, 2f);
        p.close();
        p.moveTo(20f, 16f);
        p.lineTo(5.17f, 16f);
        p.lineTo(4f, 17.17f);
        p.lineTo(4f, 4f);
        p.lineTo(20f, 4f);
        p.lineTo(20f, 16f);
        p.close();
        p.moveTo(6f, 9f);
        p.lineTo(18f, 9f);
        p.lineTo(18f, 11f);
        p.lineTo(6f, 11f);
        p.close();
        p.moveTo(6f, 6f);
        p.lineTo(18f, 6f);
        p.lineTo(18f, 8f);
        p.lineTo(6f, 8f);
        p.close();
        p.moveTo(6f, 12f);
        p.lineTo(14f, 12f);
        p.lineTo(14f, 14f);
        p.lineTo(6f, 14f);
        p.close();
    }

    private static void appendToolbarClipboard(Path p) {
        p.moveTo(19f, 2f);
        p.lineTo(14.82f, 2f);
        p.cubicTo(14.4f, 0.84f, 13.3f, 0f, 12f, 0f);
        p.cubicTo(10.7f, 0f, 9.6f, 0.84f, 9.18f, 2f);
        p.lineTo(5f, 2f);
        p.cubicTo(3.9f, 2f, 3f, 2.9f, 3f, 4f);
        p.lineTo(3f, 20f);
        p.cubicTo(3f, 21.1f, 3.9f, 22f, 5f, 22f);
        p.lineTo(19f, 22f);
        p.cubicTo(20.1f, 22f, 21f, 21.1f, 21f, 20f);
        p.lineTo(21f, 4f);
        p.cubicTo(21f, 2.9f, 20.1f, 2f, 19f, 2f);
        p.close();
        p.moveTo(12f, 2f);
        p.cubicTo(12.55f, 2f, 13f, 2.45f, 13f, 3f);
        p.cubicTo(13f, 3.55f, 12.55f, 4f, 12f, 4f);
        p.cubicTo(11.45f, 4f, 11f, 3.55f, 11f, 3f);
        p.cubicTo(11f, 2.45f, 11.45f, 2f, 12f, 2f);
        p.close();
        p.moveTo(19f, 20f);
        p.lineTo(5f, 20f);
        p.lineTo(5f, 4f);
        p.lineTo(7f, 4f);
        p.lineTo(7f, 7f);
        p.lineTo(17f, 7f);
        p.lineTo(17f, 4f);
        p.lineTo(19f, 4f);
        p.lineTo(19f, 20f);
        p.close();
    }

    private static void appendToolbarSkin(Path p) {
        p.moveTo(12f, 22f);
        p.cubicTo(6.49f, 22f, 2f, 17.51f, 2f, 12f);
        p.cubicTo(2f, 6.49f, 6.49f, 2f, 12f, 2f);
        p.cubicTo(17.51f, 2f, 22f, 6.04f, 22f, 11f);
        p.cubicTo(22f, 14.31f, 19.31f, 17f, 16f, 17f);
        p.lineTo(14.23f, 17f);
        p.cubicTo(13.95f, 17f, 13.73f, 17.22f, 13.73f, 17.5f);
        p.cubicTo(13.73f, 17.62f, 13.78f, 17.73f, 13.86f, 17.83f);
        p.cubicTo(14.27f, 18.3f, 14.5f, 18.89f, 14.5f, 19.5f);
        p.cubicTo(14.5f, 20.88f, 13.38f, 22f, 12f, 22f);
        p.close();
        p.moveTo(12f, 4f);
        p.cubicTo(7.59f, 4f, 4f, 7.59f, 4f, 12f);
        p.cubicTo(4f, 16.41f, 7.59f, 20f, 12f, 20f);
        p.cubicTo(12.28f, 20f, 12.5f, 19.78f, 12.5f, 19.5f);
        p.cubicTo(12.5f, 19.34f, 12.42f, 19.22f, 12.36f, 19.15f);
        p.cubicTo(11.95f, 18.69f, 11.73f, 18.1f, 11.73f, 17.5f);
        p.cubicTo(11.73f, 16.12f, 12.85f, 15f, 14.23f, 15f);
        p.lineTo(16f, 15f);
        p.cubicTo(18.21f, 15f, 20f, 13.21f, 20f, 11f);
        p.cubicTo(20f, 7.14f, 16.41f, 4f, 12f, 4f);
        p.close();
        p.moveTo(6.5f, 13f);
        p.cubicTo(7.3284f, 13f, 8f, 12.3284f, 8f, 11.5f);
        p.cubicTo(8f, 10.6716f, 7.3284f, 10f, 6.5f, 10f);
        p.cubicTo(5.6716f, 10f, 5f, 10.6716f, 5f, 11.5f);
        p.cubicTo(5f, 12.3284f, 5.6716f, 13f, 6.5f, 13f);
        p.close();
        p.moveTo(9.5f, 9f);
        p.cubicTo(10.3284f, 9f, 11f, 8.3284f, 11f, 7.5f);
        p.cubicTo(11f, 6.6716f, 10.3284f, 6f, 9.5f, 6f);
        p.cubicTo(8.6716f, 6f, 8f, 6.6716f, 8f, 7.5f);
        p.cubicTo(8f, 8.3284f, 8.6716f, 9f, 9.5f, 9f);
        p.close();
        p.moveTo(14.5f, 9f);
        p.cubicTo(15.3284f, 9f, 16f, 8.3284f, 16f, 7.5f);
        p.cubicTo(16f, 6.6716f, 15.3284f, 6f, 14.5f, 6f);
        p.cubicTo(13.6716f, 6f, 13f, 6.6716f, 13f, 7.5f);
        p.cubicTo(13f, 8.3284f, 13.6716f, 9f, 14.5f, 9f);
        p.close();
        p.moveTo(17.5f, 13f);
        p.cubicTo(18.3284f, 13f, 19f, 12.3284f, 19f, 11.5f);
        p.cubicTo(19f, 10.6716f, 18.3284f, 10f, 17.5f, 10f);
        p.cubicTo(16.6716f, 10f, 16f, 10.6716f, 16f, 11.5f);
        p.cubicTo(16f, 12.3284f, 16.6716f, 13f, 17.5f, 13f);
        p.close();
    }

    private static void appendToolbarScheme(Path p) {
        p.moveTo(20f, 5f);
        p.lineTo(4f, 5f);
        p.cubicTo(2.9f, 5f, 2.01f, 5.9f, 2.01f, 7f);
        p.lineTo(2f, 17f);
        p.cubicTo(2f, 18.1f, 2.9f, 19f, 4f, 19f);
        p.lineTo(20f, 19f);
        p.cubicTo(21.1f, 19f, 22f, 18.1f, 22f, 17f);
        p.lineTo(22f, 7f);
        p.cubicTo(22f, 5.9f, 21.1f, 5f, 20f, 5f);
        p.close();
        p.moveTo(20f, 17f);
        p.lineTo(4f, 17f);
        p.lineTo(4f, 7f);
        p.lineTo(20f, 7f);
        p.lineTo(20f, 17f);
        p.close();
        p.moveTo(11f, 8f);
        p.lineTo(13f, 8f);
        p.lineTo(13f, 10f);
        p.lineTo(11f, 10f);
        p.close();
        p.moveTo(11f, 11f);
        p.lineTo(13f, 11f);
        p.lineTo(13f, 13f);
        p.lineTo(11f, 13f);
        p.close();
        p.moveTo(8f, 8f);
        p.lineTo(10f, 8f);
        p.lineTo(10f, 10f);
        p.lineTo(8f, 10f);
        p.close();
        p.moveTo(8f, 11f);
        p.lineTo(10f, 11f);
        p.lineTo(10f, 13f);
        p.lineTo(8f, 13f);
        p.close();
        p.moveTo(5f, 11f);
        p.lineTo(7f, 11f);
        p.lineTo(7f, 13f);
        p.lineTo(5f, 13f);
        p.close();
        p.moveTo(5f, 8f);
        p.lineTo(7f, 8f);
        p.lineTo(7f, 10f);
        p.lineTo(5f, 10f);
        p.close();
        p.moveTo(8f, 14f);
        p.lineTo(16f, 14f);
        p.lineTo(16f, 16f);
        p.lineTo(8f, 16f);
        p.close();
        p.moveTo(14f, 11f);
        p.lineTo(16f, 11f);
        p.lineTo(16f, 13f);
        p.lineTo(14f, 13f);
        p.close();
        p.moveTo(14f, 8f);
        p.lineTo(16f, 8f);
        p.lineTo(16f, 10f);
        p.lineTo(14f, 10f);
        p.close();
        p.moveTo(17f, 11f);
        p.lineTo(19f, 11f);
        p.lineTo(19f, 13f);
        p.lineTo(17f, 13f);
        p.close();
        p.moveTo(17f, 8f);
        p.lineTo(19f, 8f);
        p.lineTo(19f, 10f);
        p.lineTo(17f, 10f);
        p.close();
    }

    private static void appendToolbarFloating(Path p) {
        p.moveTo(19f, 11f);
        p.lineTo(11f, 11f);
        p.lineTo(11f, 17f);
        p.lineTo(19f, 17f);
        p.lineTo(19f, 11f);
        p.close();
        p.moveTo(23f, 19f);
        p.lineTo(23f, 4.98f);
        p.cubicTo(23f, 3.88f, 22.1f, 3f, 21f, 3f);
        p.lineTo(3f, 3f);
        p.cubicTo(1.9f, 3f, 1f, 3.88f, 1f, 4.98f);
        p.lineTo(1f, 19f);
        p.cubicTo(1f, 20.1f, 1.9f, 21f, 3f, 21f);
        p.lineTo(21f, 21f);
        p.cubicTo(22.1f, 21f, 23f, 20.1f, 23f, 19f);
        p.close();
        p.moveTo(21f, 19.02f);
        p.lineTo(3f, 19.02f);
        p.lineTo(3f, 4.97f);
        p.lineTo(21f, 4.97f);
        p.lineTo(21f, 19.02f);
        p.close();
    }

    private static void appendToolbarDismiss(Path p) {
        p.moveTo(6f, 9f);
        p.lineTo(12f, 15f);
        p.lineTo(18f, 9f);
    }

    private static void appendShift(Path p) {
        p.moveTo(12f, 4.5f);
        p.lineTo(4.5f, 12.5f);
        p.lineTo(8.5f, 12.5f);
        p.lineTo(8.5f, 19f);
        p.lineTo(15.5f, 19f);
        p.lineTo(15.5f, 12.5f);
        p.lineTo(19.5f, 12.5f);
        p.close();
    }

    private static void appendCapsLock(Path p) {
        p.moveTo(12f, 4.5f);
        p.lineTo(4.5f, 12.5f);
        p.lineTo(8.5f, 12.5f);
        p.lineTo(8.5f, 16f);
        p.lineTo(15.5f, 16f);
        p.lineTo(15.5f, 12.5f);
        p.lineTo(19.5f, 12.5f);
        p.close();
        p.moveTo(8.5f, 19.5f);
        p.lineTo(15.5f, 19.5f);
    }

    private static void appendBackspace(Path p) {
        p.moveTo(9f, 5.5f);
        p.lineTo(19.5f, 5.5f);
        p.cubicTo(20.3284f, 5.5f, 21f, 6.1716f, 21f, 7f);
        p.lineTo(21f, 17f);
        p.cubicTo(21f, 17.8284f, 20.3284f, 18.5f, 19.5f, 18.5f);
        p.lineTo(9f, 18.5f);
        p.lineTo(3f, 12f);
        p.close();
        p.moveTo(11.5f, 9.5f);
        p.lineTo(16.5f, 14.5f);
        p.moveTo(16.5f, 9.5f);
        p.lineTo(11.5f, 14.5f);
    }

    private static void appendReturn(Path p) {
        p.moveTo(19f, 6f);
        p.lineTo(19f, 11.5f);
        p.cubicTo(19f, 12.6046f, 18.1046f, 13.5f, 17f, 13.5f);
        p.lineTo(6f, 13.5f);
        p.moveTo(9.5f, 10f);
        p.lineTo(6f, 13.5f);
        p.lineTo(9.5f, 17f);
    }

    private static void appendKeyEmoji(Path p) {
        p.moveTo(12f, 20.5f);
        p.cubicTo(16.6944f, 20.5f, 20.5f, 16.6944f, 20.5f, 12f);
        p.cubicTo(20.5f, 7.3056f, 16.6944f, 3.5f, 12f, 3.5f);
        p.cubicTo(7.3056f, 3.5f, 3.5f, 7.3056f, 3.5f, 12f);
        p.cubicTo(3.5f, 16.6944f, 7.3056f, 20.5f, 12f, 20.5f);
        p.close();
        p.moveTo(8.5f, 14.2f);
        p.cubicTo(8.5f, 14.2f, 9.7f, 16f, 12f, 16f);
        p.cubicTo(14.3f, 16f, 15.5f, 14.2f, 15.5f, 14.2f);
        p.moveTo(9f, 9.8f);
        p.lineTo(9.01f, 9.8f);
        p.moveTo(15f, 9.8f);
        p.lineTo(15.01f, 9.8f);
    }

    private static void appendMic(Path p) {
        p.moveTo(12f, 5f);
        p.cubicTo(13.1046f, 5f, 14f, 5.8954f, 14f, 7f);
        p.lineTo(14f, 11f);
        p.cubicTo(14f, 12.1046f, 13.1046f, 13f, 12f, 13f);
        p.cubicTo(10.8954f, 13f, 10f, 12.1046f, 10f, 11f);
        p.lineTo(10f, 7f);
        p.cubicTo(10f, 5.8954f, 10.8954f, 5f, 12f, 5f);
        p.close();
        p.moveTo(8f, 10.5f);
        p.cubicTo(8f, 12.7091f, 9.7909f, 14.5f, 12f, 14.5f);
        p.cubicTo(14.2091f, 14.5f, 16f, 12.7091f, 16f, 10.5f);
        p.moveTo(12f, 14.5f);
        p.lineTo(12f, 17f);
    }

    private static void appendChevron(Path p) {
        p.moveTo(6f, 9.5f);
        p.lineTo(12f, 15.5f);
        p.lineTo(18f, 9.5f);
    }

    private static void appendHandwriting(Path p) {
        p.moveTo(12f, 20f);
        p.lineTo(21f, 20f);
        p.moveTo(16.4f, 3.6f);
        p.cubicTo(16.9359f, 3.0641f, 17.717f, 2.8548f, 18.449f, 3.051f);
        p.cubicTo(19.1811f, 3.2471f, 19.7529f, 3.8189f, 19.949f, 4.551f);
        p.cubicTo(20.1452f, 5.283f, 19.9359f, 6.0641f, 19.4f, 6.6f);
        p.lineTo(7f, 19f);
        p.lineTo(3f, 20f);
        p.lineTo(4f, 16f);
        p.close();
    }

    private static void appendLexicon(Path p) {
        p.moveTo(2f, 4f);
        p.lineTo(8f, 4f);
        p.cubicTo(10.2091f, 4f, 12f, 5.7909f, 12f, 8f);
        p.lineTo(12f, 21f);
        p.cubicTo(12f, 19.3431f, 10.6569f, 18f, 9f, 18f);
        p.lineTo(2f, 18f);
        p.close();
        p.moveTo(22f, 4f);
        p.lineTo(16f, 4f);
        p.cubicTo(13.7909f, 4f, 12f, 5.7909f, 12f, 8f);
        p.lineTo(12f, 21f);
        p.cubicTo(12f, 19.3431f, 13.3431f, 18f, 15f, 18f);
        p.lineTo(22f, 18f);
        p.close();
    }

    private static void appendKeyboardHeight(Path p) {
        p.moveTo(12f, 22f);
        p.lineTo(12f, 16f);
        p.moveTo(12f, 8f);
        p.lineTo(12f, 2f);
        p.moveTo(4f, 12f);
        p.lineTo(2f, 12f);
        p.moveTo(10f, 12f);
        p.lineTo(8f, 12f);
        p.moveTo(16f, 12f);
        p.lineTo(14f, 12f);
        p.moveTo(22f, 12f);
        p.lineTo(20f, 12f);
        p.moveTo(15f, 19f);
        p.lineTo(12f, 22f);
        p.lineTo(9f, 19f);
        p.moveTo(15f, 5f);
        p.lineTo(12f, 2f);
        p.lineTo(9f, 5f);
    }

    private static void appendSettings(Path p) {
        p.moveTo(20f, 7f);
        p.lineTo(11f, 7f);
        p.moveTo(14f, 17f);
        p.lineTo(5f, 17f);
        p.moveTo(17f, 20f);
        p.cubicTo(18.6569f, 20f, 20f, 18.6569f, 20f, 17f);
        p.cubicTo(20f, 15.3431f, 18.6569f, 14f, 17f, 14f);
        p.cubicTo(15.3431f, 14f, 14f, 15.3431f, 14f, 17f);
        p.cubicTo(14f, 18.6569f, 15.3431f, 20f, 17f, 20f);
        p.close();
        p.moveTo(7f, 10f);
        p.cubicTo(8.6569f, 10f, 10f, 8.6569f, 10f, 7f);
        p.cubicTo(10f, 5.3431f, 8.6569f, 4f, 7f, 4f);
        p.cubicTo(5.3431f, 4f, 4f, 5.3431f, 4f, 7f);
        p.cubicTo(4f, 8.6569f, 5.3431f, 10f, 7f, 10f);
        p.close();
    }

    private static void appendKeySound(Path p) {
        p.moveTo(11f, 4.7f);
        p.cubicTo(11.0042f, 4.4137f, 10.8336f, 4.1538f, 10.5693f, 4.0437f);
        p.cubicTo(10.305f, 3.9336f, 10.0003f, 3.9955f, 9.8f, 4.2f);
        p.lineTo(6.4f, 7.6f);
        p.cubicTo(6.1333f, 7.8613f, 5.7733f, 8.0053f, 5.4f, 8f);
        p.lineTo(3f, 8f);
        p.cubicTo(2.4477f, 8f, 2f, 8.4477f, 2f, 9f);
        p.lineTo(2f, 15f);
        p.cubicTo(2f, 15.5523f, 2.4477f, 16f, 3f, 16f);
        p.lineTo(5.4f, 16f);
        p.cubicTo(5.7733f, 15.9947f, 6.1333f, 16.1387f, 6.4f, 16.4f);
        p.lineTo(9.8f, 19.8f);
        p.cubicTo(10.0003f, 20.0045f, 10.305f, 20.0664f, 10.5693f, 19.9563f);
        p.cubicTo(10.8336f, 19.8462f, 11.0042f, 19.5863f, 11f, 19.3f);
        p.close();
        p.moveTo(16f, 9f);
        p.cubicTo(17.3333f, 10.7778f, 17.3333f, 13.2222f, 16f, 15f);
        p.moveTo(19.4f, 18.4f);
        p.cubicTo(21.11f, 16.7093f, 22.0723f, 14.4047f, 22.0723f, 12f);
        p.cubicTo(22.0723f, 9.5953f, 21.11f, 7.2907f, 19.4f, 5.6f);
    }

    private static void appendVibration(Path p) {
        p.moveTo(2f, 8f);
        p.lineTo(4f, 10f);
        p.lineTo(2f, 12f);
        p.lineTo(4f, 14f);
        p.lineTo(2f, 16f);
        p.moveTo(22f, 8f);
        p.lineTo(20f, 10f);
        p.lineTo(22f, 12f);
        p.lineTo(20f, 14f);
        p.lineTo(22f, 16f);
        p.moveTo(9f, 5f);
        p.lineTo(15f, 5f);
        p.cubicTo(15.5523f, 5f, 16f, 5.4477f, 16f, 6f);
        p.lineTo(16f, 18f);
        p.cubicTo(16f, 18.5523f, 15.5523f, 19f, 15f, 19f);
        p.lineTo(9f, 19f);
        p.cubicTo(8.4477f, 19f, 8f, 18.5523f, 8f, 18f);
        p.lineTo(8f, 6f);
        p.cubicTo(8f, 5.4477f, 8.4477f, 5f, 9f, 5f);
        p.close();
    }

    private static void appendOneHand(Path p) {
        p.moveTo(18f, 11f);
        p.lineTo(18f, 6f);
        p.cubicTo(18f, 4.8954f, 17.1046f, 4f, 16f, 4f);
        p.cubicTo(14.8954f, 4f, 14f, 4.8954f, 14f, 6f);
        p.moveTo(14f, 10f);
        p.lineTo(14f, 4f);
        p.cubicTo(14f, 2.8954f, 13.1046f, 2f, 12f, 2f);
        p.cubicTo(10.8954f, 2f, 10f, 2.8954f, 10f, 4f);
        p.lineTo(10f, 6f);
        p.moveTo(10f, 10.5f);
        p.lineTo(10f, 6f);
        p.cubicTo(10f, 4.8954f, 9.1046f, 4f, 8f, 4f);
        p.cubicTo(6.8954f, 4f, 6f, 4.8954f, 6f, 6f);
        p.lineTo(6f, 14f);
        p.moveTo(18f, 8f);
        p.cubicTo(18f, 6.8954f, 18.8954f, 6f, 20f, 6f);
        p.cubicTo(21.1046f, 6f, 22f, 6.8954f, 22f, 8f);
        p.lineTo(22f, 14f);
        p.cubicTo(22f, 18.4183f, 18.4183f, 22f, 14f, 22f);
        p.lineTo(12f, 22f);
        p.cubicTo(9.2f, 22f, 7.5f, 21.14f, 6f, 19.66f);
        p.lineTo(2.4f, 16.06f);
        p.cubicTo(1.6854f, 15.2686f, 1.7174f, 14.0556f, 2.4727f, 13.303f);
        p.cubicTo(3.2281f, 12.5503f, 4.4411f, 12.5226f, 5.23f, 13.24f);
        p.lineTo(7f, 15f);
    }

    private static void appendIncognito(Path p) {
        p.moveTo(20f, 13f);
        p.cubicTo(20f, 18f, 16.5f, 20.5f, 12.34f, 21.95f);
        p.cubicTo(12.1222f, 22.0238f, 11.8855f, 22.0203f, 11.67f, 21.94f);
        p.cubicTo(7.5f, 20.5f, 4f, 18f, 4f, 13f);
        p.lineTo(4f, 6f);
        p.cubicTo(4f, 5.4477f, 4.4477f, 5f, 5f, 5f);
        p.cubicTo(7f, 5f, 9.5f, 3.8f, 11.24f, 2.28f);
        p.cubicTo(11.6777f, 1.9061f, 12.3223f, 1.9061f, 12.76f, 2.28f);
        p.cubicTo(14.51f, 3.81f, 17f, 5f, 19f, 5f);
        p.cubicTo(19.5523f, 5f, 20f, 5.4477f, 20f, 6f);
        p.close();
        p.moveTo(9f, 12f);
        p.lineTo(11f, 14f);
        p.lineTo(15f, 10f);
    }

    private static void appendFeedback(Path p) {
        p.moveTo(21f, 15f);
        p.cubicTo(21f, 16.1046f, 20.1046f, 17f, 19f, 17f);
        p.lineTo(7f, 17f);
        p.lineTo(3f, 21f);
        p.lineTo(3f, 5f);
        p.cubicTo(3f, 3.8954f, 3.8954f, 3f, 5f, 3f);
        p.lineTo(19f, 3f);
        p.cubicTo(20.1046f, 3f, 21f, 3.8954f, 21f, 5f);
        p.close();
        p.moveTo(13f, 8f);
        p.lineTo(7f, 8f);
        p.moveTo(17f, 12f);
        p.lineTo(7f, 12f);
    }

    private static void appendAbout(Path p) {
        p.moveTo(12f, 22f);
        p.cubicTo(17.5228f, 22f, 22f, 17.5228f, 22f, 12f);
        p.cubicTo(22f, 6.4772f, 17.5228f, 2f, 12f, 2f);
        p.cubicTo(6.4772f, 2f, 2f, 6.4772f, 2f, 12f);
        p.cubicTo(2f, 17.5228f, 6.4772f, 22f, 12f, 22f);
        p.close();
        p.moveTo(12f, 16f);
        p.lineTo(12f, 12f);
        p.moveTo(12f, 8f);
        p.lineTo(12.01f, 8f);
    }

    private static void appendCheck(Path p) {
        p.moveTo(5f, 12.5f);
        p.lineTo(9.5f, 17f);
        p.lineTo(19f, 7.5f);
    }

    private static void appendAiAssist(Path p) {
        p.moveTo(12f, 3f);
        p.lineTo(13.9f, 8.1f);
        p.lineTo(19f, 10f);
        p.lineTo(13.9f, 11.9f);
        p.lineTo(12f, 17f);
        p.lineTo(10.1f, 11.9f);
        p.lineTo(5f, 10f);
        p.lineTo(10.1f, 8.1f);
        p.close();
        p.moveTo(19f, 15f);
        p.lineTo(19.8f, 17.2f);
        p.lineTo(22f, 18f);
        p.lineTo(19.8f, 18.8f);
        p.lineTo(19f, 21f);
        p.lineTo(18.2f, 18.8f);
        p.lineTo(16f, 18f);
        p.lineTo(18.2f, 17.2f);
        p.close();
    }

    private static void appendLocalInput(Path p) {
        p.moveTo(4f, 6f);
        p.cubicTo(4f, 4.8954f, 4.8954f, 4f, 6f, 4f);
        p.lineTo(18f, 4f);
        p.cubicTo(19.1046f, 4f, 20f, 4.8954f, 20f, 6f);
        p.lineTo(20f, 15f);
        p.lineTo(4f, 15f);
        p.close();
        p.moveTo(2f, 19f);
        p.lineTo(22f, 19f);
    }

    private static void appendVoiceResult(Path p) {
        p.moveTo(9f, 5f);
        p.cubicTo(9f, 3.3431f, 10.3431f, 2f, 12f, 2f);
        p.cubicTo(13.6569f, 2f, 15f, 3.3431f, 15f, 5f);
        p.lineTo(15f, 11f);
        p.cubicTo(15f, 12.6569f, 13.6569f, 14f, 12f, 14f);
        p.cubicTo(10.3431f, 14f, 9f, 12.6569f, 9f, 11f);
        p.close();
        p.moveTo(5f, 10f);
        p.cubicTo(5f, 13.866f, 8.134f, 17f, 12f, 17f);
        p.cubicTo(15.866f, 17f, 19f, 13.866f, 19f, 10f);
        p.moveTo(12f, 17f);
        p.lineTo(12f, 21f);
        p.moveTo(8f, 21f);
        p.lineTo(16f, 21f);
    }

    private static void appendVibrationStrength(Path p) {
        p.moveTo(22f, 12f);
        p.lineTo(18f, 12f);
        p.lineTo(15f, 21f);
        p.lineTo(9f, 3f);
        p.lineTo(6f, 12f);
        p.lineTo(2f, 12f);
    }

    private static void appendClipboardHistory(Path p) {
        p.moveTo(9f, 2f);
        p.lineTo(15f, 2f);
        p.cubicTo(15.5523f, 2f, 16f, 2.4477f, 16f, 3f);
        p.lineTo(16f, 5f);
        p.cubicTo(16f, 5.5523f, 15.5523f, 6f, 15f, 6f);
        p.lineTo(9f, 6f);
        p.cubicTo(8.4477f, 6f, 8f, 5.5523f, 8f, 5f);
        p.lineTo(8f, 3f);
        p.cubicTo(8f, 2.4477f, 8.4477f, 2f, 9f, 2f);
        p.close();
        p.moveTo(16f, 4f);
        p.lineTo(18f, 4f);
        p.cubicTo(19.1046f, 4f, 20f, 4.8954f, 20f, 6f);
        p.lineTo(20f, 20f);
        p.cubicTo(20f, 21.1046f, 19.1046f, 22f, 18f, 22f);
        p.lineTo(6f, 22f);
        p.cubicTo(4.8954f, 22f, 4f, 21.1046f, 4f, 20f);
        p.lineTo(4f, 6f);
        p.cubicTo(4f, 4.8954f, 4.8954f, 4f, 6f, 4f);
        p.lineTo(8f, 4f);
        p.moveTo(12f, 11f);
        p.lineTo(16f, 11f);
        p.moveTo(12f, 16f);
        p.lineTo(16f, 16f);
        p.moveTo(8f, 11f);
        p.lineTo(8.01f, 11f);
        p.moveTo(8f, 16f);
        p.lineTo(8.01f, 16f);
    }

    private static void appendSwapSide(Path p) {
        p.moveTo(15f, 18f);
        p.lineTo(9f, 12f);
        p.lineTo(15f, 6f);
    }

    private static void appendCursorLeft(Path p) {
        p.moveTo(19f, 12f);
        p.lineTo(5f, 12f);
        p.moveTo(12f, 19f);
        p.lineTo(5f, 12f);
        p.lineTo(12f, 5f);
    }

    private static void appendToggleNext(Path p) {
        p.moveTo(5f, 12f);
        p.lineTo(19f, 12f);
        p.moveTo(12f, 5f);
        p.lineTo(19f, 12f);
        p.lineTo(12f, 19f);
    }

    private static void appendExitOneHand(Path p) {
        p.moveTo(15f, 3f);
        p.lineTo(21f, 3f);
        p.lineTo(21f, 9f);
        p.moveTo(9f, 21f);
        p.lineTo(3f, 21f);
        p.lineTo(3f, 15f);
        p.moveTo(21f, 3f);
        p.lineTo(14f, 10f);
        p.moveTo(3f, 21f);
        p.lineTo(10f, 14f);
    }

    private static void appendFloating(Path p) {
        p.moveTo(21f, 9f);
        p.lineTo(21f, 6f);
        p.cubicTo(21f, 4.8954f, 20.1046f, 4f, 19f, 4f);
        p.lineTo(4f, 4f);
        p.cubicTo(2.8954f, 4f, 2f, 4.8954f, 2f, 6f);
        p.lineTo(2f, 16f);
        p.cubicTo(2f, 17.1046f, 2.8954f, 18f, 4f, 18f);
        p.lineTo(8f, 18f);
        p.moveTo(14f, 13f);
        p.lineTo(20f, 13f);
        p.cubicTo(21.1046f, 13f, 22f, 13.8954f, 22f, 15f);
        p.lineTo(22f, 18f);
        p.cubicTo(22f, 19.1046f, 21.1046f, 20f, 20f, 20f);
        p.lineTo(14f, 20f);
        p.cubicTo(12.8954f, 20f, 12f, 19.1046f, 12f, 18f);
        p.lineTo(12f, 15f);
        p.cubicTo(12f, 13.8954f, 12.8954f, 13f, 14f, 13f);
        p.close();
    }

    private static void appendTextEdit(Path p) {
        p.moveTo(5f, 4f);
        p.lineTo(6f, 4f);
        p.cubicTo(7.6569f, 4f, 9f, 5.3431f, 9f, 7f);
        p.cubicTo(9f, 5.3431f, 10.3431f, 4f, 12f, 4f);
        p.lineTo(13f, 4f);
        p.moveTo(13f, 20f);
        p.lineTo(12f, 20f);
        p.cubicTo(10.3431f, 20f, 9f, 18.6569f, 9f, 17f);
        p.cubicTo(9f, 18.6569f, 7.6569f, 20f, 6f, 20f);
        p.lineTo(5f, 20f);
        p.moveTo(5f, 16f);
        p.lineTo(4f, 16f);
        p.cubicTo(2.8954f, 16f, 2f, 15.1046f, 2f, 14f);
        p.lineTo(2f, 10f);
        p.cubicTo(2f, 8.8954f, 2.8954f, 8f, 4f, 8f);
        p.lineTo(5f, 8f);
        p.moveTo(13f, 8f);
        p.lineTo(20f, 8f);
        p.cubicTo(21.1046f, 8f, 22f, 8.8954f, 22f, 10f);
        p.lineTo(22f, 14f);
        p.cubicTo(22f, 15.1046f, 21.1046f, 16f, 20f, 16f);
        p.lineTo(13f, 16f);
        p.moveTo(9f, 7f);
        p.lineTo(9f, 17f);
    }

    private static void appendTrash(Path p) {
        p.moveTo(3f, 6f);
        p.lineTo(21f, 6f);
        p.moveTo(19f, 6f);
        p.lineTo(19f, 20f);
        p.cubicTo(19f, 21.1046f, 18.1046f, 22f, 17f, 22f);
        p.lineTo(7f, 22f);
        p.cubicTo(5.8954f, 22f, 5f, 21.1046f, 5f, 20f);
        p.lineTo(5f, 6f);
        p.moveTo(8f, 6f);
        p.lineTo(8f, 4f);
        p.cubicTo(8f, 2.8954f, 8.8954f, 2f, 10f, 2f);
        p.lineTo(14f, 2f);
        p.cubicTo(15.1046f, 2f, 16f, 2.8954f, 16f, 4f);
        p.lineTo(16f, 6f);
        p.moveTo(10f, 11f);
        p.lineTo(10f, 17f);
        p.moveTo(14f, 11f);
        p.lineTo(14f, 17f);
    }
}
