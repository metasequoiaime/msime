package app.msime.android;

import android.content.Context;
import android.widget.TextView;

/** Shared typography and keycap sizing contract for Android keyboard controls. */
public final class KeyboardTypographyPolicy {
    private KeyboardTypographyPolicy() { }

    /** 键盘里的文字最多跟随系统字体放大到这个倍数。 */
    public static final float MAX_KEYBOARD_FONT_SCALE = 1.15f;

    /**
     * 键盘里文字实际用的字体缩放：系统设置调小时照样跟随，调大时封顶在 {@link #MAX_KEYBOARD_FONT_SCALE}。
     *
     * <p>键高、候选行高和工具栏都是固定 dp，而国产机出厂常把字体设成「大」甚至「超大」（1.3–2.0 倍）。不封顶时 22 sp 的字母在 52 dp 的键里放不下：文字超出内边距框时 TextView 不再居中，而是从上内边距处往下排，字母被挤到键底被裁掉，`123` 折成两行，「中」只剩顶上一截。系统键盘（Gboard、iOS）的键面同样不随系统字号无限放大。
     */
    public static float keyboardFontScale(float systemFontScale) {
        if (!(systemFontScale > 0) || Float.isInfinite(systemFontScale)) return 1f;
        return BoundsPolicy.atMost(systemFontScale, MAX_KEYBOARD_FONT_SCALE);
    }

    /** 键盘文字的 sp 换算成像素，字体缩放按 {@link #keyboardFontScale} 封顶。 */
    public static float keySp(Context context, float value) {
        android.content.res.Resources resources = context.getResources();
        return value * resources.getDisplayMetrics().density
            * keyboardFontScale(resources.getConfiguration().fontScale);
    }

    /** 键盘里没有另定字号的按键和文字用的字号（sp），与 Material 按钮的默认字号相同。不设时取的是系统主题里的按钮字号：各厂商不同，而且不受 {@link #keyboardFontScale} 封顶，九键的 ABC 在大字体下会折行。 */
    public static final float DEFAULT_KEY_TEXT_SP = 14f;

    /** 以 {@link #keySp} 设置键盘里控件的字号。 */
    public static void setKeyTextSize(android.widget.TextView view, float sp) {
        ViewPolicy.setTextSize(view, keySp(view.getContext(), sp));
    }

    /** 键帽左右各留的内边距（dp）：只防字形贴住圆角，键宽几乎全部留给文字。 */
    public static final int KEY_CAP_HORIZONTAL_PADDING_DP = 2;

    /**
     * 给键帽定下与系统主题无关的内边距和最小尺寸。
     *
     * <p>键帽一直沿用按钮样式自带的内边距，那份内边距来自系统主题的按钮背景：原生 Material 是左右 12 dp、上下 10 dp，各厂商的 `DeviceDefault` 主题又各不相同。TextView 把文字裁在内边距框里，36 dp 宽的字母键扣掉两侧内边距后经常放不下一个字母，于是同一个键盘在不同手机上有的正常、有的字母整排消失。键帽的文字本来就由 gravity 居中，提示、数字和图标由各自的子类在需要时另加内边距，所以这里统一归零上下、左右只留 {@link #KEY_CAP_HORIZONTAL_PADDING_DP}，并去掉字体留白和最小宽高。
     */
    public static void normalizeKeyCap(android.widget.TextView key) {
        int horizontal = DimensionPolicy.pixels(key.getContext(), KEY_CAP_HORIZONTAL_PADDING_DP);
        ViewPolicy.setHorizontalPadding(key, horizontal);
        ViewPolicy.clearFontPadding(key);
        ViewPolicy.clearMinimumSize(key);
    }

}
