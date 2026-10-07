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

    public static float nonNegative(float value) {
        return Math.max(0f, value);
    }

    public static int atMost(int value, int maximum) {
        return Math.min(value, maximum);
    }

    public static long atMost(long value, long maximum) {
        return Math.min(value, maximum);
    }

    public static float atMost(float value, float maximum) {
        return Math.min(value, maximum);
    }

    public static int atLeast(int value, int minimum) {
        return Math.max(value, minimum);
    }

    public static float atLeast(float value, float minimum) {
        return Math.max(value, minimum);
    }
    public static long atLeast(long value, long minimum) {
        return Math.max(value, minimum);
    }

}
