package app.msime.android.home;

import android.content.Context;
import android.content.Intent;
import android.provider.Settings;
import android.view.inputmethod.InputMethodManager;

/**
 * The two setup facts the host reports about its keyboard, and the two ways to fix them.
 *
 * <p>Shared by the 设置 tab's status card and onboarding's first step, so both read the system the same way. Enabled and default are separate facts: a keyboard can be enabled and never used because another one stays the default.
 */
final class ImeSetup {
    private ImeSetup() {}

    /** 是否在系统已启用输入法列表中。 */
    static boolean enabled(Context context) {
        return app.msime.android.DeviceInfoCollector.keyboardEnabled(context);
    }

    /** 是否为系统当前默认输入法。 */
    static boolean isDefault(Context context) {
        return app.msime.android.DeviceInfoCollector.keyboardDefault(context);
    }

    /** The system's on-screen keyboard list, where the user turns this keyboard on. */
    static void openSettings(Context context) {
        context.startActivity(new Intent(Settings.ACTION_INPUT_METHOD_SETTINGS));
    }

    /** Opens the keyboard settings and reports whether the system accepted the intent. */
    static boolean tryOpenSettings(Context context) {
        try {
            context.startActivity(new Intent(Settings.ACTION_INPUT_METHOD_SETTINGS));
            return true;
        } catch (RuntimeException unavailable) {
            return false;
        }
    }

    /** The system's keyboard picker; only an enabled keyboard is listed there, so before that the settings list is the way. */
    static void makeDefault(Context context) {
        if (!enabled(context)) {
            openSettings(context);
            return;
        }
        InputMethodManager manager = context.getSystemService(InputMethodManager.class);
        if (manager != null) manager.showInputMethodPicker();
    }
}
