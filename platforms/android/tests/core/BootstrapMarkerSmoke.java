package app.msime.android;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.stream.Stream;
import java.nio.charset.StandardCharsets;
import java.io.ByteArrayInputStream;

public final class BootstrapMarkerSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) throws Exception {
        Path exact = Files.createTempFile("bootstrap-marker", ".txt");
        Path oversized = Files.createTempFile("bootstrap-marker", ".txt");
        Path markerRoot = Files.createTempDirectory("bootstrap-marker-link");
        Path markerOutside = Files.createTempFile("bootstrap-marker-outside", ".txt");
        try {
            String stamp = "1234567890123";
            Files.write(exact, stamp.getBytes(StandardCharsets.UTF_8));
            Files.write(oversized, new byte[65]);
            check(stamp.equals(Bootstrap.readMarker(exact)));
            check(Bootstrap.readMarker(oversized) == null);
            Files.write(markerOutside, stamp.getBytes(StandardCharsets.UTF_8));
            Path linked = markerRoot.resolve(".package");
            Files.createSymbolicLink(linked, markerOutside);
            check(Bootstrap.readMarker(linked) == null);

            Path configurationOutside = Files.createTempFile("bootstrap-config-outside", ".json");
            Path configurationLink = markerRoot.resolve("runtime-options.json");
            Files.createSymbolicLink(configurationLink, configurationOutside);
            boolean configurationRejected = false;
            try {
                Bootstrap.existingConfiguration(configurationLink.toFile());
            } catch (java.io.IOException expected) {
                configurationRejected = true;
            }
            check(configurationRejected);
            Files.deleteIfExists(configurationOutside);
        } finally {
            Files.deleteIfExists(exact);
            Files.deleteIfExists(oversized);
            Files.deleteIfExists(markerRoot.resolve(".package"));
            Files.deleteIfExists(markerRoot.resolve("runtime-options.json"));
            Files.deleteIfExists(markerRoot);
            Files.deleteIfExists(markerOutside);
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
        Path lockRoot = Files.createTempDirectory("bootstrap-lock-root");
        Path lockTarget = Files.createTempFile("bootstrap-lock-target", ".lock");
        try {
            Path linkedLock = lockRoot.resolve("bootstrap.lock");
            Files.createSymbolicLink(linkedLock, lockTarget);
            boolean rejected = false;
            java.nio.channels.FileChannel opened = null;
            try {
                opened = Bootstrap.openLock(linkedLock);
            } catch (java.io.IOException expected) {
                rejected = true;
            } finally {
                if (opened != null) opened.close();
            }
            check(rejected);
        } finally {
            Files.deleteIfExists(lockRoot.resolve("bootstrap.lock"));
            Files.deleteIfExists(lockRoot);
            Files.deleteIfExists(lockTarget);
        }
        Path boundaryRoot = Files.createTempDirectory("bootstrap-boundary-root");
        Path boundaryOutside = Files.createTempDirectory("bootstrap-boundary-outside");
        try {
            Files.createSymbolicLink(boundaryRoot.resolve("bootstrap"), boundaryOutside);
            boolean rejected = false;
            try {
                Bootstrap.ensureSafeDirectory(boundaryRoot.resolve("bootstrap/resources"));
            } catch (java.io.IOException expected) {
                rejected = true;
            }
            check(rejected);
            check(!Files.exists(boundaryOutside.resolve("resources")));
        } finally {
            Files.deleteIfExists(boundaryRoot.resolve("bootstrap"));
            Files.deleteIfExists(boundaryRoot);
            Files.deleteIfExists(boundaryOutside.resolve("resources"));
            Files.deleteIfExists(boundaryOutside);
        }
        Path copyRoot = Files.createTempDirectory("bootstrap-copy-root");
        Path copyOutside = Files.createTempDirectory("bootstrap-copy-outside");
        try {
            Path destination = copyRoot.resolve("table.txt");
            Path sentinel = copyOutside.resolve("sentinel.txt");
            Files.writeString(sentinel, "keep");
            Files.createSymbolicLink(destination, sentinel);
            Bootstrap.copyAsset(new ByteArrayInputStream("replacement".getBytes(StandardCharsets.UTF_8)), destination);
            check(Files.readString(sentinel).equals("keep"));
            check(Files.readString(destination).equals("replacement"));
        } finally {
            Files.deleteIfExists(copyRoot.resolve("table.txt"));
            Files.deleteIfExists(copyRoot);
            Files.deleteIfExists(copyOutside.resolve("sentinel.txt"));
            Files.deleteIfExists(copyOutside);
        }
        System.out.println("Android bootstrap marker bounds passed");
    }
}
