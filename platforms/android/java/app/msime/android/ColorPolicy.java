package app.msime.android;

import android.graphics.Color;
import android.content.res.ColorStateList;

/** Shared parsing for optional Android theme and skin colours. */
public final class ColorPolicy {
    private ColorPolicy() {}

    /** Multiply a colour's existing alpha by the supplied factor. */
    public static int withAlpha(int color, float alpha) {
        int base = Color.alpha(color);
        return (color & 0x00FFFFFF) | (Math.round(base * alpha) << 24);
    }

    /** Linearly interpolate each ARGB channel; callers provide an expected 0–1 amount. */
    public static int blend(int from, int to, float amount) {
        int a = Math.round(((from >>> 24) & 0xFF) + (((to >>> 24) & 0xFF) - ((from >>> 24) & 0xFF)) * amount);
        int r = Math.round(((from >> 16) & 0xFF) + (((to >> 16) & 0xFF) - ((from >> 16) & 0xFF)) * amount);
        int g = Math.round(((from >> 8) & 0xFF) + (((to >> 8) & 0xFF) - ((from >> 8) & 0xFF)) * amount);
        int b = Math.round((from & 0xFF) + ((to & 0xFF) - (from & 0xFF)) * amount);
        return (a << 24) | (r << 16) | (g << 8) | b;
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
}
