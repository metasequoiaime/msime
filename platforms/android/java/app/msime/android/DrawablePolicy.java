package app.msime.android;

import android.content.res.ColorStateList;
import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
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
        shape.setStroke(BoundsPolicy.atLeast(strokeWidth, 1), strokeColor);
        return shape;
    }

    public static GradientDrawable rounded(int color, float radiusPx) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.RECTANGLE);
        shape.setColor(color);
        shape.setCornerRadius(radiusPx);
        return shape;
    }

    /** Create a fully rounded rectangle whose corners clamp to the short side. */
    public static GradientDrawable pill(int color) {
        return rounded(color, 9999f);
    }

    /** 创建具有固定像素尺寸的纯色圆角矩形。 */
    public static GradientDrawable rounded(int color, float radiusPx, int width, int height) {
        GradientDrawable shape = rounded(color, radiusPx);
        shape.setSize(width, height);
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
        shape.setStroke(BoundsPolicy.atLeast(strokeWidth, 1), strokeColor);
        return shape;
    }

    /** Create a filled rounded rectangle with a visible outline. */
    public static GradientDrawable outlined(int fillColor, float radiusPx, int strokeWidth,
                                            int strokeColor) {
        GradientDrawable shape = rounded(fillColor, radiusPx);
        shape.setStroke(BoundsPolicy.atLeast(strokeWidth, 1), strokeColor);
        return shape;
    }

    /** Create a filled rounded rectangle with a dashed outline. */
    public static GradientDrawable outlinedDashed(int fillColor, float radiusPx, int strokeWidth,
                                                  int strokeColor, float dashWidth,
                                                  float dashGap) {
        GradientDrawable shape = rounded(fillColor, radiusPx);
        shape.setStroke(BoundsPolicy.atLeast(strokeWidth, 1), strokeColor, dashWidth, dashGap);
        return shape;
    }

    /** 用单色状态列表、内容层和遮罩层构造平台波纹 drawable。 */
    public static RippleDrawable ripple(int color, Drawable content, Drawable mask) {
        return new RippleDrawable(ColorStateList.valueOf(color), content, mask);
    }

    /** Resolve the theme's standard selectable-item ripple. */
    public static Drawable ripple(Context context) {
        android.util.TypedValue value = new android.util.TypedValue();
        if (!context.getTheme().resolveAttribute(
                android.R.attr.selectableItemBackground, value, true)) return null;
        return context.getDrawable(value.resourceId);
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
