package app.msime.android.home;

import android.content.Context;
import androidx.annotation.Nullable;
import app.msime.android.AppThemePalette;
import app.msime.android.KeyboardSkin;
import app.msime.android.NativeClient;
import app.msime.android.PreferencesRevisionPolicy;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.TypingStatisticsDocument;
import app.msime.android.TypingStatisticsModel;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.time.LocalDate;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 设置界面这一侧对共享宿主状态的读写。
 *
 * <p>The settings app and the input service are separate processes over one directory, and the
 * shared store is what serialises them: preferences save under a compare-and-swap on the revision
 * the caller read, and statistics go through the same lock the keyboard writes under. This class is
 * that door and nothing more -- it holds no state, so a keyboard that changed a setting while this
 * screen was open is seen on the next read rather than cached over.
 *
 * <p>Every call here takes a file lock. None of them may run on the main thread.
 */
public final class HostStore {
    /** The snapshot format this host writes; the shared store rejects anything else. */
    private static final int FORMAT_VERSION = 1;

    private HostStore() {}

    /**
     * Where the shared preferences and statistics live, or an empty string before first setup.
     *
     * <p>Written by Bootstrap into the runtime options the input service also reads. Guessing a
     * path instead would give the settings screen its own private store that the keyboard never
     * looks at, which is the one failure mode that looks like it is working.
     */
    public static String directory(Context context) {
        return runtimeOption(context, "preferences_directory");
    }

    /** The runtime options' `language_dictionaries` directory, the one the keyboard reads Cantonese, Zhuyin and Stroke from, or an empty string when the configuration names none. */
    public static String languageDictionaries(Context context) {
        return runtimeOption(context, "language_dictionaries");
    }

    private static String runtimeOption(Context context, String key) {
        File files = context.getFilesDir();
        if (files == null) return "";
        return HostOptionsPolicy.readOption(files, key);
    }

    /**
     * Whether first-install preparation has run.
     *
     * <p>Distinct from a read that failed. On a fresh install the shared store does not exist yet,
     * and telling the user that reading it failed sends them looking for a fault instead of at the
     * one step that is missing.
     */
    public static boolean prepared(Context context) {
        return !directory(context).isEmpty();
    }

    /** The whole snapshot -- `revision` and `preferences` -- or null when it cannot be read. */
    @Nullable public static JSONObject loadPreferences(Context context) {
        String directory = directory(context);
        if (directory.isEmpty()) return null;
        return value(call(() -> NativeClient.loadPreferences(directory)));
    }

    /**
     * Write one edited snapshot back, refusing if the keyboard changed it first.
     *
     * @param snapshot the object {@link #loadPreferences} returned, with `preferences` edited
     * @return the saved snapshot, or null when the write was refused or failed; a saved write also marks the settings sync section dirty
     */
    @Nullable public static JSONObject savePreferences(Context context, JSONObject snapshot) {
        String directory = directory(context);
        if (directory.isEmpty() || snapshot == null) return null;
        final long revision;
        final String document;
        try {
            JSONObject pending = new JSONObject(snapshot.toString());
            revision = PreferencesRevisionPolicy.read(pending.opt("revision"), -1);
            if (revision < 0) return null;
            pending.put("format_version", FORMAT_VERSION);
            document = pending.toString();
        } catch (JSONException error) {
            return null;
        }
        JSONObject saved = value(call(() -> NativeClient.savePreferences(directory, revision, document)));
        // Every user write marks settings dirty here, so no caller can forget it and lose the edit to the next cloud download. Cloud downloads go through NativeClient.accountSettingsApply, not this method, so they never mark themselves dirty.
        if (saved != null) SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
        return saved;
    }

    /** One preference edited and saved in a single read-modify-write. */
    @Nullable public static JSONObject putPreference(Context context, String key, Object value) {
        JSONObject snapshot = loadPreferences(context);
        if (snapshot == null) return null;
        try {
            snapshot.getJSONObject("preferences").put(key, value);
        } catch (JSONException error) {
            return null;
        }
        return savePreferences(context, snapshot);
    }

    /**
     * The shared global theme catalog's entries in picker order, or an empty list when the host cannot answer. Unlike the calls above this reads no file and takes no lock, so it may run on the main thread.
     */
    public static JSONArray themeCatalog() {
        JSONObject catalog = value(call(NativeClient::themeCatalog));
        JSONArray themes = catalog == null ? null : catalog.optJSONArray("themes");
        return themes == null ? new JSONArray() : themes;
    }

    /**
     * The touch keyboard the stored global theme draws, in the mode `screen_keyboard_theme` and the app mode pick (`systemDark` stands in for the Android night mode). It goes through the same resolver the input service uses and, like {@link #themeCatalog}, takes no lock. A refused answer draws the Material 3 keyboard.
     */
    public static KeyboardSkin keyboardSkin(JSONObject preferences, boolean systemDark) {
        boolean dark = KeyboardSkin.resolveDark(
            preferences.optString("screen_keyboard_theme", "follow"),
            preferences.optString("theme", "system"), systemDark);
        String globalTheme = preferences.optString("global_theme", "system");
        JSONObject customTheme = preferences.optJSONObject("custom_theme");
        JSONObject theme = value(call(() -> NativeClient.resolveTheme(
            KeyboardSkin.themeRequest(globalTheme, customTheme, dark))));
        if (theme == null) return KeyboardSkin.system(dark);
        return KeyboardSkin.resolved(theme, KeyboardSkin.themeTitle(themeCatalog(), globalTheme),
            dark, customTheme == null ? null : customTheme.optJSONObject("keyboard"));
    }

    /**
     * {@link #keyboardSkin(JSONObject, boolean)} with the app theme's seed for the 跟随系统 skin.
     *
     * <p>跟随系统（`global_theme` 为 `system`）时键盘颜色由应用主题当前季节的种子推导（设计令牌 §1.4），与 `:ime` 画出来的一致，所以这里直接用 {@link KeyboardSkin#system(boolean, AppThemePalette.Seed)}，不再问解析器；其他皮肤与两参数形式相同。与两参数形式一样不拿锁。
     *
     * @param seed the app theme's seed, normally {@link #seed(Context)}; null draws the unseeded palette
     */
    public static KeyboardSkin keyboardSkin(JSONObject preferences, boolean systemDark,
            AppThemePalette.Seed seed) {
        if (!"system".equals(preferences.optString("global_theme", "system")))
            return keyboardSkin(preferences, systemDark);
        boolean dark = KeyboardSkin.resolveDark(
            preferences.optString("screen_keyboard_theme", "follow"),
            preferences.optString("theme", "system"), systemDark);
        return KeyboardSkin.system(dark, seed);
    }

    /**
     * The app theme's seed colours the host last resolved for today's season, or 秋杉 (the base theme) before the first resolution. Reads only this app's own SharedPreferences, so it may run on the main thread.
     */
    public static AppThemePalette.Seed seed(Context context) {
        return AppThemeController.cachedSeed(context);
    }

    /**
     * Run one typing-statistics action against the same directory the keyboard records into and return the raw `value` of the answer.
     *
     * <p>For the operations whose answers are not the statistics document itself (the derived summary, badges and the like). The directory is chosen by {@link #statisticsDirectory} so that the page and the input service never read two different stores. Takes the statistics lock; call it on a worker.
     *
     * @param action the action object, e.g. {@code {"operation": "summary", ...}}
     * @return the envelope's `value`, or null when the store is unavailable or refused the action
     */
    @Nullable public static JSONObject statisticsAction(Context context, JSONObject action) {
        return statisticsValue(context, action);
    }

    @Nullable public static TypingStatisticsModel loadStatistics(Context context) {
        return statistics(context, action("load"));
    }

    @Nullable public static TypingStatisticsModel setStatisticsEnabled(Context context,
            boolean enabled) {
        JSONObject action = action("set_enabled");
        if (action == null) return null;
        try {
            action.put("enabled", enabled);
        } catch (JSONException error) {
            return null;
        }
        return statistics(context, action);
    }

    /** `forever`, `30d`, `90d`, `180d` or `365d`; the window counts back from the caller's day. */
    @Nullable public static TypingStatisticsModel setStatisticsRetention(Context context,
            String retention) {
        JSONObject action = action("set_retention");
        if (action == null) return null;
        try {
            action.put("retention", retention).put("day", LocalDate.now().toString());
        } catch (JSONException error) {
            return null;
        }
        return statistics(context, action);
    }

    @Nullable public static TypingStatisticsModel resetStatistics(Context context) {
        return statistics(context, action("reset"));
    }

    @Nullable private static TypingStatisticsModel statistics(Context context, JSONObject action) {
        return TypingStatisticsDocument.from(statisticsValue(context, action));
    }

    /** One statistics request in the shared directory; the envelope's `value`, or null for any failure. */
    @Nullable private static JSONObject statisticsValue(Context context, @Nullable JSONObject action) {
        String directory = statisticsDirectory(context);
        if (directory.isEmpty() || action == null) return null;
        final String request;
        try {
            request = new JSONObject().put("directory", directory).put("action", action).toString();
        } catch (JSONException error) {
            return null;
        }
        return value(call(() -> NativeClient.typingStatistics(request)));
    }

    /**
     * Where the counts are kept.
     *
     * <p>The input service files them beside the preferences when it has a directory and under the
     * bootstrap state root otherwise, so this reader follows the same order. Reading only one of
     * the two would show an empty page to a profile that has been recording all along.
     */
    private static String statisticsDirectory(Context context) {
        String preferences = directory(context);
        if (!preferences.isEmpty()) return preferences;
        File files = context.getFilesDir();
        return files == null ? "" : new File(files, "bootstrap/state").getAbsolutePath();
    }

    @Nullable private static JSONObject action(String operation) {
        try {
            return new JSONObject().put("operation", operation);
        } catch (JSONException error) {
            return null;
        }
    }

    /** The `value` of a successful shared-host envelope, or null for a failure of any kind. */
    @Nullable private static JSONObject value(@Nullable String response) {
        if (response == null) return null;
        try {
            JSONObject root = new JSONObject(response);
            return Boolean.TRUE.equals(root.opt("ok")) ? root.optJSONObject("value") : null;
        } catch (JSONException error) {
            return null;
        }
    }

    /** Native envelopes use a typed JSON status; reject org.json's string coercion. */
    static boolean strictOk(Object value) {
        return value instanceof Boolean && (Boolean) value;
    }

    @Nullable private static String call(Call call) {
        try {
            return call.run();
        } catch (Exception | LinkageError error) {
            // A settings screen has nothing to do with a host that will not load or answer; every
            // caller here already has an "unavailable" state to show.
            return null;
        }
    }

    private interface Call {
        String run() throws Exception;
    }
}
