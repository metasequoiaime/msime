import app.msime.client.policy.HostOptionsPolicy;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public final class HostOptionsPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws IOException {
        Path root = Files.createTempDirectory("msime-host-options");
        try {
            Path options = root.resolve("runtime-options.json");
            String valid = "{\"preferences_directory\":\"/data/user/0/app/files/preferences\"}";
            Files.write(options, valid.getBytes(StandardCharsets.UTF_8));
            check(HostOptionsPolicy.read(options.toFile()).equals(valid), "a valid options document is read");

            Files.write(options, new byte[HostOptionsPolicy.MAX_BYTES + 1]);
            check(HostOptionsPolicy.read(options.toFile()).isEmpty(), "an oversized options document is refused");
            check(HostOptionsPolicy.read(root.resolve("missing").toFile()).isEmpty(), "a missing options file is empty");
        } finally {
            Files.deleteIfExists(root.resolve("runtime-options.json"));
            Files.delete(root);
        }
        System.out.println("HostOptionsPolicySmoke passed");
    }
}
