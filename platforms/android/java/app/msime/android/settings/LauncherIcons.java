package app.msime.android;

import android.content.ComponentName;
import android.content.Context;
import android.content.pm.PackageManager;

/** 图标组件的共同入口，宿主与输入法进程都用它恢复旧版状态并只启用一个别名。 */
public final class LauncherIcons {
    private LauncherIcons() {}

    /** 覆盖安装后保留原来明确启用的彩色别名，而不是新清单默认启用的经典别名。 */
    public static AppIconStyle selected(Context context) {
        PackageManager packages = context.getPackageManager();
        for (AppIconStyle style : AppIconStyle.all()) {
            if (style == AppIconStyle.CLASSIC) continue;
            if (packages.getComponentEnabledSetting(component(context, style.component()))
                    == PackageManager.COMPONENT_ENABLED_STATE_ENABLED) return style;
        }
        return AppIconStyle.CLASSIC;
    }

    /** 两条进程入口都协调状态，旧版禁用的真实 Activity 不依赖用户先打开设置才能恢复。 */
    public static void normalize(Context context) {
        try {
            select(context, selected(context));
        } catch (RuntimeException error) {
            android.util.Log.w("MSIMEAppIcon", "Icon state reconciliation refused", error);
        }
    }

    /** 先保证真实宿主与目标别名可用；切图标绝不禁用真实宿主。 */
    public static boolean select(Context context, AppIconStyle style) {
        PackageManager packages = context.getPackageManager();
        try {
            setEnabled(packages, component(context, HostDeepLink.HOME_ACTIVITY), true, true);
            setEnabled(packages, component(context, style.component()), true,
                style == AppIconStyle.CLASSIC);
            for (AppIconStyle other : AppIconStyle.all()) {
                if (other == style) continue;
                setEnabled(packages, component(context, other.component()), false,
                    other == AppIconStyle.CLASSIC);
            }
            return true;
        } catch (RuntimeException error) {
            android.util.Log.w("MSIMEAppIcon", "Icon switch refused for " + style.component(), error);
            return false;
        }
    }

    /** 不重复写相同状态，避免每次进程启动都排入新的延迟组件广播。 */
    private static void setEnabled(PackageManager packages, ComponentName component,
            boolean enabled, boolean manifestEnabled) {
        int state = packages.getComponentEnabledSetting(component);
        boolean current = state == PackageManager.COMPONENT_ENABLED_STATE_ENABLED
            || state == PackageManager.COMPONENT_ENABLED_STATE_DEFAULT && manifestEnabled;
        if (current == enabled) return;
        packages.setComponentEnabledSetting(component, enabled
            ? PackageManager.COMPONENT_ENABLED_STATE_ENABLED
            : PackageManager.COMPONENT_ENABLED_STATE_DISABLED, PackageManager.DONT_KILL_APP);
    }

    private static ComponentName component(Context context, String className) {
        return new ComponentName(context.getPackageName(), className);
    }
}
