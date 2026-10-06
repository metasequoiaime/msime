package app.msime.android;

import android.graphics.Color;

/** Shared parsing for optional Android theme and skin colours. */
public final class ColorPolicy {
    private ColorPolicy() {}

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
