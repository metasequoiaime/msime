package app.msime.android.home;

import android.content.ComponentName;
import android.content.Context;
import android.content.pm.PackageManager;
import app.msime.android.AppIconStyle;

/**
 * 切换主屏幕图标：启用一个组件，关掉其余的。
 *
 * <p>The target is enabled before the others are disabled, so the launcher never observes a package
 * with no entry point at all -- the state the user cannot get out of from the launcher. The process
 * is kept alive through the change; killing it here would close the settings screen that asked for
 * it, and the launcher picks the change up either way.
 */
public final class AppIcons {
    private AppIcons() {}

    /** The style currently enabled, which is classic unless an alias was selected. */
    public static AppIconStyle selected(Context context) {
        PackageManager packages = context.getPackageManager();
        for (AppIconStyle style : AppIconStyle.all()) {
            if (style == AppIconStyle.CLASSIC) continue;
            if (packages.getComponentEnabledSetting(component(context, style))
                == PackageManager.COMPONENT_ENABLED_STATE_ENABLED) return style;
        }
        return AppIconStyle.CLASSIC;
    }

    /** Enable one style's component and disable every other. Returns false if the change failed. */
    public static boolean select(Context context, AppIconStyle style) {
        PackageManager packages = context.getPackageManager();
        try {
            packages.setComponentEnabledSetting(component(context, style),
                PackageManager.COMPONENT_ENABLED_STATE_ENABLED, PackageManager.DONT_KILL_APP);
            for (AppIconStyle other : AppIconStyle.all()) {
                if (other == style) continue;
                packages.setComponentEnabledSetting(component(context, other),
                    PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                    PackageManager.DONT_KILL_APP);
            }
            return true;
        } catch (RuntimeException error) {
            // 记下是哪个组件、为什么被拒。组件名不是用户数据，而吞掉这个原因正是上一次查不动的
            // 原因：界面只说「系统拒绝了」，而真正的那句话在这里被扔掉了。
            android.util.Log.w("MSIMEAppIcon",
                "Icon switch refused for " + component(context, style), error);
            return false;
        }
    }

    private static ComponentName component(Context context, AppIconStyle style) {
        // 类字面量而不是字符串：applicationId 与类所在的 namespace 现在都是 app.msime.android，但两者曾经不同，写死的字符串要同时跟着两者走；类字面量在两者再分开时也不会错。
        return new ComponentName(context.getPackageName(),
            style.component(HomeActivity.class.getName()));
    }
}
