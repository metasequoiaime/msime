package app.msime.client.home;

import android.content.Context;
import androidx.annotation.Nullable;
import app.msime.client.KeyboardSkin;
import app.msime.client.NativeClient;
import app.msime.client.TypingStatisticsDocument;
import app.msime.client.TypingStatisticsModel;
import app.msime.client.policy.HostOptionsPolicy;
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
        File files = context.getFilesDir();
        if (files == null) return "";
        File options = new File(files, "runtime-options.json");
        if (!options.isFile()) return "";
        try {
            JSONObject root = new JSONObject(HostOptionsPolicy.read(options));
            return root.optString("preferences_directory", "");
        } catch (JSONException | java.io.IOException | SecurityException error) {
            return "";
        }
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
     * @return the saved snapshot, or null when the write was refused or failed
     */
    @Nullable public static JSONObject savePreferences(Context context, JSONObject snapshot) {
        String directory = directory(context);
        if (directory.isEmpty() || snapshot == null) return null;
        final long revision;
        final String document;
        try {
            JSONObject pending = new JSONObject(snapshot.toString());
            revision = pending.getLong("revision");
            pending.put("format_version", FORMAT_VERSION);
            document = pending.toString();
        } catch (JSONException error) {
            return null;
        }
        return value(call(() -> NativeClient.savePreferences(directory, revision, document)));
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
        String directory = statisticsDirectory(context);
        if (directory.isEmpty() || action == null) return null;
        final String request;
        try {
            request = new JSONObject().put("directory", directory).put("action", action).toString();
        } catch (JSONException error) {
            return null;
        }
        return TypingStatisticsDocument.from(value(call(() -> NativeClient.typingStatistics(request))));
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
            return root.optBoolean("ok", false) ? root.optJSONObject("value") : null;
        } catch (JSONException error) {
            return null;
        }
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
