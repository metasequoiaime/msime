package app.msime.client.home;

import android.content.Context;
import android.content.res.Configuration;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatDelegate;
import org.json.JSONObject;

/**
 * The host's light or dark mode, taken from the shared `theme` preference (`system`, `light` or `dark`).
 *
 * <p>The keyboard already draws in this mode; without it the settings pages followed only the system, so choosing 深色 turned the keyboard dark and left the app that chose it light. The preference lives in the shared store, which may not be read on the main thread, so the last mode applied is kept in this app's own preferences and restored before the first activity draws; the store is then read as the pages load and any change is applied from there.
 */
final class AppMode {
    static final String SYSTEM = "system";
    static final String LIGHT = "light";
    static final String DARK = "dark";

    private static final String STORE = "msime_home_v1";
    private static final String KEY = "app_mode";

    private AppMode() {}

    /** Apply the mode applied last time. Called by every host activity before `super.onCreate`, so a process restored into any of them starts in the right mode. */
    static void restore(Context context) {
        apply(context.getSharedPreferences(STORE, Context.MODE_PRIVATE).getString(KEY, SYSTEM));
    }

    /** Follow the `theme` value in a freshly read preferences object; a change recreates the open activities in the new mode. */
    static void follow(Context context, @Nullable JSONObject preferences) {
        if (preferences == null) return;
        String mode = of(preferences);
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE).edit().putString(KEY, mode).apply();
        apply(mode);
    }

    /** The stored mode, with anything unrecognised read as `system`, the way the keyboard reads it. */
    static String of(JSONObject preferences) {
        String mode = preferences.optString("theme", SYSTEM);
        return LIGHT.equals(mode) || DARK.equals(mode) ? mode : SYSTEM;
    }

    /** Whether this context is drawing dark right now: the system's night mode, or the one `theme` forces. */
    static boolean dark(Context context) {
        return (context.getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK)
            == Configuration.UI_MODE_NIGHT_YES;
    }

    private static void apply(String mode) {
        int night = LIGHT.equals(mode) ? AppCompatDelegate.MODE_NIGHT_NO
            : DARK.equals(mode) ? AppCompatDelegate.MODE_NIGHT_YES
            : AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM;
        int current = AppCompatDelegate.getDefaultNightMode();
        // An app that never set a mode is already following the system; setting it anyway would walk every open activity for nothing.
        if (current == AppCompatDelegate.MODE_NIGHT_UNSPECIFIED) current = AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM;
        if (current != night) AppCompatDelegate.setDefaultNightMode(night);
    }
}
