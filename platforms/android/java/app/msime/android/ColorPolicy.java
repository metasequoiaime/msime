package app.msime.android;

import android.graphics.Color;
import android.content.res.ColorStateList;

/** Shared parsing for optional Android theme and skin colours. */
public final class ColorPolicy {
    private ColorPolicy() {}

    /** Multiply a colour's existing alpha by the supplied factor. */
    public static int withAlpha(int color, float alpha) {
        int base = (color >>> 24) & 0xFF;
        return (color & 0x00FFFFFF) | (Math.round(base * alpha) << 24);
    }

    /** 把 double 透明度限制在 0–1 后乘进颜色已有的 alpha。 */
    public static int withAlpha(int color, double alpha) {
        return withAlpha(color, (float) BoundsPolicy.bounded(alpha, 0d, 1d));
    }

    /** Replace a colour's alpha channel with an explicit 0–255 value. */
    public static int withAlpha(int color, int alpha) {
        return (color & 0x00FFFFFF) | ((alpha & 0xFF) << 24);
    }

    /** Choose the paint alpha used for an enabled or disabled control. */
    public static int enabledAlpha(boolean enabled, int activeAlpha, int inactiveAlpha) {
        return enabled ? activeAlpha : inactiveAlpha;
    }

    /** Linearly interpolate each ARGB channel; callers provide an expected 0–1 amount. */
    public static int blend(int from, int to, float amount) {
        int a = blendChannel(from >>> 24, to >>> 24, amount);
        int r = blendChannel(from >> 16, to >> 16, amount);
        int g = blendChannel(from >> 8, to >> 8, amount);
        int b = blendChannel(from, to, amount);
        return (a << 24) | (r << 16) | (g << 8) | b;
    }

    private static int blendChannel(int from, int to, float amount) {
        return Math.round((from & 0xFF) + ((to & 0xFF) - (from & 0xFF)) * amount);
    }

    /** Blend opaque RGB channels after clamping the mix amount to 0–1. */
    public static int blendOpaque(int from, int to, float amount) {
        float bounded = BoundsPolicy.bounded(amount, 0f, 1f);
        return Color.rgb(
            blendChannel(from, to, bounded),
            blendChannel(from >> 8, to >> 8, bounded),
            blendChannel(from >> 16, to >> 16, bounded));
    }

    /** Build a color state list while validating that each state has a matching color. */
    public static ColorStateList stateList(int[][] states, int[] colors) {
        if (states == null || colors == null || states.length != colors.length) {
            throw new IllegalArgumentException("state and color counts differ");
        }
        return new ColorStateList(states, colors);
    }

    /** Return the fallback when the value is missing, empty, or not a valid Android colour. */
    public static int parse(String value, int fallback) {
        if (value == null || value.isEmpty()) return fallback;
        try {
            return Color.parseColor(value);
        } catch (IllegalArgumentException error) {
            return fallback;
        }
    }

    /** Replace the alpha channel of a hexadecimal RGB or ARGB string. */
    public static String withAlpha(String color, int alpha) {
        return String.format(java.util.Locale.ROOT, "#%02X%s", alpha & 0xFF,
            color.substring(color.length() - 6));
    }

    /** 将不透明 RGB 整数格式化为稳定的大写 `#RRGGBB` 颜色串。 */
    public static String hexRgb(int color) {
        return String.format(java.util.Locale.ROOT, "#%06X", color & 0xFFFFFF);
    }

    /** 将 ARGB 整数格式化为稳定的大写 `#AARRGGBB` 颜色串。 */
    public static String hexArgb(int color) {
        return String.format(java.util.Locale.ROOT, "#%08X", color);
    }

    /** Parse only the hexadecimal colour forms emitted by keyboard skin data. */
    public static int parseHex(String value, int fallback) {
        if (value == null || !value.matches("#[0-9a-fA-F]{6}|#[0-9a-fA-F]{8}")) return fallback;
        long parsed = Long.parseLong(value.substring(1), 16);
        return value.length() == 7 ? 0xff000000 | (int) parsed : (int) parsed;
    }
}
