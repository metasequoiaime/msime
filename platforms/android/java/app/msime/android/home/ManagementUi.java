package app.msime.android.home;

import android.content.Context;
import android.content.Intent;

/** 可选管理界面的合包能力与原生页面跳转契约。 */
public final class ManagementUi {
    private static final String ACTIVITY = "app.msime.android.MainActivity";

    private ManagementUi() {}

    /** 当前 APK 是否包含管理界面。 */
    public static boolean available() {
        try {
            Class.forName(ACTIVITY);
            return true;
        } catch (ClassNotFoundException absent) {
            return false;
        }
    }

    /** 创建打开共享设置页的 intent。 */
    public static Intent settingsPage(Context context, String page) {
        return intent(context).putExtra("msime_settings_page", page);
    }

    /** 创建打开共享移动面板的 intent。 */
    public static Intent mobilePanel(Context context, String panel) {
        return intent(context).putExtra("msime_mobile_panel", panel);
    }

    private static Intent intent(Context context) {
        return new Intent().setClassName(context, ACTIVITY);
    }
}
