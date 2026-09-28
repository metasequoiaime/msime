package app.msime.client.policy;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/** Bounded access to the small runtime-options document shared by the Android processes. */
public final class HostOptionsPolicy {
    /** The bootstrap document contains metadata and preferences, never dictionary payloads. */
    public static final int MAX_BYTES = 16 * 1024;

    private HostOptionsPolicy() {}

    /** Returns UTF-8 contents, or an empty string when the file is missing, unreadable or too large. */
    public static String read(File file) throws IOException {
        if (file == null || !file.isFile()) return "";
        if (Files.size(file.toPath()) > MAX_BYTES) return "";
        try (InputStream input = Files.newInputStream(file.toPath())) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(MAX_BYTES);
            byte[] buffer = new byte[4096];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() + count > MAX_BYTES) return "";
                bytes.write(buffer, 0, count);
            }
            return new String(bytes.toByteArray(), StandardCharsets.UTF_8);
        }
    }
}
