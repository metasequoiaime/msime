package app.msime.android.home;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.provider.Settings;
import android.view.inputmethod.InputMethodInfo;
import android.view.inputmethod.InputMethodManager;

/**
 * The two setup facts the host reports about its keyboard, and the two ways to fix them.
 *
 * <p>Shared by the 设置 tab's status card and onboarding's first step, so both read the system the same way. Enabled and default are separate facts: a keyboard can be enabled and never used because another one stays the default.
 */
final class ImeSetup {
    private static final String SERVICE = "app.msime.android.MSIMEInputService";

    private ImeSetup() {}

    /** Whether this host is in the system's enabled list, rather than merely installed. */
    static boolean enabled(Context context) {
        InputMethodManager manager = context.getSystemService(InputMethodManager.class);
        if (manager == null) return false;
        for (InputMethodInfo info : manager.getEnabledInputMethodList()) {
            if (context.getPackageName().equals(info.getPackageName())
                && SERVICE.equals(info.getServiceName())) return true;
        }
        return false;
    }

    /** Whether this host is the system's current default input method, not just an enabled one. */
    static boolean isDefault(Context context) {
        String id = Settings.Secure.getString(context.getContentResolver(),
            Settings.Secure.DEFAULT_INPUT_METHOD);
        ComponentName component = id == null ? null : ComponentName.unflattenFromString(id);
        return component != null
            && context.getPackageName().equals(component.getPackageName())
            && SERVICE.equals(component.getClassName());
    }

    /** The system's on-screen keyboard list, where the user turns this keyboard on. */
    static void openSettings(Context context) {
        context.startActivity(new Intent(Settings.ACTION_INPUT_METHOD_SETTINGS));
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
