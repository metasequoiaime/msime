package app.msime.client;

import android.content.Context;
import android.content.SharedPreferences;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import org.json.JSONException;
import org.json.JSONObject;

/** Shared, bounded feedback settings for the app and the isolated IME process. */
public final class KeyboardFeedbackStore {
    private static final String PREFERENCES_NAME = "keyboard-feedback";
    private static final String FILE_NAME = "keyboard-feedback.json";
    private static final int MAX_BYTES = 4096;

    public static final class Settings {
        private final boolean soundEnabled;
        private final boolean hapticsEnabled;
        private final KeyboardFeedbackPreferences.HapticStrength hapticStrength;

        public Settings(boolean soundEnabled, boolean hapticsEnabled,
                        KeyboardFeedbackPreferences.HapticStrength hapticStrength) {
            this.soundEnabled = soundEnabled;
            this.hapticsEnabled = hapticsEnabled;
            this.hapticStrength = hapticStrength == null
                ? KeyboardFeedbackPreferences.HapticStrength.MEDIUM : hapticStrength;
        }

        public boolean soundEnabled() { return soundEnabled; }
        public boolean hapticsEnabled() { return hapticsEnabled; }
        public KeyboardFeedbackPreferences.HapticStrength hapticStrength() {
            return hapticStrength;
        }
    }

    private KeyboardFeedbackStore() {}

    public static Settings load(Context context) {
        Settings legacy = loadLegacy(context);
        Path file = file(context);
        try {
            if (Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)) {
                long bytes = Files.size(file);
                if (bytes > 0 && bytes <= MAX_BYTES) {
                    // Files.readString/writeString need API 34; this host starts at 28.
                    return decode(new String(KeyboardFeedbackFileReader.read(file), StandardCharsets.UTF_8));
                }
                return legacy;
            }
            // Persist the old SharedPreferences value once so both processes converge on the
            // same source of truth even when the app has never opened its settings page.
            save(context, legacy);
        } catch (Exception ignored) {
            // Keep the last usable in-memory value; a corrupt file is never replaced here.
        }
        return legacy;
    }

    public static void save(Context context, Settings settings) throws IOException {
        if (settings == null) throw new IllegalArgumentException("settings");
        Path file = file(context);
        saveFile(file, settings);
        // Keep older APKs functional during an in-place upgrade. New code always reads the
        // atomic file first, so a stale legacy copy cannot overwrite a newer setting.
        context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE).edit()
            .putBoolean(KeyboardFeedbackPreferences.SOUND_KEY, settings.soundEnabled())
            .putBoolean(KeyboardFeedbackPreferences.HAPTICS_KEY, settings.hapticsEnabled())
            .putString(KeyboardFeedbackPreferences.STRENGTH_KEY,
                settings.hapticStrength().id())
            .apply();
    }

    static void saveFile(Path file, Settings settings) throws IOException {
        if (file == null) throw new IllegalArgumentException("feedback file");
        if (settings == null) throw new IllegalArgumentException("settings");
        Path parent = file.getParent();
        if (parent == null) throw new IOException("feedback directory unavailable");
        ensureSafeDirectory(parent);
        Path temporary = Files.createTempFile(parent, FILE_NAME + ".", ".tmp");
        try {
            String encoded;
            try {
                encoded = encode(settings);
            } catch (JSONException error) {
                throw new IOException("feedback encoding failed", error);
            }
            Files.write(temporary, encoded.getBytes(StandardCharsets.UTF_8),
                StandardOpenOption.TRUNCATE_EXISTING);
            try {
                Files.move(temporary, file, StandardCopyOption.ATOMIC_MOVE,
                    StandardCopyOption.REPLACE_EXISTING);
            } catch (AtomicMoveNotSupportedException ignored) {
                Files.move(temporary, file, StandardCopyOption.REPLACE_EXISTING);
            }
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    private static void rejectSymlinkComponents(Path path) throws IOException {
        Path absolute = path.toAbsolutePath().normalize();
        Path current = absolute.getRoot();
        if (current == null) throw new IOException("feedback path unavailable");
        for (Path component : absolute) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current))
                throw new IOException("feedback path contains a symbolic link");
        }
    }

    static void ensureSafeDirectory(Path directory) throws IOException {
        if (directory == null) throw new IOException("feedback directory unavailable");
        rejectSymlinkComponents(directory);
        if (Files.exists(directory, LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(directory, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("feedback directory unavailable");
        Files.createDirectories(directory);
        rejectSymlinkComponents(directory);
        if (!Files.isDirectory(directory, LinkOption.NOFOLLOW_LINKS)
                || Files.isSymbolicLink(directory))
            throw new IOException("feedback directory unavailable");
    }

    static Settings decode(String text) throws JSONException {
        if (text == null || text.length() > MAX_BYTES) throw new JSONException("feedback size");
        JSONObject value = new JSONObject(text);
        return new Settings(
            value.optBoolean("soundEnabled", true),
            value.optBoolean("hapticsEnabled", false),
            KeyboardFeedbackPreferences.strength(value.optString("hapticStrength", "medium")));
    }

    static String encode(Settings settings) throws JSONException {
        return new JSONObject()
            .put("soundEnabled", settings.soundEnabled())
            .put("hapticsEnabled", settings.hapticsEnabled())
            .put("hapticStrength", settings.hapticStrength().id())
            .toString();
    }

    private static Settings loadLegacy(Context context) {
        SharedPreferences preferences = context.getSharedPreferences(PREFERENCES_NAME,
            Context.MODE_PRIVATE);
        return new Settings(
            preferences.getBoolean(KeyboardFeedbackPreferences.SOUND_KEY, true),
            preferences.getBoolean(KeyboardFeedbackPreferences.HAPTICS_KEY, false),
            KeyboardFeedbackPreferences.strength(preferences.getString(
                KeyboardFeedbackPreferences.STRENGTH_KEY, "medium")));
    }

    private static Path file(Context context) {
        Path files = context.getFilesDir().toPath();
        return files.resolve("bootstrap").resolve("state").resolve(FILE_NAME);
    }
}
