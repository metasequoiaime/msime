package app.msime.android;

/** Shared platform-neutral numeric bounds used by host policies and geometry adapters. */
public final class BoundsPolicy {
    private BoundsPolicy() {}

    public static int bounded(int value, int minimum, int maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    public static long bounded(long value, long minimum, long maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    public static float bounded(float value, float minimum, float maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    public static double bounded(double value, double minimum, double maximum) {
        return Math.max(minimum, Math.min(value, maximum));
    }

    public static int nonNegative(int value) {
        return Math.max(0, value);
    }

    public static long nonNegative(long value) {
        return Math.max(0, value);
    }
}
