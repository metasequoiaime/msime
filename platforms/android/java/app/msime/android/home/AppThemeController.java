package app.msime.android.home;

import android.content.Context;
import android.content.SharedPreferences;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.AppThemePalette;
import app.msime.android.AppThemeResolver;
import java.time.LocalDate;
import java.time.ZoneId;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 宿主的应用主题：读 Android 本地设置里的 `general.app_theme`（{@link AndroidLocalSettings#APP_THEME}，不在共享偏好里），按本地日期的月份交给 Rust 解析出这一季的种子色，再把季节和种子缓存进宿主自己的 `msime_home_v1`。
 *
 * <p>季节规则和种子色都只在 Rust（`msime_client_resolve_app_theme`）里：这里只问它，不自己按月份推季节，所以「水杉四季」在各平台同一天换季。缓存的季节由 {@link AppMode#restore} 在每个宿主 activity 的 `super.onCreate` 之前叠加成主题，缓存的种子供跟随系统皮肤的键盘预览取色（{@link HostStore#seed}）。
 *
 * <p>解析是纯计算；本地设置文件很小、读一次有缓存，但仍和偏好一样只在工作线程读，所以调用方在拿到偏好快照之后再调用 {@link #follow}。
 */
final class AppThemeController {
    /** 本地设置里没有应用主题时的默认值，与 Rust 的 `AppTheme::default()` 一致。 */
    static final String DEFAULT_THEME = "siji";

    private static final String STORE = "msime_home_v1";
    private static final String SEED_LIGHT_KEY = "app_theme_seed_light";
    private static final String SEED_DARK_KEY = "app_theme_seed_dark";

    private AppThemeController() {}

    /**
     * 按本地设置里的应用主题与今天的月份解析，并更新缓存。`preferences` 为 null（偏好还没读出来）时什么也不做。
     *
     * @return 缓存的季节因此变了时为 true：已经画出来的 activity 用的还是旧季节，调用方应 `recreate()`
     */
    static boolean follow(Context context, @Nullable JSONObject preferences) {
        if (preferences == null) return false;
        String theme = AndroidLocalSettings.load(context).choice(AndroidLocalSettings.APP_THEME);
        int month = LocalDate.now(ZoneId.systemDefault()).getMonthValue();
        JSONObject light = AppThemeResolver.resolve(theme, month, false);
        JSONObject dark = AppThemeResolver.resolve(theme, month, true);
        if (light == null || dark == null || AppThemePalette.Seed.fromResolved(light, dark) == null) return false;
        String season = light.optString("season", "");
        SharedPreferences store = store(context);
        String previous = store.getString(AppMode.SEASON_KEY, null);
        store.edit()
            .putString(AppMode.SEASON_KEY, season)
            .putString(SEED_LIGHT_KEY, light.toString())
            .putString(SEED_DARK_KEY, dark.toString())
            .apply();
        // 从来没缓存过时宿主画的是基础主题秋杉；第一次解析到秋天也就不必重建。
        String drawn = previous == null ? "autumn" : previous;
        return !drawn.equals(season);
    }

    /** 上次 {@link #follow} 缓存的季节，没有时为 null。 */
    @Nullable static String cachedSeason(Context context) {
        return store(context).getString(AppMode.SEASON_KEY, null);
    }

    /** 上次缓存的种子；没有缓存或缓存读不懂时是基础主题的秋杉。 */
    static AppThemePalette.Seed cachedSeed(Context context) {
        SharedPreferences store = store(context);
        String light = store.getString(SEED_LIGHT_KEY, null);
        String dark = store.getString(SEED_DARK_KEY, null);
        if (light == null || dark == null) return AppThemePalette.Seed.AUTUMN;
        try {
            AppThemePalette.Seed seed = AppThemePalette.Seed.fromResolved(new JSONObject(light), new JSONObject(dark));
            return seed == null ? AppThemePalette.Seed.AUTUMN : seed;
        } catch (JSONException error) {
            return AppThemePalette.Seed.AUTUMN;
        }
    }

    private static SharedPreferences store(Context context) {
        return context.getSharedPreferences(STORE, Context.MODE_PRIVATE);
    }
}
