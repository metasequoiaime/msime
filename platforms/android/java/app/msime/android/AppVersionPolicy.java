package app.msime.android;

import android.content.Context;
import android.content.pm.PackageInfo;
import android.content.pm.PackageManager;

/** Android 宿主共用的当前应用版本查询。 */
public final class AppVersionPolicy {
    private static final String BUILD_CONFIG = "app.msime.android.BuildConfig";

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

    /** 发布日期（`YYYY-MM-DD`），gradle-app 写进 BuildConfig 的 `RELEASE_DATE`。check-host.sh 用 javac 直接编译这里，没有 Gradle 生成的 BuildConfig，所以经反射读。没有 BuildConfig、没有这个字段（Tauri 生成的工程不写它）或构建时取不到日期时返回 null。 */
    public static String releaseDate() {
        try {
            Object value = Class.forName(BUILD_CONFIG).getField("RELEASE_DATE").get(null);
            return value instanceof String date && !date.isEmpty() ? date : null;
        } catch (ReflectiveOperationException absent) {
            return null;
        }
    }

    /** 这个包是为 Google Play 打的：gradle-app 收到 -PmsimeDistribution=play 时把 BuildConfig 的 `DISTRIBUTION` 写成 `play`。这样的包不做应用内自更新，也不下载可执行的资源包，Play 的政策两样都不允许。经反射读的理由同 {@link #releaseDate()}；没有 BuildConfig 或这个字段时按 GitHub 渠道处理。 */
    public static boolean playDistribution() {
        try {
            return "play".equals(Class.forName(BUILD_CONFIG).getField("DISTRIBUTION").get(null));
        } catch (ReflectiveOperationException absent) {
            return false;
        }
    }

    /** 关于页显示的版本：有发布日期时是 `0.4.2（2026-10-19 发布）`，没有时就是版本名本身。反馈和使用统计报的仍是纯版本名，不经过这里。 */
    public static String displayVersion(String versionName, String releaseDate) {
        if (releaseDate == null || releaseDate.isEmpty()) return versionName;
        return versionName + "（" + releaseDate + " 发布）";
    }
}
