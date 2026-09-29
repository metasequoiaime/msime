import app.msime.client.CustomSkinLibrary;
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
