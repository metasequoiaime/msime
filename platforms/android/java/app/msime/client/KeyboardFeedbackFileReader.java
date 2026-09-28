package app.msime.client;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;

/** Reads the cross-process feedback file without trusting its size metadata. */
final class KeyboardFeedbackFileReader {
    private static final int MAX_BYTES = 4096;

    private KeyboardFeedbackFileReader() {}

    static byte[] read(Path file) throws IOException {
        try (InputStream input = Files.newInputStream(file)) {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream(MAX_BYTES);
            byte[] buffer = new byte[8192];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (bytes.size() > MAX_BYTES - count) throw new IOException("feedback size");
                bytes.write(buffer, 0, count);
            }
            return bytes.toByteArray();
        }
    }
}
