package app.msime.android;

import android.content.Context;
import android.graphics.Color;
import androidx.annotation.AttrRes;
import androidx.annotation.ColorInt;
import com.google.android.material.color.MaterialColors;

/** Shared Material theme colour lookups used by Android host surfaces. */
public final class ThemeColorPolicy {
    private ThemeColorPolicy() { }

    /** 读一个颜色主题属性；属性缺失时退回洋红，让漏配的属性在截图里一眼可见，而不是悄悄显示成别的颜色。 */
    @ColorInt public static int color(Context context, @AttrRes int attr) {
        return MaterialColors.getColor(context, attr, Color.MAGENTA);
    }

    @ColorInt public static int accent(Context context) {
        return color(context, androidx.appcompat.R.attr.colorPrimary);
    }

    @ColorInt public static int onAccent(Context context) {
        return color(context, com.google.android.material.R.attr.colorOnPrimary);
    }

    @ColorInt public static int accentSoft(Context context) {
        return color(context, com.google.android.material.R.attr.colorPrimaryContainer);
    }

    @ColorInt public static int card(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurfaceContainer);
    }

    @ColorInt public static int rowBackground(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurfaceContainerLowest);
    }

    @ColorInt public static int sheetBackground(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurfaceContainerLow);
    }

    @ColorInt public static int page(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurface);
    }

    @ColorInt public static int text(Context context) {
        return color(context, com.google.android.material.R.attr.colorOnSurface);
    }

    @ColorInt public static int subText(Context context) {
        return color(context, com.google.android.material.R.attr.colorOnSurfaceVariant);
    }

    @ColorInt public static int outline(Context context) {
        return color(context, com.google.android.material.R.attr.colorOutline);
    }

    @ColorInt public static int hairline(Context context) {
        return color(context, com.google.android.material.R.attr.colorOutlineVariant);
    }

    @ColorInt public static int danger(Context context) {
        return color(context, androidx.appcompat.R.attr.colorError);
    }

}
