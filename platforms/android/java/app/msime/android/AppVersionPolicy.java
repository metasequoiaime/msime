package app.msime.android;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;

/** Android 宿主共用的当前应用版本查询。 */
public final class AppVersionPolicy {
    private AppVersionPolicy() {}

    /** 当前安装包清单里的版本名与版本码。 */
    public record Version(String name, long code) {}

    /** 查询当前安装包清单里的版本。 */
    public static Version current(Context context) throws PackageManager.NameNotFoundException {
        PackageInfo info = context.getPackageManager().getPackageInfo(context.getPackageName(), 0);
        return new Version(info.versionName, info.getLongVersionCode());
    }

    /** 返回清单里的原始版本名，并保留当前包查询失败的异常语义。 */
    public static String versionName(Context context) throws PackageManager.NameNotFoundException {
        return current(context).name();
    }

    /** 返回清单里的版本名；清单未设置或系统查不到当前包时返回调用方给出的值。 */
    public static String versionName(Context context, String fallback) {
        try {
            String versionName = versionName(context);
            return versionName == null ? fallback : versionName;
        } catch (PackageManager.NameNotFoundException missing) {
            return fallback;
        }
    }
}
