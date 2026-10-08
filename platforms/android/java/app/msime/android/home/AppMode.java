package app.msime.android.home;

import android.content.Context;
import android.content.SharedPreferences;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatDelegate;
import app.msime.android.R;
import app.msime.android.core.InputViewValuePolicy;
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
    /** 应用主题最近一次解析到的季节（`spring`、`summer`、`autumn`、`winter`），由应用主题控制器在读到共享偏好后写入；没有时用基础主题的秋杉。 */
    static final String SEASON_KEY = "app_theme_season";

    private AppMode() {}

    /** 恢复上次应用的深浅模式，并给这个 activity 的主题叠上缓存季节的配色。每个宿主 activity 都在 `super.onCreate` 之前调用它，所以进程被恢复到任何一个页面时，第一帧就是对的模式和季节。 */
    static void restore(Context context) {
        SharedPreferences store = context.getSharedPreferences(STORE, Context.MODE_PRIVATE);
        apply(store.getString(KEY, SYSTEM));
        int overlay = seasonOverlay(store.getString(SEASON_KEY, null));
        // 基础主题本身就是秋杉，没有缓存时不必再叠一层。
        if (overlay != 0) context.getTheme().applyStyle(overlay, true);
    }

    /** 季节对应的主题叠加层；不认识的值和 null 都回到基础主题的秋杉，返回 0 表示不用叠加。 */
    private static int seasonOverlay(@Nullable String season) {
        if (season == null) return 0;
        switch (season) {
            case "spring": return R.style.ThemeOverlay_MSIME_Season_Spring;
            case "summer": return R.style.ThemeOverlay_MSIME_Season_Summer;
            case "autumn": return R.style.ThemeOverlay_MSIME_Season_Autumn;
            case "winter": return R.style.ThemeOverlay_MSIME_Season_Winter;
            default: return 0;
        }
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
        String mode = InputViewValuePolicy.textOr(preferences, "theme", SYSTEM);
        return LIGHT.equals(mode) || DARK.equals(mode) ? mode : SYSTEM;
    }

    /** Whether this context is drawing dark right now: the system's night mode, or the one `theme` forces. */
    static boolean dark(Context context) {
        return Ui.isNight(context);
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
