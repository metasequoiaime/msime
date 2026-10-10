package app.msime.android;

import java.math.BigDecimal;
import java.util.Locale;
import org.json.JSONObject;

/** Shared locale-stable formatting for integer values shown in Android host copy. */
public final class NumberPolicy {
    private NumberPolicy() {}

    /** Format an integer with locale-stable thousands separators. */
    public static String grouped(long value) {
        return String.format(Locale.ROOT, "%,d", value);
    }

    /** 条数按千位分隔并限制为非负数，例如「128,406 条」。 */
    public static String groupedCount(long count) {
        return grouped(BoundsPolicy.nonNegative(count)) + " 条";
    }

    /** Read an integer JSON number without accepting fractional values or booleans. */
    public static int strictInt(JSONObject object, String key, int fallback) {
        return strictInt(object == null ? null : object.opt(key), fallback);
    }

    public static int strictInt(Object raw, int fallback) {
        if (!(raw instanceof Number) || raw instanceof Boolean) return fallback;
        if (raw instanceof Double || raw instanceof Float) return fallback;
        try {
            return new BigDecimal(raw.toString()).intValueExact();
        } catch (NumberFormatException | ArithmeticException error) {
            return fallback;
        }
    }

    /** Read a long JSON number without accepting fractional values or booleans. */
    public static long strictLong(Object raw, long fallback) {
        if (!(raw instanceof Number) || raw instanceof Boolean) return fallback;
        if (raw instanceof Double || raw instanceof Float) return fallback;
        try {
            return new BigDecimal(raw.toString()).longValueExact();
        } catch (NumberFormatException | ArithmeticException error) {
            return fallback;
        }
    }

    /** Read a finite JSON number without accepting numeric strings or booleans. */
    public static double strictDouble(Object raw, double fallback) {
        if (!(raw instanceof Number) || raw instanceof Boolean) return fallback;
        double value = ((Number) raw).doubleValue();
        return Double.isFinite(value) ? value : fallback;
    }

    /** Format a decimal with one locale-stable fractional digit. */
    public static String decimal1(double value) {
        return String.format(Locale.ROOT, "%.1f", value);
    }
}
