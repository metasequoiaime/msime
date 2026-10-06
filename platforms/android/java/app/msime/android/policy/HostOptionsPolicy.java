package app.msime.android.policy;

import app.msime.android.HttpBodyPolicy;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;

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
            byte[] bytes = HttpBodyPolicy.readBounded(input, MAX_BYTES);
            return bytes == null ? "" : new String(bytes, StandardCharsets.UTF_8);
        }
    }
}
