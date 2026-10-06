package app.msime.android.policy;

import app.msime.android.HttpBodyPolicy;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import org.json.JSONException;
import org.json.JSONObject;

/** Bounded access to the small runtime-options document shared by the Android processes. */
public final class HostOptionsPolicy {
    /** The bootstrap document contains metadata and preferences, never dictionary payloads. */
    public static final int MAX_BYTES = 16 * 1024;

    private HostOptionsPolicy() {}

    /** Returns UTF-8 contents, or an empty string when the file is missing, unreadable or too large. */
    public static String read(File file) throws IOException {
        if (file == null || !Files.isRegularFile(file.toPath(), LinkOption.NOFOLLOW_LINKS)) return "";
        if (Files.size(file.toPath()) > MAX_BYTES) return "";
        try (InputStream input = Files.newInputStream(file.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            return new String(HttpBodyPolicy.readRequired(input, MAX_BYTES), StandardCharsets.UTF_8);
        }
    }

    /** Read the standard runtime-options document below a host files directory. */
    public static String readRuntimeOptions(File files) {
        if (files == null) return "";
        try {
            return read(new File(files, "runtime-options.json"));
        } catch (IOException error) {
            return "";
        }
    }

    /** Read one string field from the host options document, or empty when unavailable. */
    public static String readOption(File files, String key) {
        String raw = readRuntimeOptions(files);
        if (raw.isEmpty()) return "";
        try {
            return new JSONObject(raw).optString(key, "");
        } catch (JSONException error) {
            return "";
        }
    }
}
