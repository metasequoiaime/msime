package app.msime.android;

import android.content.Context;
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
        Path file = file(context);
        try {
            if (Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)) {
                long bytes = Files.size(file);
                if (bytes > 0 && bytes <= MAX_BYTES) {
                    // Files.readString/writeString need API 34; this host starts at 28.
                    return decode(new String(KeyboardFeedbackFileReader.read(file), StandardCharsets.UTF_8));
                }
            }
        } catch (Exception ignored) {
            // Fall back to the defaults; a corrupt file is never replaced here.
        }
        return defaults();
    }

    public static void save(Context context, Settings settings) throws IOException {
        if (settings == null) throw new IllegalArgumentException("settings");
        saveFile(file(context), settings);
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

    static void ensureSafeDirectory(Path directory) throws IOException {
        if (directory == null) throw new IOException("feedback directory unavailable");
        SafePaths.ensureDirectory(directory);
    }

    static Settings decode(String text) throws JSONException {
        if (text == null || text.length() > MAX_BYTES) throw new JSONException("feedback size");
        JSONObject value = new JSONObject(text);
        return fromValues(value.opt("soundEnabled"), value.opt("hapticsEnabled"),
            value.opt("hapticStrength"));
    }

    /** Decode feedback values crossing a JSON or native boundary without scalar coercion. */
    public static Settings fromValues(Object soundEnabled, Object hapticsEnabled,
                                      Object hapticStrength) {
        String strength = hapticStrength instanceof String ? (String) hapticStrength : "medium";
        return new Settings(booleanValue(soundEnabled, true), booleanValue(hapticsEnabled, false),
            KeyboardFeedbackPreferences.strength(strength));
    }

    /** Persisted flags are typed JSON booleans; do not accept org.json's string coercion. */
    static Boolean strictBoolean(Object value) {
        return JsonPolicy.strictBoolean(value);
    }

    static boolean booleanValue(Object value, boolean fallback) {
        Boolean parsed = strictBoolean(value);
        return parsed == null ? fallback : parsed;
    }

    static String encode(Settings settings) throws JSONException {
        return new JSONObject()
            .put("soundEnabled", settings.soundEnabled())
            .put("hapticsEnabled", settings.hapticsEnabled())
            .put("hapticStrength", settings.hapticStrength().id())
            .toString();
    }

    // Sound on, haptics off, medium strength: the same values decode() uses for absent fields.
    private static Settings defaults() {
        return new Settings(true, false, KeyboardFeedbackPreferences.HapticStrength.MEDIUM);
    }

    private static Path file(Context context) {
        Path files = context.getFilesDir().toPath();
        return files.resolve("bootstrap").resolve("state").resolve(FILE_NAME);
    }
}
