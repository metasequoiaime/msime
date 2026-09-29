package app.msime.android;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.Paths;

/** Resolves snapshot inputs without following links inside the app-owned files root. */
final class DictionarySnapshotPathPolicy {
    private DictionarySnapshotPathPolicy() {}

    static Path privateSource(Path filesRoot, Path queueRoot, String value) throws IOException {
        if (filesRoot == null || queueRoot == null || value == null)
            throw new IOException("invalid snapshot source");
        Path files = filesRoot.toAbsolutePath().normalize();
        Path queue = queueRoot.toAbsolutePath().normalize();
        Path source;
        try { source = Paths.get(value).toAbsolutePath().normalize(); }
        catch (RuntimeException error) { throw new IOException("invalid snapshot source", error); }
        if (!queue.startsWith(files) || !source.startsWith(queue))
            throw new IOException("invalid snapshot source");
        Path current = files;
        for (Path component : files.relativize(source)) {
            current = current.resolve(component);
            if (Files.isSymbolicLink(current)) throw new IOException("invalid snapshot source");
        }
        if (!Files.isRegularFile(source, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("invalid snapshot source");
        return source;
    }
}
