package app.msime.android;

import android.graphics.Color;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.StateListDrawable;

/** Shared drawable factories for host UI surfaces. */
public final class DrawablePolicy {
    private DrawablePolicy() {}

    public static GradientDrawable circle(int color) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.OVAL);
        shape.setColor(color);
        return shape;
    }

    /** Create a filled circle with a visible outline. */
    public static GradientDrawable circleOutlined(int fillColor, int strokeWidth,
                                                  int strokeColor) {
        GradientDrawable shape = circle(fillColor);
        shape.setStroke(Math.max(1, strokeWidth), strokeColor);
        return shape;
    }

    public static GradientDrawable rounded(int color, float radiusPx) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.RECTANGLE);
        shape.setColor(color);
        shape.setCornerRadius(radiusPx);
        return shape;
    }

    public static GradientDrawable rounded(int color, float[] radii) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.RECTANGLE);
        shape.setColor(color);
        shape.setCornerRadii(radii);
        return shape;
    }

    public static GradientDrawable outlined(float radiusPx, int strokeWidth, int strokeColor) {
        GradientDrawable shape = rounded(Color.TRANSPARENT, radiusPx);
        shape.setStroke(Math.max(1, strokeWidth), strokeColor);
        return shape;
    }

    /** Create a filled rounded rectangle with a visible outline. */
    public static GradientDrawable outlined(int fillColor, float radiusPx, int strokeWidth,
                                            int strokeColor) {
        GradientDrawable shape = rounded(fillColor, radiusPx);
        shape.setStroke(Math.max(1, strokeWidth), strokeColor);
        return shape;
    }

    /** Create a filled rounded rectangle with a dashed outline. */
    public static GradientDrawable outlinedDashed(int fillColor, float radiusPx, int strokeWidth,
                                                  int strokeColor, float dashWidth,
                                                  float dashGap) {
        GradientDrawable shape = rounded(fillColor, radiusPx);
        shape.setStroke(Math.max(1, strokeWidth), strokeColor, dashWidth, dashGap);
        return shape;
    }

    /** Build a drawable state list while keeping the supplied state precedence. */
    public static StateListDrawable stateList(int[][] states, Drawable... drawables) {
        if (states == null || drawables == null || states.length != drawables.length) {
            throw new IllegalArgumentException("state and drawable counts differ");
        }
        StateListDrawable result = new StateListDrawable();
        for (int index = 0; index < states.length; index++) {
            result.addState(states[index], drawables[index]);
        }
        return result;
    }
}
