package app.msime.android;

import app.msime.android.LayoutPolicy;

/** Pure drag math for the transparent keyboard-layout adjustment surface. */
public final class KeyboardLayoutAdjustPolicy {
    public enum Axis { HORIZONTAL, VERTICAL }

    private static final float SPACING_DRAG_SCALE_DP = 18f;

    private KeyboardLayoutAdjustPolicy() { }

    public static Axis chooseAxis(float translationX, float translationY, Axis previous) {
        if (previous != null) return previous;
        if (!Float.isFinite(translationX) || !Float.isFinite(translationY)) return null;
        return Math.abs(translationY) >= Math.abs(translationX)
            ? Axis.VERTICAL : Axis.HORIZONTAL;
    }

    public static int keySpacingFromDrag(int baseTenths, float translationDp) {
        if (!Float.isFinite(translationDp)) return KeyboardSpacingPolicy.keySpacing(baseTenths);
        int delta = Math.round(translationDp * 10f / SPACING_DRAG_SCALE_DP);
        return BoundsPolicy.bounded(baseTenths + delta, KeyboardSpacingPolicy.MIN_KEY_SPACING_TENTHS,
            KeyboardSpacingPolicy.MAX_KEY_SPACING_TENTHS);
    }

    public static int rowSpacingFromDrag(int baseTenths, float translationDp) {
        if (!Float.isFinite(translationDp)) return KeyboardSpacingPolicy.rowSpacing(baseTenths);
        int delta = Math.round(translationDp * 10f / SPACING_DRAG_SCALE_DP);
        return BoundsPolicy.bounded(baseTenths + delta, KeyboardSpacingPolicy.MIN_ROW_SPACING_TENTHS,
            KeyboardSpacingPolicy.MAX_ROW_SPACING_TENTHS);
    }

    public static int heightFromDrag(int baseAdjustment, float translationDp) {
        if (!Float.isFinite(translationDp)) return KeyboardHeightPolicy.heightAdjustment(baseAdjustment);
        int delta = Math.round(translationDp);
        return BoundsPolicy.bounded(KeyboardHeightPolicy.heightAdjustment(baseAdjustment) - delta,
            KeyboardHeightPolicy.MIN_HEIGHT_ADJUSTMENT_DP,
            KeyboardHeightPolicy.MAX_HEIGHT_ADJUSTMENT_DP);
    }
}
