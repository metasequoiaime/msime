package app.msime.android.home;

import android.content.ComponentName;
import android.content.Context;
import android.content.pm.PackageManager;
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

    /**
     * 选择面板和「我的」列表画的图标，与启动器显示的是同一张：系统设了 config_useRoundIcon（圆形遮罩的设备）时，框架解析清单就把别名的图标换成 android:roundIcon，读别名解析后的图标就跟着换，不用自己判断遮罩。别名禁用时照样读得到（MATCH_DISABLED_COMPONENTS）；读不到时退回清单里的方形图标。
     */
    public static int resource(Context context, AppIconStyle style) {
        try {
            int icon = context.getPackageManager().getActivityInfo(
                new ComponentName(context, style.component()),
                PackageManager.MATCH_DISABLED_COMPONENTS).icon;
            if (icon != 0) return icon;
        } catch (PackageManager.NameNotFoundException missing) {
            // The aliases are this app's own manifest entries; fall through to the square icon.
        }
        return resource(style);
    }

    /** 清单里 android:icon 的那一张（圆角方形遮罩下的图标）。 */
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
