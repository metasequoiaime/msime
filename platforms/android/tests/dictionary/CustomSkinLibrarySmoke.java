import app.msime.android.CustomSkinLibrary;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.stream.Stream;

public final class CustomSkinLibrarySmoke {
    interface Checked { void run() throws Exception; }

    private static void check(boolean condition) {
        if (!condition) throw new AssertionError();
    }

    private static void fails(Checked action) throws Exception {
        try {
            action.run();
        } catch (java.io.IOException expected) {
            return;
        }
        throw new AssertionError("expected custom skin library rejection");
    }

    public static void main(String[] args) throws Exception {
        Path root = Files.createTempDirectory("msime-custom-skin-");
        Path outside = Files.createTempDirectory("msime-custom-skin-outside-");
        try {
            Path preferences = root.resolve("preferences");
            Path directory = preferences.resolve("CustomSkins");
            Files.createDirectories(directory);

            Path hardlinkSource = outside.resolve("library-hardlink.json");
            Files.writeString(hardlinkSource,
                "[{\"id\":\"outside\",\"name\":\"outside\",\"design\":{}}]");
            Path hardlinkLibrary = directory.resolve("library.json");
            Files.createLink(hardlinkLibrary, hardlinkSource);
            fails(() -> CustomSkinLibrary.read(preferences));
            Files.delete(hardlinkLibrary);

            Path externalFile = outside.resolve("library.json");
            byte[] original = new byte[1_048_577];
            Files.write(externalFile, original);
            Path file = directory.resolve("library.json");
            Files.createSymbolicLink(file, externalFile);
            fails(() -> CustomSkinLibrary.read(preferences));
            check(java.util.Arrays.equals(original, Files.readAllBytes(externalFile)));
            Files.delete(file);

            Path externalDirectory = outside.resolve("CustomSkins");
            Files.createDirectories(externalDirectory);
            Files.delete(directory);
            Files.createSymbolicLink(directory, externalDirectory);
            fails(() -> CustomSkinLibrary.read(preferences));
            Files.delete(directory);
            Files.createDirectories(directory);

            Path parentOutside = outside.resolve("parent-outside");
            Files.createDirectories(parentOutside);
            Path linkedParent = root.resolve("linked-parent");
            Files.createSymbolicLink(linkedParent, parentOutside);
            boolean parentRejected = false;
            try {
                java.lang.reflect.Method ensure = CustomSkinLibrary.class
                    .getDeclaredMethod("ensureSafeDirectory", Path.class);
                ensure.setAccessible(true);
                ensure.invoke(null, linkedParent.resolve("preferences"));
            } catch (java.lang.reflect.InvocationTargetException expected) {
                check(expected.getCause() instanceof java.io.IOException);
                parentRejected = true;
            }
            check(parentRejected);
            check(!Files.exists(parentOutside.resolve("preferences/CustomSkins/library.json")));

            // The shared Rust store permits the documented 9 MiB library and 32 extended
            // graphemes per name. Check the Java reader's private contract directly because the
            // host check runs against android.jar, whose org.json parser is a runtime stub.
            java.lang.reflect.Field maximumBytes = CustomSkinLibrary.class
                .getDeclaredField("MAX_LIBRARY_BYTES");
            maximumBytes.setAccessible(true);
            check(maximumBytes.getLong(null) == 9_000_000L);
            java.lang.reflect.Method boundedName = CustomSkinLibrary.class
                .getDeclaredMethod("boundedName", String.class);
            boundedName.setAccessible(true);
            String graphemeName = "👩‍👩‍👧‍👦".repeat(32);
            check((Boolean) boundedName.invoke(null, graphemeName));
            check(!(Boolean) boundedName.invoke(null, graphemeName + "x"));
            java.lang.reflect.Method strictString = CustomSkinLibrary.class
                .getDeclaredMethod("strictString", Object.class);
            strictString.setAccessible(true);
            check("synthetic".equals(strictString.invoke(null, "synthetic")));
            check(strictString.invoke(null, 42) == null);
            check(strictString.invoke(null, Boolean.TRUE) == null);

            Path externalRoot = outside.resolve("preferences");
            Files.createDirectories(externalRoot.resolve("CustomSkins"));
            Path linkedRoot = root.resolve("linked-preferences");
            Files.createSymbolicLink(linkedRoot, externalRoot);
            fails(() -> CustomSkinLibrary.read(linkedRoot));
            System.out.println("Android custom skin library: symlink boundaries passed");
        } finally {
            try (Stream<Path> paths = Files.walk(root)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
            try (Stream<Path> paths = Files.walk(outside)) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> {
                    try { Files.deleteIfExists(path); }
                    catch (Exception error) { throw new IllegalStateException(error); }
                });
            }
        }
    }
}
