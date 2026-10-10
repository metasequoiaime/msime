package app.msime.android;

import android.content.Context;
import android.content.res.Configuration;
import android.util.TypedValue;
import android.view.View;

/** Shared display and density conversions used by Android hosts and UI components. */
public final class DimensionPolicy {
    private DimensionPolicy() {}

    /** Return the shorter of two dimensions for proportional control sizing. */
    public static float shorterSide(float width, float height) {
        return BoundsPolicy.atMost(width, height);
    }

    /** Return the current display width in physical pixels. */
    public static int screenWidthPixels(Context context) {
        return context.getResources().getDisplayMetrics().widthPixels;
    }

    /** Return whether the supplied context currently uses the system night configuration. */
    public static boolean isNight(Context context) {
        return (context.getResources().getConfiguration().uiMode
            & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
    }

    /** Read the display density used by Android layout calculations. */
    public static float density(Context context) {
        return context.getResources().getDisplayMetrics().density;
    }

    /** Return a view's non-negative width after horizontal padding. */
    public static int contentWidth(View view) {
        return BoundsPolicy.nonNegative(view.getWidth() - view.getPaddingLeft() - view.getPaddingRight());
    }

    /** Return a view's non-negative height after vertical padding. */
    public static int contentHeight(View view) {
        return BoundsPolicy.nonNegative(view.getHeight() - view.getPaddingTop() - view.getPaddingBottom());
    }

    /** Convert a size to pixels while guaranteeing at least one physical pixel. */
    public static int atLeastOnePixel(Context context, float dp) {
        return BoundsPolicy.atLeast(pixels(context, dp), 1);
    }

    /** Convert an integer density-independent size to pixels using Android's rounding rule. */
    public static int pixels(int dp, float density) {
        return Math.round(dp * density);
    }

    /** Convert a fractional density-independent size to rounded pixels. */
    public static int pixels(float dp, float density) {
        return Math.round(dp * density);
    }

    /** Convert a density-independent size to rounded pixels using the context's density. */
    public static int pixels(Context context, float dp) {
        return pixels(dp, density(context));
    }

    /** Convert a fractional density-independent size to pixels without rounding. */
    public static float floatPixels(double dp, float density) {
        return (float) dp * density;
    }

    /** Convert a fractional density-independent size to pixels using the context's density. */
    public static float floatPixels(Context context, double dp) {
        return floatPixels(dp, density(context));
    }

    /** Convert pixels back to density-independent units using the context's density. */
    public static float fromPixels(Context context, float pixels) {
        float density = density(context);
        return density <= 0 ? pixels : pixels / density;
    }

    /** Convert scalable text units using the view context's display metrics. */
    public static float sp(Context context, float value) {
        return TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value,
            context.getResources().getDisplayMetrics());
    }
}
