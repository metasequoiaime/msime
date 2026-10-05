package app.msime.android;

import android.app.Dialog;
import android.os.Build;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowInsetsController;
import android.widget.LinearLayout;

/**
 * 包住键区的容器（扩展点）：键行与底行经这里放进键盘的竖向布局。现在原样放入，不加任何外层；单手模式以后在这里包一层。导航栏颜色在这里跟随键盘底色。
 */
final class ImeFrame {
    private final MSIMEInputService s;
    /** onCreateInputView 里建好的键盘竖向布局，键区放进它。 */
    LinearLayout keyboard;
    private int navigationColor;
    private boolean navigationDark;
    private boolean navigationApplied;

    ImeFrame(MSIMEInputService s) {
        this.s = s;
    }

    /** 按默认布局参数放入键区。 */
    void wrap(ViewGroup keyArea) {
        keyboard.addView(keyArea);
    }

    /** 按给定布局参数放入键区。 */
    void wrap(ViewGroup keyArea, ViewGroup.LayoutParams params) {
        keyboard.addView(keyArea, params);
    }

    /**
     * IME 窗口的导航栏跟键盘底色：Android 15 以前直接设导航栏颜色，所有版本都按底色明暗切换导航栏按钮的深浅（深色键盘配浅色按钮）。
     *
     * @param color 键盘底色（ARGB）
     * @param dark 键盘是否深色
     */
    void applyNavigationBar(int color, boolean dark) {
        if (navigationApplied && navigationColor == color && navigationDark == dark) return;
        Dialog dialog = s.getWindow();
        Window window = dialog == null ? null : dialog.getWindow();
        if (window == null) return;
        navigationApplied = true;
        navigationColor = color;
        navigationDark = dark;
        if (Build.VERSION.SDK_INT < 35) setNavigationBarColor(window, color);
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = window.getInsetsController();
            if (controller != null) {
                controller.setSystemBarsAppearance(
                    dark ? 0 : WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
                    WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
            }
        }
    }

    // Android 15 起导航栏颜色由系统按 edge-to-edge 处理，这个调用只给更早的版本。
    @SuppressWarnings("deprecation")
    private static void setNavigationBarColor(Window window, int color) {
        window.setNavigationBarColor(color);
    }
}
