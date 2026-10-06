package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;

/** Reads the cross-process feedback file without trusting its size metadata. */
final class KeyboardFeedbackFileReader {
    private static final int MAX_BYTES = 4096;

    private KeyboardFeedbackFileReader() {}

    static byte[] read(Path file) throws IOException {
        if (file == null || Files.isSymbolicLink(file)
                || !Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("feedback path is not a regular file");
        try (InputStream input = Files.newInputStream(file)) {
            byte[] bytes = HttpBodyPolicy.readBounded(input, MAX_BYTES);
            if (bytes == null) throw new IOException("feedback size");
            return bytes;
        }
    }
}
