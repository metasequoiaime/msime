package app.msime.client;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.stream.Stream;
import java.nio.charset.StandardCharsets;

public final class BootstrapMarkerSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        Path exact = Files.createTempFile("bootstrap-marker", ".txt");
        Path oversized = Files.createTempFile("bootstrap-marker", ".txt");
        try {
            String stamp = "1234567890123";
            Files.write(exact, stamp.getBytes(StandardCharsets.UTF_8));
            Files.write(oversized, new byte[65]);
            check(stamp.equals(Bootstrap.readMarker(exact)));
            check(Bootstrap.readMarker(oversized) == null);
        } finally {
            Files.deleteIfExists(exact);
            Files.deleteIfExists(oversized);
        }
        Path root = Files.createTempDirectory("bootstrap-delete-tree");
        Path outside = Files.createTempDirectory("bootstrap-delete-outside");
        try {
            Path sentinel = outside.resolve("keep.txt");
            Files.writeString(sentinel, "synthetic");
            Path link = root.resolve("offline-glosses");
            Files.createSymbolicLink(link, outside);
            java.lang.reflect.Method deleteTree = Bootstrap.class.getDeclaredMethod("deleteTree", java.io.File.class);
            deleteTree.setAccessible(true);
            deleteTree.invoke(null, link.toFile());
            check(Files.exists(outside));
            check(Files.exists(sentinel));
            check(!Files.exists(link));
        } finally {
            try (Stream<Path> paths = Files.walk(root)) {
                paths.sorted(Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
            try (Stream<Path> paths = Files.walk(outside)) {
                paths.sorted(Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }
        System.out.println("Android bootstrap marker bounds passed");
    }
}
