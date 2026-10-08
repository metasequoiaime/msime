package app.msime.android;

import android.app.Activity;
import android.graphics.Rect;
import android.os.Build;
import android.view.View;
import android.view.WindowInsets;

/** Keep content usable with Android 15's enforced edge-to-edge windows. */
public final class WindowLayout {
    private WindowLayout() {}
    public static void theme(Activity activity) {
        activity.setTheme(android.R.style.Theme_Material_Light_NoActionBar);
    }
    public static void fitSystemBars(View root) {
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            Rect bars = systemBars(insets);
            ViewPolicy.setPadding(view, bars.left, bars.top, bars.right, bars.bottom);
            return insets;
        });
    }

    /** 系统栏占去的四边（像素）。 */
    @SuppressWarnings("deprecation")
    public static Rect systemBars(WindowInsets insets) {
        if (Build.VERSION.SDK_INT >= 30) {
            android.graphics.Insets bars = insets.getInsets(WindowInsets.Type.systemBars());
            return new Rect(bars.left, bars.top, bars.right, bars.bottom);
        }
        return new Rect(insets.getSystemWindowInsetLeft(), insets.getSystemWindowInsetTop(),
            insets.getSystemWindowInsetRight(), insets.getSystemWindowInsetBottom());
    }
}
