package app.msime.android.home;

import android.content.Context;
import app.msime.android.AppIconStyle;
import app.msime.android.LauncherIcons;
import app.msime.android.R;

/** 设置页的图标入口；与输入法进程共用组件协调，真实宿主不参与图标切换。 */
public final class AppIcons {
    private AppIcons() {}

    /** 当前配色，未选彩色别名时为经典。 */
    public static AppIconStyle selected(Context context) {
        return LauncherIcons.selected(context);
    }

    /** 先保住宿主与所选别名，再关闭其余别名；失败时交给设置页提示。 */
    public static boolean select(Context context, AppIconStyle style) {
        return LauncherIcons.select(context, style);
    }

    /** 选择面板和「我的」列表共用同一张彩色图标。 */
    public static int resource(AppIconStyle style) {
        return switch (style) {
            case CLASSIC -> R.drawable.app_icon_classic;
            case FOREST -> R.drawable.app_icon_forest;
            case SKY -> R.drawable.app_icon_sky;
            case DUSK -> R.drawable.app_icon_dusk;
            case VERMILION -> R.drawable.app_icon_vermilion;
        };
    }
}
