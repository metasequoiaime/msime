package app.msime.android.home;

import android.content.Context;

/** 统计页认证只属于这台设备，不参与账号同步。默认关闭：每次看统计都要验证太打扰，需要的人自己打开。 */
final class StatisticsAuthenticationPreference {
    private static final String STORE = "statistics-authentication";
    private static final String KEY = "enabled";

    static boolean enabled(Context context) {
        return context.getSharedPreferences(STORE, Context.MODE_PRIVATE).getBoolean(KEY, false);
    }

    static void save(Context context, boolean enabled) {
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE).edit().putBoolean(KEY, enabled).apply();
    }
}
