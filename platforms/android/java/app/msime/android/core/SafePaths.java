package app.msime.android;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;

/**
 * The host's one symbolic-link check for storage paths, the Java side of crates/path-trust: a planted link must not redirect what the keyboard writes, but the links the system itself puts on the way to every app's storage have to be passed.
 *
 * <p>Android 11 and later isolate app data, and inside an app's mount namespace {@code /data/user/0} is a link to {@code /data/data}; {@code Context.getFilesDir()} of the primary user goes through it, and {@code adb shell} does not show it. macOS's {@code /tmp} and {@code /var} are links into {@code /private}, where the JVM smoke tests put their temporary directories. Each is trusted only with that exact target. Keep this list equal to {@code SYSTEM_ALIASES} in crates/path-trust/src/lib.rs.
 */
public final class SafePaths {
    private static final String[][] SYSTEM_ALIASES = {
        {"/data/user/0", "/data/data"},
        {"/tmp", "/private/tmp"},
        {"/var", "/private/var"},
    };

    private SafePaths() {}

    /** Whether {@code path} is one of the system's links and {@code target}, as read from it, resolves to the one place that link is trusted to point. */
    static boolean trustedSystemAliasTarget(Path path, Path target) {
        for (String[] alias : SYSTEM_ALIASES) {
            if (!path.equals(Path.of(alias[0]))) continue;
            Path parent = path.getParent() == null ? path.getRoot() : path.getParent();
            return parent.resolve(target).normalize().equals(Path.of(alias[1]));
        }
        return false;
    }

    static boolean isTrustedSystemAlias(Path path) {
        try {
            return trustedSystemAliasTarget(path, Files.readSymbolicLink(path));
        } catch (IOException | UnsupportedOperationException | SecurityException error) {
            return false;
        }
    }

    /** Rejects a symbolic link at any level of {@code path}, the last level included, except at most one trusted system link above the last level. Missing levels are fine: the caller is about to create them. */
    public static void rejectSymlinkComponents(Path path) throws IOException {
        if (path == null) throw new IOException("path unavailable");
        Path absolute = path.toAbsolutePath().normalize();
        Path current = absolute.getRoot();
        if (current == null) throw new IOException("path unavailable");
        boolean sawSystemAlias = false;
        int remaining = absolute.getNameCount();
        for (Path component : absolute) {
            current = current.resolve(component);
            remaining--;
            if (!Files.isSymbolicLink(current)) continue;
            if (remaining == 0 || sawSystemAlias || !isTrustedSystemAlias(current))
                throw new IOException("path contains a symbolic link");
            sawSystemAlias = true;
        }
    }

    /** Creates {@code directory} after checking that no link redirects it, and checks again once it exists. */
    public static void ensureDirectory(Path directory) throws IOException {
        rejectSymlinkComponents(directory);
        Path absolute = directory.toAbsolutePath().normalize();
        if (Files.exists(absolute, LinkOption.NOFOLLOW_LINKS)
                && !Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("directory unavailable");
        Files.createDirectories(absolute);
        rejectSymlinkComponents(absolute);
        if (!Files.isDirectory(absolute, LinkOption.NOFOLLOW_LINKS))
            throw new IOException("directory unavailable");
    }
}
