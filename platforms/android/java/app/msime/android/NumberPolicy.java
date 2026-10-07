package app.msime.android;

import java.util.Locale;

/** Shared locale-stable formatting for integer values shown in Android host copy. */
public final class NumberPolicy {
    private NumberPolicy() {}

    /** Format an integer with locale-stable thousands separators. */
    public static String grouped(long value) {
        return String.format(Locale.ROOT, "%,d", value);
    }

    /** Format a decimal with one locale-stable fractional digit. */
    public static String decimal1(double value) {
        return String.format(Locale.ROOT, "%.1f", value);
    }
}
